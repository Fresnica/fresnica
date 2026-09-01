from dataclasses import replace
from types import SimpleNamespace

import pytest

from fresnica.errors import (
    NetworkError,
    SignerError,
    TransactionError,
    TransactionSubmissionUncertain,
)
from fresnica.soroban_service import (
    PreparedSorobanTransaction,
    SorobanAuthorizationEntryFacts,
    SorobanAuthorizationService,
    SorobanReview,
    SorobanSubmitService,
)


class FakeEnvelope:
    def __init__(self, xdr="reviewed-xdr", tx_hash="reviewed-hash"):
        self.xdr = xdr
        self.tx_hash = tx_hash

    def to_xdr(self):
        return self.xdr

    def hash_hex(self):
        return self.tx_hash


REVIEW = SorobanReview(
    wallet_name="main",
    fee_payer="GFEEPAYER",
    operation_source="GFEEPAYER",
    contract_id="CCONTRACT",
    function_name="transfer",
    argument_count=3,
    authorizers=("GAUTH",),
    credential_types=("address-v2",),
    auth_entry_count=1,
    total_fee_stroops=5100,
    resource_fee_stroops=5000,
    inclusion_fee_stroops=100,
    min_resource_fee=4900,
    simulation_ledger=123456,
    network="testnet",
    transaction_hash="reviewed-hash",
)


def _prepared(review=REVIEW):
    envelope = FakeEnvelope()
    return PreparedSorobanTransaction(
        envelope=envelope,
        review=review,
        reviewed_envelope_xdr=envelope.to_xdr(),
        reviewed_transaction_hash=envelope.hash_hex(),
    )


ADDRESS_ENTRY = SorobanAuthorizationEntryFacts(
    index=0,
    entry_xdr="unsigned-entry",
    authorizer="GAUTH",
    credential_type="address-v2",
    nonce=42,
    signature_expiration_ledger=500,
    root_invocation_xdr="root-invocation",
)


class FakeAuthorizationAdapter:
    network = SimpleNamespace(passphrase="Test SDF Network ; September 2015")

    def __init__(self, entries=(ADDRESS_ENTRY,), signed_facts=None):
        self.entries = entries
        self.signed_facts = signed_facts or replace(
            ADDRESS_ENTRY,
            entry_xdr="signed-entry",
        )
        self.replacements = []

    def authorization_entries(self, envelope):
        return self.entries

    def inspect_authorization_entry_xdr(self, entry_xdr, operation_source, *, index):
        assert entry_xdr == "signed-entry"
        assert operation_source == "GFEEPAYER"
        assert index == 0
        return self.signed_facts

    def replace_authorization_entry(
        self,
        envelope,
        *,
        index,
        expected_entry_xdr,
        signed_entry_xdr,
    ):
        assert index == 0
        assert expected_entry_xdr == "unsigned-entry"
        assert signed_entry_xdr == "signed-entry"
        self.replacements.append(index)
        envelope.xdr = "authorized-xdr"
        envelope.tx_hash = "authorized-hash"


class RecordingAuthorizationSigner:
    def __init__(self):
        self.calls = []

    def sign_soroban_authorization_entry(self, entry_xdr, network_passphrase):
        self.calls.append((entry_xdr, network_passphrase))
        return "signed-entry"


def test_source_account_authorization_requires_no_detached_signer():
    source_entry = replace(
        ADDRESS_ENTRY,
        authorizer="GFEEPAYER",
        credential_type="source-account",
        nonce=None,
        signature_expiration_ledger=None,
    )
    prepared = _prepared(
        replace(
            REVIEW,
            authorizers=("GFEEPAYER",),
            credential_types=("source-account",),
        )
    )
    adapter = FakeAuthorizationAdapter(entries=(source_entry,))

    SorobanAuthorizationService(adapter).authorize(prepared, {})

    assert adapter.replacements == []
    assert prepared.signing_transaction_hash == "reviewed-hash"
    prepared.assert_review_binding()


def test_detached_address_v2_authorization_rebinds_exact_transaction():
    prepared = _prepared()
    adapter = FakeAuthorizationAdapter()
    signer = RecordingAuthorizationSigner()

    result = SorobanAuthorizationService(adapter).authorize(
        prepared,
        {"GAUTH": signer},
    )

    assert result is prepared
    assert signer.calls == [
        ("unsigned-entry", "Test SDF Network ; September 2015")
    ]
    assert adapter.replacements == [0]
    assert prepared.authorized_envelope_xdr == "authorized-xdr"
    assert prepared.signing_transaction_hash == "authorized-hash"
    prepared.assert_review_binding()

    prepared.envelope.tx_hash = "mutated-after-authorization"
    with pytest.raises(TransactionError, match="changed after review"):
        prepared.assert_review_binding()


def test_detached_authorization_fails_closed_without_capable_signer():
    prepared = _prepared()
    adapter = FakeAuthorizationAdapter()

    with pytest.raises(SignerError, match="No signer capability"):
        SorobanAuthorizationService(adapter).authorize(prepared, {})

    with pytest.raises(SignerError, match="cannot sign Soroban"):
        SorobanAuthorizationService(adapter).authorize(
            _prepared(),
            {"GAUTH": object()},
        )


def test_signed_authorization_cannot_change_reviewed_meaning():
    changed = replace(ADDRESS_ENTRY, entry_xdr="signed-entry", nonce=43)
    adapter = FakeAuthorizationAdapter(signed_facts=changed)

    with pytest.raises(TransactionError, match="changed reviewed authorization") as captured:
        SorobanAuthorizationService(adapter).authorize(
            _prepared(),
            {"GAUTH": RecordingAuthorizationSigner()},
        )

    assert "nonce" in captured.value.details
    assert adapter.replacements == []


def test_delegated_authorization_fails_closed():
    delegated = replace(ADDRESS_ENTRY, credential_type="address-with-delegates")
    adapter = FakeAuthorizationAdapter(entries=(delegated,))

    with pytest.raises(TransactionError, match="Unsupported detached"):
        SorobanAuthorizationService(adapter).authorize(
            _prepared(),
            {"GAUTH": RecordingAuthorizationSigner()},
        )


class FakeSubmitAdapter:
    def __init__(self, send=None, lookups=()):
        self.send = send or {"hash": "signed-hash", "status": "PENDING"}
        self.lookups = list(lookups)
        self.sent = []

    def send_transaction(self, envelope):
        self.sent.append(envelope)
        if isinstance(self.send, Exception):
            raise self.send
        return self.send

    def lookup_transaction(self, tx_hash):
        if not self.lookups:
            return None
        value = self.lookups.pop(0)
        if isinstance(value, Exception):
            raise value
        return value


def test_submit_reconciles_pending_to_terminal_success():
    envelope = FakeEnvelope(tx_hash="signed-hash")
    adapter = FakeSubmitAdapter(
        lookups=[
            None,
            {
                "hash": "signed-hash",
                "ledger": 123,
                "successful": True,
                "status": "SUCCESS",
            },
        ]
    )
    sleeps = []
    service = SorobanSubmitService(
        adapter,
        max_attempts=3,
        sleep_seconds=0.25,
        sleep=sleeps.append,
    )

    result = service.submit(envelope)

    assert result["successful"] is True
    assert result["ledger"] == 123
    assert sleeps == [0.25]


def test_submit_returns_terminal_failure_for_caller_to_present():
    envelope = FakeEnvelope(tx_hash="signed-hash")
    failed = {
        "hash": "signed-hash",
        "ledger": 124,
        "successful": False,
        "status": "FAILED",
    }
    result = SorobanSubmitService(
        FakeSubmitAdapter(lookups=[failed]),
        max_attempts=1,
    ).submit(envelope)
    assert result == failed


@pytest.mark.parametrize("status", ["ERROR", "TRY_AGAIN_LATER"])
def test_submit_rejects_statuses_that_were_not_accepted(status):
    envelope = FakeEnvelope(tx_hash="signed-hash")
    adapter = FakeSubmitAdapter(send={"hash": "signed-hash", "status": status})
    with pytest.raises(TransactionError):
        SorobanSubmitService(adapter, max_attempts=1).submit(envelope)


def test_submit_rejects_rpc_hash_mismatch():
    envelope = FakeEnvelope(tx_hash="signed-hash")
    adapter = FakeSubmitAdapter(send={"hash": "other-hash", "status": "PENDING"})
    with pytest.raises(TransactionError, match="does not match"):
        SorobanSubmitService(adapter, max_attempts=1).submit(envelope)


def test_network_or_poll_timeout_becomes_uncertain_submission():
    envelope = FakeEnvelope(tx_hash="signed-hash")
    network = FakeSubmitAdapter(send=NetworkError("network down"))
    with pytest.raises(TransactionSubmissionUncertain) as captured:
        SorobanSubmitService(network, max_attempts=1).submit(envelope)
    assert captured.value.tx_hash == "signed-hash"

    pending = FakeSubmitAdapter(lookups=[None])
    with pytest.raises(TransactionSubmissionUncertain) as captured:
        SorobanSubmitService(pending, max_attempts=1).submit(envelope)
    assert captured.value.tx_hash == "signed-hash"


class PendingRecorder:
    def __init__(self):
        self.calls = []

    def remember(self, account, tx_hash, kind):
        self.calls.append((account, tx_hash, kind))


def test_transaction_service_uses_fee_payer_for_uncertain_soroban_pending():
    from fresnica.transaction_service import TransactionService

    prepared = _prepared()
    prepared._bind_authorized_envelope()
    pending = PendingRecorder()
    submitter = SorobanSubmitService(
        FakeSubmitAdapter(send=NetworkError("network down")),
        max_attempts=1,
    )

    with pytest.raises(TransactionSubmissionUncertain):
        TransactionService(submitter, pending).submit(prepared)

    assert pending.calls == [("GFEEPAYER", "reviewed-hash", "transaction")]


def test_submit_binding_allows_envelope_signature_xdr_but_not_hash_mutation():
    prepared = _prepared()
    prepared._bind_authorized_envelope()
    prepared.envelope.xdr = "signed-envelope-xdr"
    prepared.assert_submit_binding()

    prepared.envelope.tx_hash = "other-hash"
    with pytest.raises(TransactionError, match="changed after signing"):
        prepared.assert_submit_binding()
