"""Opt-in live Testnet evidence for the RefPython Soroban lifecycle."""

import os

import pytest
from stellar_sdk import Asset, Keypair, TransactionBuilder, scval

from fresnica.friendbot import FriendbotService
from fresnica.manager import WalletManager
from fresnica.network import TESTNET
from fresnica.process_client import FresnicaProcessClient
from fresnica.soroban_rpc_adapter import SorobanRpcAdapter
from fresnica.soroban_service import (
    SorobanAuthorizationService,
    SorobanSimulationService,
    SorobanSubmitService,
)
from fresnica.storage import MemoryWalletStorage
from fresnica.transaction_service import TransactionService


pytestmark = pytest.mark.skipif(
    os.environ.get("FRESNICA_LIVE_TESTNET") != "1",
    reason="set FRESNICA_LIVE_TESTNET=1 to run disposable Testnet probes",
)

RPC_URL = os.environ.get(
    "FRESNICA_SOROBAN_RPC_URL",
    "https://soroban-testnet.stellar.org:443",
)
PASSPHRASE = "fresnica-soroban-live-passcode"


def _protected_session(core, name: str, keypair: Keypair):
    manager = WalletManager(MemoryWalletStorage(), core_client=core)
    manager.import_secret(
        name,
        keypair.secret,
        PASSPHRASE,
        network="testnet",
    )
    return manager.unlock(name, PASSPHRASE)


def _candidate(adapter, source: str, from_address: str, to_address: str):
    source_account = adapter.load_account(source)
    return (
        TransactionBuilder(
            source_account,
            TESTNET.passphrase,
            base_fee=100,
        )
        .set_timeout(300)
        .append_invoke_contract_function_op(
            contract_id=Asset.native().contract_id(TESTNET.passphrase),
            function_name="transfer",
            parameters=[
                scval.to_address(from_address),
                scval.to_address(to_address),
                scval.to_int128(1),
            ],
        )
        .build()
    )


def _prepare_authorize_sign_submit(
    adapter,
    fee_payer_session,
    candidate,
    signers_by_authorizer,
):
    prepared = SorobanSimulationService(adapter).prepare(
        fee_payer_session.record.name,
        candidate,
    )
    reviewed_hash = prepared.review.transaction_hash
    SorobanAuthorizationService(adapter).authorize(
        prepared,
        signers_by_authorizer,
    )
    transaction = TransactionService(
        SorobanSubmitService(adapter, max_attempts=30, sleep_seconds=1.0)
    )
    transaction.sign(fee_payer_session.wallet, prepared)
    result = transaction.submit(prepared)
    assert result.successful is True
    assert result.hash == prepared.signing_transaction_hash
    assert result.ledger is not None
    return prepared, reviewed_hash


def test_source_and_detached_g_authorization_submit_on_testnet():
    binary = os.environ.get("FRESNICA_PROCESS_BIN")
    if not binary:
        pytest.skip("FRESNICA_PROCESS_BIN is required for protected Soroban signing")

    core = FresnicaProcessClient(binary)
    first_key = Keypair.random()
    second_key = Keypair.random()
    friendbot = FriendbotService()
    friendbot.fund(first_key.public_key)
    friendbot.fund(second_key.public_key)

    first = _protected_session(core, "fee-payer", first_key)
    second = _protected_session(core, "detached-authorizer", second_key)
    adapter = SorobanRpcAdapter(TESTNET, RPC_URL)

    source_prepared, source_reviewed_hash = _prepare_authorize_sign_submit(
        adapter,
        first,
        _candidate(
            adapter,
            first.record.address,
            first.record.address,
            second.record.address,
        ),
        {},
    )
    assert source_prepared.review.credential_types == ("source-account",)
    assert source_prepared.signing_transaction_hash == source_reviewed_hash

    detached_prepared, detached_reviewed_hash = _prepare_authorize_sign_submit(
        adapter,
        first,
        _candidate(
            adapter,
            first.record.address,
            second.record.address,
            first.record.address,
        ),
        {second.record.address: second.wallet.signer},
    )
    assert detached_prepared.review.fee_payer == first.record.address
    assert detached_prepared.review.authorizers == (second.record.address,)
    assert detached_prepared.review.credential_types[0] in {"address", "address-v2"}
    assert detached_prepared.signing_transaction_hash != detached_reviewed_hash
