import os

import pytest
from stellar_sdk import Account, Keypair, Network, TransactionBuilder, TransactionEnvelope

from fresnica.rust_sdk_client import RustSdkClient
from fresnica.runtime import Runtime


@pytest.fixture(scope="module")
def sdk_client():
    binary = os.environ.get("FRESNICA_SDK_BIN")
    if not binary:
        pytest.skip("FRESNICA_SDK_BIN is not configured")
    return RustSdkClient(binary)


def _transaction(public_key: str):
    source = Account(public_key, 0)
    return (
        TransactionBuilder(
            source_account=source,
            network_passphrase=Network.TESTNET_NETWORK_PASSPHRASE,
            base_fee=100,
        )
        .append_manage_data_op(data_name="sdk-check", data_value=b"refpython")
        .set_timeout(60)
        .build()
    )


def test_runtime_prefers_sdk_bridge(tmp_path, monkeypatch, sdk_client):
    monkeypatch.setenv("FRESNICA_SDK_BIN", str(sdk_client.binary))
    runtime = Runtime(home=tmp_path)
    assert isinstance(runtime.core_client, RustSdkClient)


def test_sdk_bridge_version(sdk_client):
    version = sdk_client.version()
    assert version["sdk_api_version"] == 3
    assert version["client_api_version"] == 3


def test_mnemonic_hd_derivation_and_passcode_signing(sdk_client):
    generated = sdk_client.generate_mnemonic("correct horse battery staple", index=0)
    derived = sdk_client.derive_mnemonic_signer(
        generated.envelope,
        "correct horse battery staple",
        generated.signer_public_key,
        1,
    )
    assert derived.signer_public_key != generated.signer_public_key

    revealed = sdk_client.reveal(
        generated.envelope,
        "correct horse battery staple",
        generated.signer_public_key,
    )
    assert revealed["kind"] == "mnemonic"
    assert revealed["index"] == 0

    transaction = _transaction(generated.signer_public_key)
    signed_xdr = sdk_client.sign_transaction_with_passcode(
        generated.envelope,
        "correct horse battery staple",
        generated.signer_public_key,
        transaction.to_xdr(),
        Network.TESTNET_NETWORK_PASSPHRASE,
    )
    signed = TransactionEnvelope.from_xdr(
        signed_xdr, Network.TESTNET_NETWORK_PASSPHRASE
    )
    assert len(signed.signatures) == 1


def test_external_ed25519_prepare_apply_roundtrip(sdk_client):
    signer = Keypair.random()
    transaction = _transaction(signer.public_key)
    prepared = sdk_client.prepare_ed25519_signing(
        transaction.to_xdr(), Network.TESTNET_NETWORK_PASSPHRASE
    )
    signature = signer.sign(prepared.transaction_hash)
    signed_xdr = sdk_client.apply_ed25519_signature(
        prepared.transaction_xdr,
        prepared.network_passphrase,
        signer.public_key,
        signature,
    )
    signed = TransactionEnvelope.from_xdr(
        signed_xdr, Network.TESTNET_NETWORK_PASSPHRASE
    )
    assert len(signed.signatures) == 1
