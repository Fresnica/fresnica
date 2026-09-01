"""Official stellar-sdk backed Soroban RPC/assembly adapter for RefPython."""

from stellar_sdk import Address, SorobanServer, xdr
from stellar_sdk.exceptions import PrepareTransactionException, SdkError
from stellar_sdk.operation import InvokeHostFunction
from stellar_sdk.soroban_rpc import GetTransactionStatus

from .errors import NetworkError, TransactionError
from .soroban_service import (
    SorobanAuthorizationEntryFacts,
    SorobanEnvelopeFacts,
    SorobanSimulationResult,
)


class SorobanRpcAdapter:
    """Narrow provider adapter; shared RPC abstractions wait for RefPython evidence."""

    def __init__(self, network, rpc_url: str):
        if not rpc_url:
            raise ValueError("rpc_url is required")
        self.network = network
        self.rpc_url = rpc_url
        self.server = SorobanServer(server_url=rpc_url)

    def load_account(self, address: str):
        try:
            return self.server.load_account(address)
        except SdkError as exc:
            raise NetworkError(
                f"Unable to load Soroban source account {address}",
                details=str(exc),
            ) from exc

    def simulate_transaction(self, envelope) -> SorobanSimulationResult:
        try:
            response = self.server.simulate_transaction(
                envelope,
                use_upgraded_auth=True,
            )
        except SdkError as exc:
            raise NetworkError(
                "Soroban RPC simulation request failed",
                details=str(exc),
            ) from exc

        if response.error is None and response.min_resource_fee is None:
            raise TransactionError(
                "Soroban simulation response is missing minResourceFee"
            )
        return SorobanSimulationResult(
            raw_response=response,
            error=response.error,
            restore_required=response.restore_preamble is not None,
            min_resource_fee=_integer(response.min_resource_fee),
            latest_ledger=int(response.latest_ledger),
        )

    def prepare_transaction(self, envelope, simulation_response):
        try:
            return self.server.prepare_transaction(
                envelope,
                simulate_transaction_response=simulation_response,
            )
        except (PrepareTransactionException, SdkError, ValueError, TypeError) as exc:
            raise TransactionError(
                "Soroban transaction assembly failed",
                details=str(exc),
            ) from exc

    def inspect_envelope(self, envelope) -> SorobanEnvelopeFacts:
        operation, transaction_source, operation_source = self._invoke_operation(envelope)
        host_function = operation.host_function
        invoke = host_function.invoke_contract
        assert invoke is not None

        authorizers: list[str] = []
        credential_types: list[str] = []
        for auth in operation.auth:
            authorizer, credential_type = _authorization_identity(
                auth.credentials,
                operation_source,
            )
            authorizers.append(authorizer)
            credential_types.append(credential_type)

        transaction = envelope.transaction
        soroban_data = transaction.soroban_data
        resource_fee = (
            soroban_data.resource_fee.int64 if soroban_data is not None else 0
        )
        return SorobanEnvelopeFacts(
            transaction_source=transaction_source,
            operation_source=operation_source,
            invocation_xdr=host_function.to_xdr(),
            contract_id=Address.from_xdr_sc_address(invoke.contract_address).address,
            function_name=invoke.function_name.sc_symbol.decode("utf-8"),
            argument_count=len(invoke.args),
            authorizers=tuple(authorizers),
            credential_types=tuple(credential_types),
            auth_entry_count=len(operation.auth),
            total_fee_stroops=int(transaction.fee),
            resource_fee_stroops=int(resource_fee),
            has_soroban_data=soroban_data is not None,
            signature_count=len(envelope.signatures),
        )

    def authorization_entries(self, envelope) -> tuple[SorobanAuthorizationEntryFacts, ...]:
        operation, _, operation_source = self._invoke_operation(envelope)
        return tuple(
            self._authorization_entry_facts(auth, operation_source, index)
            for index, auth in enumerate(operation.auth)
        )

    def set_authorization_expiration_ledger(
        self,
        envelope,
        valid_until_ledger_sequence: int,
    ) -> int:
        """Set detached auth expiry before the assembled object is reviewed."""
        operation, _, _ = self._invoke_operation(envelope)
        updated = 0
        for entry in operation.auth:
            credentials = _address_credentials(entry.credentials)
            if credentials is None:
                continue
            if credentials.signature.type != xdr.SCValType.SCV_VOID:
                raise TransactionError(
                    "Soroban assembly produced pre-signed authorization"
                )
            credentials.signature_expiration_ledger = xdr.Uint32(
                valid_until_ledger_sequence
            )
            updated += 1
        return updated

    def inspect_authorization_entry_xdr(
        self,
        entry_xdr: str,
        operation_source: str,
        *,
        index: int,
    ) -> SorobanAuthorizationEntryFacts:
        try:
            entry = xdr.SorobanAuthorizationEntry.from_xdr(entry_xdr)
        except (TypeError, ValueError) as exc:
            raise TransactionError(
                "Signed Soroban authorization entry is invalid",
                details=str(exc),
            ) from exc
        return self._authorization_entry_facts(entry, operation_source, index)

    def replace_authorization_entry(
        self,
        envelope,
        *,
        index: int,
        expected_entry_xdr: str,
        signed_entry_xdr: str,
    ) -> None:
        operation, _, _ = self._invoke_operation(envelope)
        try:
            current = operation.auth[index]
        except IndexError as exc:
            raise TransactionError("Soroban authorization entry index changed") from exc
        if current.to_xdr() != expected_entry_xdr:
            raise TransactionError(
                "Soroban authorization entry changed after review; prepare it again"
            )
        try:
            operation.auth[index] = xdr.SorobanAuthorizationEntry.from_xdr(
                signed_entry_xdr
            )
        except (TypeError, ValueError) as exc:
            raise TransactionError(
                "Signed Soroban authorization entry is invalid",
                details=str(exc),
            ) from exc

    def send_transaction(self, envelope) -> dict:
        try:
            response = self.server.send_transaction(envelope)
        except SdkError as exc:
            raise NetworkError(
                "Soroban RPC submission request failed",
                details=str(exc),
            ) from exc
        return {
            "hash": response.hash,
            "status": response.status.value,
            "latest_ledger": response.latest_ledger,
            "error_result_xdr": response.error_result_xdr,
            "diagnostic_events_xdr": response.diagnostic_events_xdr,
        }

    def lookup_transaction(self, tx_hash: str) -> dict | None:
        try:
            response = self.server.get_transaction(tx_hash)
        except SdkError as exc:
            raise NetworkError(
                f"Unable to reconcile Soroban transaction {tx_hash}",
                details=str(exc),
            ) from exc
        if response.status == GetTransactionStatus.NOT_FOUND:
            return None
        return {
            "hash": response.transaction_hash,
            "ledger": response.ledger,
            "successful": response.status == GetTransactionStatus.SUCCESS,
            "status": response.status.value,
            "result_xdr": response.result_xdr,
            "result_meta_xdr": response.result_meta_xdr,
            "envelope_xdr": response.envelope_xdr,
            "diagnostic_events_xdr": response.diagnostic_events_xdr,
        }

    def _invoke_operation(self, envelope):
        if envelope.network_passphrase != self.network.passphrase:
            raise TransactionError("Soroban transaction network does not match adapter")
        transaction = envelope.transaction
        if not transaction.v1:
            raise TransactionError("Soroban RefPython v1 requires a TransactionEnvelope v1")
        if len(transaction.operations) != 1:
            raise TransactionError("Soroban RefPython v1 requires exactly one operation")

        operation = transaction.operations[0]
        if not isinstance(operation, InvokeHostFunction):
            raise TransactionError(
                "Soroban RefPython v1 supports only InvokeHostFunction"
            )
        host_function = operation.host_function
        if host_function.type != xdr.HostFunctionType.HOST_FUNCTION_TYPE_INVOKE_CONTRACT:
            raise TransactionError(
                "Soroban RefPython v1 supports only invoke-contract host functions"
            )
        if host_function.invoke_contract is None:
            raise TransactionError("Invoke-contract host function is missing arguments")

        transaction_source = transaction.source.universal_account_id
        operation_source = (
            operation.source.universal_account_id
            if operation.source is not None
            else transaction_source
        )
        return operation, transaction_source, operation_source

    def _authorization_entry_facts(
        self,
        entry,
        operation_source: str,
        index: int,
    ) -> SorobanAuthorizationEntryFacts:
        authorizer, credential_type = _authorization_identity(
            entry.credentials,
            operation_source,
        )
        credential = _address_credentials(entry.credentials)
        return SorobanAuthorizationEntryFacts(
            index=index,
            entry_xdr=entry.to_xdr(),
            authorizer=authorizer,
            credential_type=credential_type,
            nonce=int(credential.nonce.int64) if credential is not None else None,
            signature_expiration_ledger=(
                int(credential.signature_expiration_ledger.uint32)
                if credential is not None
                else None
            ),
            root_invocation_xdr=entry.root_invocation.to_xdr(),
        )


def _authorization_identity(credentials, operation_source: str) -> tuple[str, str]:
    credential_type = credentials.type
    if credential_type == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_SOURCE_ACCOUNT:
        return operation_source, "source-account"
    if credential_type == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS:
        assert credentials.address is not None
        return Address.from_xdr_sc_address(credentials.address.address).address, "address"
    if credential_type == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_V2:
        assert credentials.address_v2 is not None
        return Address.from_xdr_sc_address(credentials.address_v2.address).address, "address-v2"
    if (
        credential_type
        == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_WITH_DELEGATES
    ):
        assert credentials.address_with_delegates is not None
        return (
            Address.from_xdr_sc_address(
                credentials.address_with_delegates.address_credentials.address
            ).address,
            "address-with-delegates",
        )
    raise TransactionError(f"Unsupported Soroban credential type: {credential_type}")


def _address_credentials(credentials):
    if credentials.type == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS:
        return credentials.address
    if credentials.type == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_V2:
        return credentials.address_v2
    if (
        credentials.type
        == xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_WITH_DELEGATES
    ):
        assert credentials.address_with_delegates is not None
        return credentials.address_with_delegates.address_credentials
    return None


def _integer(value) -> int:
    if value is None:
        return 0
    return int(value)
