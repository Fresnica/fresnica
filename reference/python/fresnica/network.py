"""Network context for Fresnica.

Network configuration is separate from wallet identity.
The same account can exist on different Stellar networks.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class Network:
    name: str
    passphrase: str
    horizon_url: str


MAINNET = Network(
    name="mainnet",
    passphrase="Public Global Stellar Network ; September 2015",
    horizon_url="https://horizon.stellar.org",
)

TESTNET = Network(
    name="testnet",
    passphrase="Test SDF Network ; September 2015",
    horizon_url="https://horizon-testnet.stellar.org",
)
