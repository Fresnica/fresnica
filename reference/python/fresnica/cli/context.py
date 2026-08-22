"""CLI guards shared by one-shot commands."""

from ..errors import NetworkError


def require_wallet_network(record, network: str) -> None:
    if record.network != network:
        raise NetworkError(
            f'Wallet "{record.name}" is configured for {record.network}; '
            f"use --network {record.network}"
        )
