"""Official stellar-sdk backed Soroban RPC/assembly adapter for RefPython."""

from stellar_sdk import Address, SorobanServer, xdr
from stellar_sdk.exceptions import PrepareTransactionException, SdkError
from stellar_sdk.operation import InvokeHostFunction

from .errors import NetworkError, TransactionError
from .soroban_service import SorobanEnvelopeFacts, SorobanSimulationResult


class SorobanRpcAdapter:
    """Narrow provider adapter; shared RPC abstractions wait for RefPython evidence."""

    def __init__(self, network, rpc_url: str):
        if not rpc_url:
            raise ValueError("rpc_url is required")
        self.network = network
        self.rpc_url = rpc_url
        self.server = SorobanServer(server_url=rpc_url)

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
        invoke = host_function.invoke_contract
        if invoke is None:
            raise TransactionError("Invoke-contract host function is missing arguments")

        transaction_source = transaction.source.universal_account_id
        operation_source = (
            operation.source.universal_account_id
            if operation.source is not None
            else transaction_source
        )
        authorizers: list[str] = []
        credential_types: list[str] = []
        for auth in operation.auth:
            authorizer, credential_type = _authorization_identity(
                auth.credentials,
                operation_source,
            )
            authorizers.append(authorizer)
            credential_types.append(credential_type)

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


def _integer(value) -> int:
    if value is None:
        return 0
    return int(value)
