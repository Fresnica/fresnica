"""Opt-in physical Ledger proof against Stellar Testnet.

This test never asks for or stores the Ledger seed/secret. It uses the normal
SEP-5 account path by default, funds that public key on Testnet when needed,
asks the physical Ledger to review/sign one 1 XLM payment, has Fresnica
SDK/Core verify+apply the signature, and submits the resulting envelope.

Run from ``reference/python`` on the Mac connected to the Ledger:

    uv pip install -r requirements-ledger.txt
    FRESNICA_LIVE_LEDGER=1 \
    FRESNICA_PROCESS_BIN=../../bindings/process/target/release/fresnica-process \
    uv run --no-sync pytest -q -s tests/test_ledger_testnet_live.py

Optional derivation path override:

    FRESNICA_LEDGER_PATH="m/44'/148'/1'"
"""

import os

import pytest
from stellar_sdk import Asset, Keypair, TransactionBuilder

from fresnica.friendbot import FriendbotService
from fresnica.ledger_stellar import DEFAULT_LEDGER_STELLAR_PATH, LedgerStellarProvider
from fresnica.network import TESTNET
from fresnica.process_client import FresnicaProcessClient
from fresnica.signer import FresnicaProcessExternalEd25519Signer
from fresnica.stellar_adapter import StellarAdapter
from fresnica.wallet import Wallet


pytestmark = pytest.mark.skipif(
    os.environ.get("FRESNICA_LIVE_LEDGER") != "1",
    reason="set FRESNICA_LIVE_LEDGER=1 to run the physical Ledger Testnet proof",
)


def _fund_if_needed(adapter, friendbot, address):
    if adapter.account_exists(address):
        return
    response = friendbot.fund(address)
    assert response.get("hash")


def test_ledger_signs_core_verified_testnet_payment():
    process_binary = os.environ.get("FRESNICA_PROCESS_BIN")
    if not process_binary:
        pytest.fail("FRESNICA_PROCESS_BIN must point to the built fresnica-process binary")

    path = os.environ.get("FRESNICA_LEDGER_PATH", DEFAULT_LEDGER_STELLAR_PATH)
    core = FresnicaProcessClient(process_binary)
    adapter = StellarAdapter(TESTNET)
    friendbot = FriendbotService()

    print("\nLedger: unlock the device, open the Stellar app, and close Ledger Live.")
    with LedgerStellarProvider(path=path) as ledger:
        config = ledger.get_configuration()
        if config.blind_signing_enabled:
            pytest.fail(
                "Disable Blind Signing in the Ledger Stellar app before this clear-signing proof"
            )
        address = ledger.get_public_key(confirm_on_device=True)
        print(f"Ledger Stellar app: {config.version}")
        print(f"Derivation path: {path}")
        print(f"Testnet source: {address}")

        _fund_if_needed(adapter, friendbot, address)
        destination = Keypair.random()
        _fund_if_needed(adapter, friendbot, destination.public_key)
        print(f"Testnet destination: {destination.public_key}")
        print("Expected Ledger review: 1 XLM and memo fresnica-ledger-proof.")

        source_account = adapter.server.load_account(address)
        envelope = (
            TransactionBuilder(
                source_account=source_account,
                network_passphrase=TESTNET.passphrase,
                base_fee=adapter.fetch_base_fee(),
            )
            .append_payment_op(
                destination=destination.public_key,
                asset=Asset.native(),
                amount="1",
            )
            .add_text_memo("fresnica-ledger-proof")
            .set_timeout(60)
            .build()
        )

        signer = FresnicaProcessExternalEd25519Signer(
            address,
            core,
            ledger.sign_request,
        )
        wallet = Wallet.from_signer(signer)

        print("Approve the 1 XLM Testnet payment on the Ledger when prompted.")
        wallet.sign(envelope)
        response = adapter.submit_transaction(envelope)

    assert response.get("successful", True)
    assert response.get("hash")
    print(f"Submitted Testnet transaction: {response['hash']}")
