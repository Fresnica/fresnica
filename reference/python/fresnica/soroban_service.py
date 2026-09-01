"""Soroban simulation -> assembly -> review semantics for the RefPython lab.

This module deliberately has no ``stellar_sdk`` dependency. Network/XDR-specific
inspection lives behind the injected adapter so the lifecycle semantics can be
proved with deterministic unit tests before a RustClient/shared RPC contract is
introduced.
"""

from dataclasses import dataclass

from .errors import TransactionError


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
    network: str
    transaction_hash: str


@dataclass
class PreparedSorobanTransaction:
    envelope: object
    review: SorobanReview
    reviewed_envelope_xdr: str
    reviewed_transaction_hash: str

    def assert_review_binding(self) -> None:
        """Fail if the assembled envelope changed after semantic review."""
        current_xdr = self.envelope.to_xdr()
        current_hash = self.envelope.hash_hex()
        if (
            current_xdr != self.reviewed_envelope_xdr
            or current_hash != self.reviewed_transaction_hash
        ):
            raise TransactionError(
                "Soroban transaction changed after review; prepare and review it again"
            )


class SorobanSimulationService:
    """Prepare one contract invocation using RPC simulation as authoritative input."""

    def __init__(self, adapter):
        self.adapter = adapter

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
            network=self.adapter.network.name,
            transaction_hash=reviewed_hash,
        )
        return PreparedSorobanTransaction(
            envelope=assembled_envelope,
            review=review,
            reviewed_envelope_xdr=reviewed_xdr,
            reviewed_transaction_hash=reviewed_hash,
        )


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
