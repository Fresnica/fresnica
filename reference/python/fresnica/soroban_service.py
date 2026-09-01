"""Soroban simulation -> assembly -> authorization -> submission semantics.

This module deliberately has no ``stellar_sdk`` dependency. Network/XDR-specific
inspection lives behind the injected adapter so lifecycle semantics can be proved
with deterministic unit tests before a RustClient/shared RPC contract exists.
"""

from dataclasses import dataclass
import time

from .errors import (
    NetworkError,
    SignerError,
    TransactionError,
    TransactionSubmissionUncertain,
)


@dataclass(frozen=True)
class SorobanEnvelopeFacts:
    transaction_source: str
    operation_source: str
    invocation_xdr: str
    contract_id: str
    function_name: str
    argument_count: int
    authorizers: tuple[str, ...]
    credential_types: tuple[str, ...]
    auth_entry_count: int
    total_fee_stroops: int
    resource_fee_stroops: int
    has_soroban_data: bool
    signature_count: int


@dataclass(frozen=True)
class SorobanAuthorizationEntryFacts:
    index: int
    entry_xdr: str
    authorizer: str
    credential_type: str
    nonce: int | None
    signature_expiration_ledger: int | None
    root_invocation_xdr: str


@dataclass(frozen=True)
class SorobanSimulationResult:
    raw_response: object
    error: str | None
    restore_required: bool
    min_resource_fee: int
    latest_ledger: int


@dataclass(frozen=True)
class SorobanReview:
    wallet_name: str
    fee_payer: str
    operation_source: str
    contract_id: str
    function_name: str
    argument_count: int
    authorizers: tuple[str, ...]
    credential_types: tuple[str, ...]
    auth_entry_count: int
    total_fee_stroops: int
    resource_fee_stroops: int
    inclusion_fee_stroops: int
    min_resource_fee: int
    simulation_ledger: int
    authorization_expiration_ledger: int | None
    network: str
    transaction_hash: str


@dataclass
class PreparedSorobanTransaction:
    envelope: object
    review: SorobanReview
    reviewed_envelope_xdr: str
    reviewed_transaction_hash: str
    authorized_envelope_xdr: str | None = None
    authorized_transaction_hash: str | None = None

    @property
    def signing_transaction_hash(self) -> str:
        return self.authorized_transaction_hash or self.reviewed_transaction_hash

    def assert_review_binding(self) -> None:
        """Fail if the currently authorized object changed before envelope signing."""
        current_xdr = self.envelope.to_xdr()
        current_hash = self.envelope.hash_hex()
        expected_xdr = self.authorized_envelope_xdr or self.reviewed_envelope_xdr
        expected_hash = self.signing_transaction_hash
        if current_xdr != expected_xdr or current_hash != expected_hash:
            raise TransactionError(
                "Soroban transaction changed after review; prepare and review it again"
            )

    def _bind_authorized_envelope(self) -> None:
        """Rebind exact XDR/hash after reviewed detached auth entries are signed."""
        self.authorized_envelope_xdr = self.envelope.to_xdr()
        self.authorized_transaction_hash = self.envelope.hash_hex()

    def assert_submit_binding(self) -> None:
        """Envelope signatures may change XDR, but the signed transaction hash may not."""
        if self.envelope.hash_hex() != self.signing_transaction_hash:
            raise TransactionError(
                "Soroban transaction changed after signing; prepare and review it again"
            )


class SorobanSimulationService:
    """Prepare one contract invocation using RPC simulation as authoritative input."""

    def __init__(self, adapter, authorization_lifetime_ledgers: int = 100):
        if authorization_lifetime_ledgers < 1:
            raise ValueError("authorization_lifetime_ledgers must be at least 1")
        self.adapter = adapter
        self.authorization_lifetime_ledgers = authorization_lifetime_ledgers

    def prepare(
        self,
        wallet_name: str,
        candidate_envelope,
    ) -> PreparedSorobanTransaction:
        candidate = self.adapter.inspect_envelope(candidate_envelope)
        _validate_candidate(candidate)

        simulation = self.adapter.simulate_transaction(candidate_envelope)
        if simulation.error:
            raise TransactionError(
                "Soroban transaction simulation failed",
                details=simulation.error,
            )
        if simulation.restore_required:
            raise TransactionError(
                "Soroban transaction requires an explicit footprint restore before review"
            )

        assembled_envelope = self.adapter.prepare_transaction(
            candidate_envelope,
            simulation.raw_response,
        )
        authorization_expiration_ledger = (
            simulation.latest_ledger + self.authorization_lifetime_ledgers
        )
        if authorization_expiration_ledger > 0xFFFFFFFF:
            raise TransactionError(
                "Soroban authorization expiration exceeds the ledger sequence range"
            )
        detached_auth_count = self.adapter.set_authorization_expiration_ledger(
            assembled_envelope,
            authorization_expiration_ledger,
        )
        assembled = self.adapter.inspect_envelope(assembled_envelope)
        _validate_assembled(candidate, assembled)

        reviewed_xdr = assembled_envelope.to_xdr()
        reviewed_hash = assembled_envelope.hash_hex()
        inclusion_fee = assembled.total_fee_stroops - assembled.resource_fee_stroops
        review = SorobanReview(
            wallet_name=wallet_name,
            fee_payer=assembled.transaction_source,
            operation_source=assembled.operation_source,
            contract_id=assembled.contract_id,
            function_name=assembled.function_name,
            argument_count=assembled.argument_count,
            authorizers=assembled.authorizers,
            credential_types=assembled.credential_types,
            auth_entry_count=assembled.auth_entry_count,
            total_fee_stroops=assembled.total_fee_stroops,
            resource_fee_stroops=assembled.resource_fee_stroops,
            inclusion_fee_stroops=inclusion_fee,
            min_resource_fee=simulation.min_resource_fee,
            simulation_ledger=simulation.latest_ledger,
            authorization_expiration_ledger=(
                authorization_expiration_ledger if detached_auth_count else None
            ),
            network=self.adapter.network.name,
            transaction_hash=reviewed_hash,
        )
        return PreparedSorobanTransaction(
            envelope=assembled_envelope,
            review=review,
            reviewed_envelope_xdr=reviewed_xdr,
            reviewed_transaction_hash=reviewed_hash,
        )


class SorobanAuthorizationService:
    """Apply only reviewed direct G-account authorization entries to an assembled tx."""

    def __init__(self, adapter):
        self.adapter = adapter

    def authorize(self, prepared: PreparedSorobanTransaction, signers_by_authorizer: dict):
        prepared.assert_review_binding()
        entries = self.adapter.authorization_entries(prepared.envelope)
        if len(entries) != prepared.review.auth_entry_count:
            raise TransactionError("Soroban authorization entry count changed after review")

        for entry in entries:
            if entry.credential_type == "source-account":
                continue
            if entry.credential_type not in {"address", "address-v2"}:
                raise TransactionError(
                    f"Unsupported detached Soroban authorization: {entry.credential_type}"
                )
            if not entry.authorizer.startswith("G"):
                raise TransactionError(
                    "Unsupported detached Soroban authorizer; only Classic G accounts are supported"
                )
            signer = signers_by_authorizer.get(entry.authorizer)
            if signer is None:
                raise SignerError(
                    f"No signer capability provided for Soroban authorizer {entry.authorizer}"
                )
            sign_entry = getattr(signer, "sign_soroban_authorization_entry", None)
            if not callable(sign_entry):
                raise SignerError(
                    "Selected signer cannot sign Soroban authorization entries"
                )
            signed_xdr = sign_entry(entry.entry_xdr, self.adapter.network.passphrase)
            if not isinstance(signed_xdr, str) or signed_xdr == entry.entry_xdr:
                raise SignerError("Soroban authorization signer returned no signed entry")
            signed = self.adapter.inspect_authorization_entry_xdr(
                signed_xdr,
                prepared.review.operation_source,
                index=entry.index,
            )
            _validate_signed_authorization(entry, signed)
            self.adapter.replace_authorization_entry(
                prepared.envelope,
                index=entry.index,
                expected_entry_xdr=entry.entry_xdr,
                signed_entry_xdr=signed_xdr,
            )

        prepared._bind_authorized_envelope()
        return prepared


class SorobanSubmitService:
    """Submit through RPC and reconcile PENDING/DUPLICATE to a terminal status."""

    def __init__(self, adapter, max_attempts: int = 30, sleep_seconds: float = 1.0, sleep=None):
        if max_attempts < 1:
            raise ValueError("max_attempts must be at least 1")
        self.adapter = adapter
        self.max_attempts = max_attempts
        self.sleep_seconds = sleep_seconds
        self.sleep = sleep or time.sleep

    def submit(self, envelope) -> dict:
        tx_hash = envelope.hash_hex()
        try:
            response = self.adapter.send_transaction(envelope)
        except NetworkError as exc:
            raise TransactionSubmissionUncertain(tx_hash, details=str(exc)) from exc

        returned_hash = response.get("hash")
        if returned_hash != tx_hash:
            raise TransactionError(
                "Soroban RPC returned a transaction hash that does not match the signed transaction"
            )
        status = response.get("status")
        if status == "ERROR":
            raise TransactionError(
                "Soroban RPC rejected the transaction",
                details=response.get("error_result_xdr"),
            )
        if status == "TRY_AGAIN_LATER":
            raise TransactionError("Soroban RPC did not accept the transaction; try again later")
        if status not in {"PENDING", "DUPLICATE"}:
            raise TransactionError(f"Unsupported Soroban RPC submission status: {status}")

        for attempt in range(self.max_attempts):
            try:
                terminal = self.lookup_transaction(tx_hash)
            except NetworkError as exc:
                raise TransactionSubmissionUncertain(tx_hash, details=str(exc)) from exc
            if terminal is not None:
                return terminal
            if attempt + 1 < self.max_attempts:
                self.sleep(self.sleep_seconds)

        raise TransactionSubmissionUncertain(
            tx_hash,
            details="Soroban RPC did not report a terminal transaction status in time",
        )

    def lookup_transaction(self, tx_hash: str) -> dict | None:
        return self.adapter.lookup_transaction(tx_hash)


def _validate_candidate(facts: SorobanEnvelopeFacts) -> None:
    if facts.signature_count:
        raise TransactionError("Soroban candidate must be unsigned before simulation")
    if facts.auth_entry_count:
        # stellar-sdk prepare_transaction deliberately preserves caller-provided auth
        # over simulation results. Fresnica v1 makes simulation authoritative instead.
        raise TransactionError(
            "Soroban candidate must not contain preexisting authorization entries"
        )
    if facts.has_soroban_data:
        raise TransactionError(
            "Soroban candidate must not contain preassembled transaction data"
        )


def _validate_assembled(
    candidate: SorobanEnvelopeFacts,
    assembled: SorobanEnvelopeFacts,
) -> None:
    if assembled.signature_count:
        raise TransactionError("Assembled Soroban transaction must remain unsigned")
    if not assembled.has_soroban_data:
        raise TransactionError(
            "Soroban assembly did not produce authoritative transaction data"
        )
    if assembled.resource_fee_stroops < 0:
        raise TransactionError("Soroban assembly produced a negative resource fee")
    if assembled.total_fee_stroops < assembled.resource_fee_stroops:
        raise TransactionError(
            "Soroban assembly resource fee exceeds the total transaction fee"
        )

    immutable_fields = (
        "transaction_source",
        "operation_source",
        "invocation_xdr",
        "contract_id",
        "function_name",
        "argument_count",
    )
    changed = [
        field
        for field in immutable_fields
        if getattr(candidate, field) != getattr(assembled, field)
    ]
    if changed:
        raise TransactionError(
            "Soroban assembly changed the reviewed invocation intent",
            details="changed fields: " + ", ".join(changed),
        )


def _validate_signed_authorization(
    reviewed: SorobanAuthorizationEntryFacts,
    signed: SorobanAuthorizationEntryFacts,
) -> None:
    immutable_fields = (
        "index",
        "authorizer",
        "credential_type",
        "nonce",
        "signature_expiration_ledger",
        "root_invocation_xdr",
    )
    changed = [
        field
        for field in immutable_fields
        if getattr(reviewed, field) != getattr(signed, field)
    ]
    if changed:
        raise TransactionError(
            "Soroban authorization signer changed reviewed authorization meaning",
            details="changed fields: " + ", ".join(changed),
        )
