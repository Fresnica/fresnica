"""Stellar network configuration.

Network identity is separate from wallet identity. The same Stellar public key
can be used on more than one network.
"""

from dataclasses import dataclass

from .errors import NetworkError


@dataclass(frozen=True)
class Network:
    name: str
    passphrase: str
    horizon_url: str
    explorer_network: str


MAINNET = Network(
    name="mainnet",
    passphrase="Public Global Stellar Network ; September 2015",
    horizon_url="https://horizon.stellar.org",
    explorer_network="public",
)

TESTNET = Network(
    name="testnet",
    passphrase="Test SDF Network ; September 2015",
    horizon_url="https://horizon-testnet.stellar.org",
    explorer_network="testnet",
)

NETWORKS = {
    MAINNET.name: MAINNET,
    TESTNET.name: TESTNET,
}


def get_network(name: str) -> Network:
    try:
        return NETWORKS[name.lower()]
    except KeyError as exc:
        raise NetworkError(f"Unknown Stellar network: {name}") from exc
