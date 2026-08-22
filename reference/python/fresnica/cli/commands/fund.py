"""Fund the active testnet wallet with Stellar Friendbot."""

from ...errors import NetworkError
from ..context import require_wallet_network


def execute_fund(runtime, args, renderer):
    if runtime.network != "testnet":
        raise NetworkError("Friendbot is only available on testnet")

    manager = runtime.wallet_manager
    record = manager.get_record(args.wallet)
    require_wallet_network(record, runtime.network)

    services = runtime.services_for()
    if services.testnet_service is None:
        raise NetworkError("Friendbot is unavailable for the current network")

    result = services.testnet_service.fund(record.address)
    tx_hash = result.get("hash") if isinstance(result, dict) else None
    message = f'Funded wallet "{record.name}" on testnet'
    if tx_hash:
        message += f"; transaction {tx_hash}"
    renderer.success(message)
    return result
