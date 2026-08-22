"""Stellar account derivation.

Fresnica delegates Stellar key derivation and Keypair handling to the
official Stellar Python SDK.
"""

from stellar_sdk import Keypair


def derive_account(
    mnemonic: str,
    passphrase: str = "",
    index: int = 0,
) -> Keypair:
    """Create a Stellar Keypair from a mnemonic phrase.

    The SDK handles the Stellar mnemonic derivation path and Ed25519 key
    generation. Fresnica only exposes the wallet-level abstraction.
    """
    return Keypair.from_mnemonic_phrase(
        mnemonic,
        passphrase=passphrase,
        index=index,
    )
