from types import SimpleNamespace

import pytest
from stellar_sdk import Account as StellarAccount
from stellar_sdk import Asset as StellarAsset
from stellar_sdk import Keypair, TransactionBuilder, TransactionEnvelope

from fresnica.errors import SignerError
from fresnica.ledger_stellar import (
    LedgerStellarError,
    LedgerStellarProvider,
    LedgerStellarUserRejected,
    _pack_derivation_path,
)
from fresnica.network import TESTNET
from fresnica.signer import FresnicaProcessExternalEd25519Signer


class FakeTransport:
    def __init__(self, responses):
        self.responses = list(responses)
        self.calls = []
        self.closed = False

    def exchange(self, **kwargs):
        self.calls.append(kwargs)
        if not self.responses:
            raise AssertionError("unexpected Ledger exchange")
        return self.responses.pop(0)

    def close(self):
        self.closed = True


def _envelope(keypair):
    return (
        TransactionBuilder(
            source_account=StellarAccount(keypair.public_key, 7),
            network_passphrase=TESTNET.passphrase,
            base_fee=100,
        )
        .append_payment_op(
            destination=Keypair.random().public_key,
            asset=StellarAsset.native(),
            amount="1",
        )
        .set_timeout(30)
        .build()
    )


def test_pack_sep5_path_matches_ledger_bip32_wire_shape():
    assert _pack_derivation_path("m/44'/148'/0'") == bytes.fromhex(
        "038000002c8000009480000000"
    )


def test_provider_reads_configuration_and_public_key():
    keypair = Keypair.random()
    transport = FakeTransport(
        [
            (0x9000, bytes([0, 6, 0, 1, 0x20, 0x00])),
            (0x9000, keypair.raw_public_key()),
        ]
    )
    provider = LedgerStellarProvider(transport=transport)

    config = provider.get_configuration()
    public_key = provider.get_public_key()

    assert config.version == "6.0.1"
    assert config.blind_signing_enabled is False
    assert config.max_data_size == 8192
    assert public_key == keypair.public_key
    assert transport.calls[1]["cdata"] == bytes.fromhex(
        "038000002c8000009480000000"
    )


def test_provider_streams_signature_base_and_returns_device_signature():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    signature = keypair.sign(envelope.hash())
    transport = FakeTransport(
        [
            (0x9000, b""),
            (0x9000, signature),
        ]
    )
    provider = LedgerStellarProvider(transport=transport)
    request = SimpleNamespace(
        transaction_hash=envelope.hash(),
        transaction_xdr=envelope.to_xdr(),
        network_passphrase=TESTNET.passphrase,
    )

    result = provider.sign_request(request)

    assert result == signature
    assert transport.calls[0]["p1"] == 0x00
    assert transport.calls[0]["p2"] == 0x80
    assert transport.calls[1]["p1"] == 0x80
    assert transport.calls[1]["p2"] == 0x00
    assert transport.calls[1]["cdata"] == envelope.signature_base()


def test_provider_chunks_large_signature_base_like_official_stellar_app_protocol():
    transport = FakeTransport(
        [
            (0x9000, b""),
            (0x9000, b""),
            (0x9000, b""),
            (0x9000, b"s" * 64),
        ]
    )
    provider = LedgerStellarProvider(transport=transport)
    payload = b"x" * 600

    assert provider.sign_transaction_signature_base(payload) == b"s" * 64

    streamed = transport.calls[1:]
    assert [len(call["cdata"]) for call in streamed] == [255, 255, 90]
    assert [call["p2"] for call in streamed] == [0x80, 0x80, 0x00]


def test_provider_rejects_core_hash_xdr_mismatch_before_device_io():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    transport = FakeTransport([])
    provider = LedgerStellarProvider(transport=transport)
    request = SimpleNamespace(
        transaction_hash=b"x" * 32,
        transaction_xdr=envelope.to_xdr(),
        network_passphrase=TESTNET.passphrase,
    )

    with pytest.raises(LedgerStellarError, match="hash does not match"):
        provider.sign_request(request)

    assert transport.calls == []


def test_provider_maps_device_rejection():
    transport = FakeTransport([(0x6985, b"")])
    provider = LedgerStellarProvider(transport=transport)

    with pytest.raises(LedgerStellarUserRejected):
        provider.get_public_key(confirm_on_device=True)


def test_core_verified_external_signer_mutates_only_after_core_apply():
    keypair = Keypair.random()
    envelope = _envelope(keypair)
    unsigned_xdr = envelope.to_xdr()
    signature = keypair.sign(envelope.hash())
    calls = []

    class Core:
        def prepare_ed25519_signing(self, transaction_xdr, network_passphrase):
            calls.append(("prepare", transaction_xdr, network_passphrase))
            return SimpleNamespace(
                transaction_hash=envelope.hash(),
                transaction_xdr=transaction_xdr,
                network_passphrase=network_passphrase,
            )

        def apply_ed25519_signature(
            self,
            transaction_xdr,
            network_passphrase,
            signer_public_key,
            returned_signature,
        ):
            calls.append(("apply", signer_public_key, returned_signature))
            signed = TransactionEnvelope.from_xdr(transaction_xdr, network_passphrase)
            signed.sign(keypair)
            return signed.to_xdr()

    signer = FresnicaProcessExternalEd25519Signer(
        keypair.public_key,
        Core(),
        lambda request: signature,
    )

    signer.sign(envelope)

    assert calls[0] == ("prepare", unsigned_xdr, TESTNET.passphrase)
    assert calls[1] == ("apply", keypair.public_key, signature)
    assert len(envelope.signatures) == 1


def test_core_verified_external_signer_preserves_provider_rejection():
    keypair = Keypair.random()
    envelope = _envelope(keypair)

    class Core:
        def prepare_ed25519_signing(self, transaction_xdr, network_passphrase):
            return SimpleNamespace(
                transaction_hash=envelope.hash(),
                transaction_xdr=transaction_xdr,
                network_passphrase=network_passphrase,
            )

    def reject(_request):
        raise LedgerStellarUserRejected("rejected on device")

    signer = FresnicaProcessExternalEd25519Signer(
        keypair.public_key,
        Core(),
        reject,
    )

    with pytest.raises(LedgerStellarUserRejected, match="rejected on device"):
        signer.sign(envelope)

    assert envelope.signatures == []


def test_core_verified_external_signer_does_not_mutate_on_apply_failure():
    keypair = Keypair.random()
    envelope = _envelope(keypair)

    class Core:
        def prepare_ed25519_signing(self, transaction_xdr, network_passphrase):
            return SimpleNamespace(
                transaction_hash=envelope.hash(),
                transaction_xdr=transaction_xdr,
                network_passphrase=network_passphrase,
            )

        def apply_ed25519_signature(self, *args, **kwargs):
            raise SignerError("wrong signer")

    signer = FresnicaProcessExternalEd25519Signer(
        keypair.public_key,
        Core(),
        lambda request: keypair.sign(request.transaction_hash),
    )

    with pytest.raises(SignerError, match="wrong signer"):
        signer.sign(envelope)

    assert envelope.signatures == []
