import pytest
from stellar_sdk import Account as StellarAccount
from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Keypair, TransactionBuilder

from fresnica.errors import SignerError
from fresnica.network import TESTNET
from fresnica.signer import ExternalEd25519Signer
from fresnica.wallet import Account, Wallet


def _envelope(source):
    destination = Keypair.random().public_key
    return (
        TransactionBuilder(
            source_account=StellarAccount(source.public_key, 1),
            network_passphrase=TESTNET.passphrase,
            base_fee=100,
        )
        .append_payment_op(
            destination=destination,
            asset=StellarAsset.native(),
            amount="1",
        )
        .set_timeout(30)
        .build()
    )


def test_external_signer_receives_public_transaction_material_and_adds_verified_signature():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    unsigned_xdr = envelope.to_xdr()
    tx_hash = envelope.hash()
    requests = []

    def provider(request):
        requests.append(request)
        return keypair.sign(request.transaction_hash)

    wallet = Wallet.from_signer(ExternalEd25519Signer(keypair.public_key, provider))
    result = wallet.sign(envelope)

    assert result is envelope
    assert wallet.address() == keypair.public_key
    assert wallet.can_sign()
    assert len(requests) == 1
    request = requests[0]
    assert request.transaction_hash == tx_hash
    assert request.transaction_xdr == unsigned_xdr
    assert request.network_passphrase == TESTNET.passphrase
    assert envelope.signatures == [keypair.sign_decorated(tx_hash)]


def test_external_signer_rejects_wrong_key_signature_before_mutating_envelope():
    expected = Keypair.random()
    wrong = Keypair.random()
    envelope = _envelope(expected)
    signer = ExternalEd25519Signer(
        expected.public_key,
        lambda request: wrong.sign(request.transaction_hash),
    )

    with pytest.raises(SignerError, match="wrong key or payload"):
        signer.sign(envelope)

    assert envelope.signatures == []


def test_external_signer_requires_exact_ed25519_signature_shape():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    signer = ExternalEd25519Signer(keypair.public_key, lambda request: b"short")

    with pytest.raises(SignerError, match="64-byte"):
        signer.sign(envelope)

    assert envelope.signatures == []


def test_external_signer_wraps_transport_failure_without_mutating_envelope():
    keypair = Keypair.random()
    envelope = _envelope(keypair)

    def fail(request):
        raise OSError("device disconnected")

    signer = ExternalEd25519Signer(keypair.public_key, fail)

    with pytest.raises(SignerError, match="External signer failed") as captured:
        signer.sign(envelope)

    assert isinstance(captured.value.__cause__, OSError)
    assert envelope.signatures == []


def test_external_signer_rejects_duplicate_signature():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    signer = ExternalEd25519Signer(
        keypair.public_key,
        lambda request: keypair.sign(request.transaction_hash),
    )

    signer.sign(envelope)
    with pytest.raises(SignerError, match="already signed"):
        signer.sign(envelope)

    assert len(envelope.signatures) == 1


def test_wallet_rejects_signer_for_different_account():
    account_key = Keypair.random()
    signer_key = Keypair.random()
    signer = ExternalEd25519Signer(
        signer_key.public_key,
        lambda request: signer_key.sign(request.transaction_hash),
    )

    with pytest.raises(ValueError, match="does not match"):
        Wallet(
            Account(0, account_key.public_key, account_key.public_key),
            signer,
        )
