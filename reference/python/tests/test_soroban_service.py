from dataclasses import replace

import pytest

from fresnica.errors import TransactionError
from fresnica.soroban_service import (
    SorobanEnvelopeFacts,
    SorobanSimulationResult,
    SorobanSimulationService,
)


class FakeNetwork:
    name = "testnet"


class FakeEnvelope:
    def __init__(self, xdr, tx_hash):
        self.xdr = xdr
        self.tx_hash = tx_hash

    def to_xdr(self):
        return self.xdr

    def hash_hex(self):
        return self.tx_hash


BASE_FACTS = SorobanEnvelopeFacts(
    transaction_source="GFEEPAYER",
    operation_source="GOPSOURCE",
    invocation_xdr="invoke-contract-xdr",
    contract_id="CCONTRACT",
    function_name="transfer",
    argument_count=3,
    authorizers=(),
    credential_types=(),
    auth_entry_count=0,
    total_fee_stroops=100,
    resource_fee_stroops=0,
    has_soroban_data=False,
    signature_count=0,
)

ASSEMBLED_FACTS = replace(
    BASE_FACTS,
    authorizers=("GOPSOURCE", "CAUTHORIZER"),
    credential_types=("source-account", "address-v2"),
    auth_entry_count=2,
    total_fee_stroops=5100,
    resource_fee_stroops=5000,
    has_soroban_data=True,
)


class FakeAdapter:
    network = FakeNetwork()

    def __init__(self, candidate_facts=BASE_FACTS, assembled_facts=ASSEMBLED_FACTS):
        self.candidate = FakeEnvelope("candidate-xdr", "candidate-hash")
        self.assembled = FakeEnvelope("assembled-xdr", "assembled-hash")
        self.candidate_facts = candidate_facts
        self.assembled_facts = assembled_facts
        self.simulation = SorobanSimulationResult(
            raw_response=object(),
            error=None,
            restore_required=False,
            min_resource_fee=4900,
            latest_ledger=123456,
        )
        self.calls = []

    def inspect_envelope(self, envelope):
        self.calls.append(("inspect", envelope))
        if envelope is self.candidate:
            return self.candidate_facts
        assert envelope is self.assembled
        return self.assembled_facts

    def simulate_transaction(self, envelope):
        self.calls.append(("simulate", envelope))
        return self.simulation

    def prepare_transaction(self, envelope, simulation_response):
        self.calls.append(("prepare", envelope, simulation_response))
        return self.assembled

    def set_authorization_expiration_ledger(self, envelope, valid_until_ledger_sequence):
        self.calls.append(("set-auth-expiration", envelope, valid_until_ledger_sequence))
        detached = sum(
            credential != "source-account"
            for credential in self.assembled_facts.credential_types
        )
        if detached:
            envelope.xdr = "assembled-expiry-xdr"
            envelope.tx_hash = "assembled-expiry-hash"
        return detached


def test_review_is_derived_from_post_simulation_assembled_transaction():
    adapter = FakeAdapter()
    prepared = SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert [call[0] for call in adapter.calls] == [
        "inspect",
        "simulate",
        "prepare",
        "set-auth-expiration",
        "inspect",
    ]
    assert prepared.envelope is adapter.assembled
    assert prepared.review.fee_payer == "GFEEPAYER"
    assert prepared.review.operation_source == "GOPSOURCE"
    assert prepared.review.authorizers == ("GOPSOURCE", "CAUTHORIZER")
    assert prepared.review.credential_types == ("source-account", "address-v2")
    assert prepared.review.total_fee_stroops == 5100
    assert prepared.review.resource_fee_stroops == 5000
    assert prepared.review.inclusion_fee_stroops == 100
    assert prepared.review.min_resource_fee == 4900
    assert prepared.review.simulation_ledger == 123456
    assert prepared.review.authorization_expiration_ledger == 123556
    assert prepared.review.transaction_hash == "assembled-expiry-hash"
    assert prepared.review.transaction_hash != adapter.candidate.hash_hex()


def test_detached_authorization_expiration_is_set_before_review_binding():
    adapter = FakeAdapter()

    prepared = SorobanSimulationService(
        adapter,
        authorization_lifetime_ledgers=25,
    ).prepare("main", adapter.candidate)

    expiration_call = next(
        call for call in adapter.calls if call[0] == "set-auth-expiration"
    )
    assert expiration_call[2] == 123481
    assert prepared.review.authorization_expiration_ledger == 123481
    assert prepared.reviewed_envelope_xdr == "assembled-expiry-xdr"
    assert prepared.reviewed_transaction_hash == "assembled-expiry-hash"


def test_candidate_with_preexisting_auth_is_rejected_before_rpc():
    adapter = FakeAdapter(
        candidate_facts=replace(BASE_FACTS, auth_entry_count=1),
    )

    with pytest.raises(TransactionError, match="preexisting authorization"):
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert [call[0] for call in adapter.calls] == ["inspect"]


def test_candidate_with_preassembled_soroban_data_is_rejected_before_rpc():
    adapter = FakeAdapter(
        candidate_facts=replace(BASE_FACTS, has_soroban_data=True),
    )

    with pytest.raises(TransactionError, match="preassembled transaction data"):
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert [call[0] for call in adapter.calls] == ["inspect"]


def test_signed_candidate_is_rejected_before_rpc():
    adapter = FakeAdapter(candidate_facts=replace(BASE_FACTS, signature_count=1))

    with pytest.raises(TransactionError, match="must be unsigned"):
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert [call[0] for call in adapter.calls] == ["inspect"]


def test_simulation_error_stops_before_assembly():
    adapter = FakeAdapter()
    adapter.simulation = replace(adapter.simulation, error="HostError: boom")

    with pytest.raises(TransactionError, match="simulation failed") as captured:
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert captured.value.details == "HostError: boom"
    assert [call[0] for call in adapter.calls] == ["inspect", "simulate"]


def test_restore_preamble_requires_separate_explicit_lifecycle():
    adapter = FakeAdapter()
    adapter.simulation = replace(adapter.simulation, restore_required=True)

    with pytest.raises(TransactionError, match="explicit footprint restore"):
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert [call[0] for call in adapter.calls] == ["inspect", "simulate"]


@pytest.mark.parametrize(
    "changed_facts, expected_field",
    [
        (replace(ASSEMBLED_FACTS, invocation_xdr="other-invocation"), "invocation_xdr"),
        (replace(ASSEMBLED_FACTS, transaction_source="GOTHER"), "transaction_source"),
        (replace(ASSEMBLED_FACTS, operation_source="GOTHER"), "operation_source"),
    ],
)
def test_assembly_cannot_mutate_intent_identity(changed_facts, expected_field):
    adapter = FakeAdapter(assembled_facts=changed_facts)

    with pytest.raises(TransactionError, match="changed the reviewed invocation") as captured:
        SorobanSimulationService(adapter).prepare("main", adapter.candidate)

    assert expected_field in captured.value.details


def test_assembled_transaction_must_stay_unsigned_and_have_soroban_data():
    signed = FakeAdapter(assembled_facts=replace(ASSEMBLED_FACTS, signature_count=1))
    with pytest.raises(TransactionError, match="must remain unsigned"):
        SorobanSimulationService(signed).prepare("main", signed.candidate)

    missing_data = FakeAdapter(
        assembled_facts=replace(ASSEMBLED_FACTS, has_soroban_data=False)
    )
    with pytest.raises(TransactionError, match="authoritative transaction data"):
        SorobanSimulationService(missing_data).prepare("main", missing_data.candidate)



def test_assembled_fee_relationship_must_be_protocol_sane():
    negative = FakeAdapter(
        assembled_facts=replace(ASSEMBLED_FACTS, resource_fee_stroops=-1)
    )
    with pytest.raises(TransactionError, match="negative resource fee"):
        SorobanSimulationService(negative).prepare("main", negative.candidate)

    exceeds_total = FakeAdapter(
        assembled_facts=replace(
            ASSEMBLED_FACTS,
            total_fee_stroops=100,
            resource_fee_stroops=101,
        )
    )
    with pytest.raises(TransactionError, match="exceeds the total"):
        SorobanSimulationService(exceeds_total).prepare(
            "main", exceeds_total.candidate
        )

def test_review_binding_detects_post_review_envelope_mutation():
    adapter = FakeAdapter()
    prepared = SorobanSimulationService(adapter).prepare("main", adapter.candidate)
    prepared.assert_review_binding()

    prepared.envelope.xdr = "mutated-xdr"
    prepared.envelope.tx_hash = "mutated-hash"
    with pytest.raises(TransactionError, match="changed after review"):
        prepared.assert_review_binding()
