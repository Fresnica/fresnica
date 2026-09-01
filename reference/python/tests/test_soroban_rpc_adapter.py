import json
from pathlib import Path
from types import SimpleNamespace

import pytest
from stellar_sdk import Account, Address, Keypair, TransactionBuilder, xdr
from stellar_sdk.soroban_rpc import GetTransactionStatus, SendTransactionStatus

from fresnica.errors import TransactionError
from fresnica.network import TESTNET
from fresnica.soroban_rpc_adapter import SorobanRpcAdapter, _authorization_identity


SOROBAN_VECTOR_PATH = (
    Path(__file__).parents[3]
    / "spec"
    / "test-vectors"
    / "soroban-authorization-signing-v1.json"
)


class FakeSorobanServer:
    def __init__(self):
        self.simulated = None
        self.use_upgraded_auth = None
        self.prepared = None
        self.simulation_response = SimpleNamespace(
            error=None,
            restore_preamble=None,
            min_resource_fee="4900",
            latest_ledger=123456,
        )
        self.prepared_envelope = object()

    def simulate_transaction(self, envelope, use_upgraded_auth=False):
        self.simulated = envelope
        self.use_upgraded_auth = use_upgraded_auth
        return self.simulation_response

    def prepare_transaction(self, envelope, simulate_transaction_response=None):
        self.prepared = (envelope, simulate_transaction_response)
        return self.prepared_envelope

    def send_transaction(self, envelope):
        return SimpleNamespace(
            hash=envelope.hash_hex(),
            status=SendTransactionStatus.PENDING,
            latest_ledger=123456,
            error_result_xdr=None,
            diagnostic_events_xdr=None,
        )

    def get_transaction(self, tx_hash):
        return SimpleNamespace(
            status=GetTransactionStatus.SUCCESS,
            transaction_hash=tx_hash,
            ledger=123457,
            result_xdr="result-xdr",
            result_meta_xdr="meta-xdr",
            envelope_xdr="envelope-xdr",
            diagnostic_events_xdr=None,
        )


def build_candidate():
    source = Keypair.random()
    contract_id = Address.from_raw_contract(bytes(range(32))).address
    envelope = (
        TransactionBuilder(
            Account(source.public_key, 1),
            TESTNET.passphrase,
            base_fee=100,
        )
        .set_timeout(300)
        .append_invoke_contract_function_op(
            contract_id=contract_id,
            function_name="hello",
            parameters=[],
        )
        .build()
    )
    return source, contract_id, envelope


def test_adapter_inspects_unsigned_invoke_contract_candidate_from_xdr_objects():
    source, contract_id, envelope = build_candidate()
    adapter = SorobanRpcAdapter(TESTNET, "https://rpc.example")

    facts = adapter.inspect_envelope(envelope)

    assert facts.transaction_source == source.public_key
    assert facts.operation_source == source.public_key
    assert facts.contract_id == contract_id
    assert facts.function_name == "hello"
    assert facts.argument_count == 0
    assert facts.auth_entry_count == 0
    assert facts.authorizers == ()
    assert facts.credential_types == ()
    assert facts.total_fee_stroops == 100
    assert facts.resource_fee_stroops == 0
    assert facts.has_soroban_data is False
    assert facts.signature_count == 0
    assert facts.invocation_xdr == envelope.transaction.operations[0].host_function.to_xdr()


def test_adapter_requests_upgraded_address_v2_auth_and_reuses_simulation_for_assembly():
    _, _, envelope = build_candidate()
    adapter = SorobanRpcAdapter(TESTNET, "https://rpc.example")
    fake_server = FakeSorobanServer()
    adapter.server = fake_server

    simulation = adapter.simulate_transaction(envelope)
    assembled = adapter.prepare_transaction(envelope, simulation.raw_response)

    assert fake_server.simulated is envelope
    assert fake_server.use_upgraded_auth is True
    assert simulation.error is None
    assert simulation.restore_required is False
    assert simulation.min_resource_fee == 4900
    assert simulation.latest_ledger == 123456
    assert fake_server.prepared == (envelope, fake_server.simulation_response)
    assert assembled is fake_server.prepared_envelope


def test_adapter_rejects_network_mismatch_before_rpc_semantics():
    _, _, envelope = build_candidate()
    envelope.network_passphrase = "other network"
    adapter = SorobanRpcAdapter(TESTNET, "https://rpc.example")

    with pytest.raises(TransactionError, match="network does not match"):
        adapter.inspect_envelope(envelope)


@pytest.mark.parametrize(
    "credential_type, arm, expected_label",
    [
        (
            xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS,
            "address",
            "address",
        ),
        (
            xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_V2,
            "address_v2",
            "address-v2",
        ),
    ],
)
def test_address_authorization_credentials_preserve_authorizer_identity(
    credential_type,
    arm,
    expected_label,
):
    authorizer = Keypair.random().public_key
    address_credentials = SimpleNamespace(
        address=Address(authorizer).to_xdr_sc_address(),
    )
    credentials = SimpleNamespace(
        type=credential_type,
        address=address_credentials if arm == "address" else None,
        address_v2=address_credentials if arm == "address_v2" else None,
        address_with_delegates=None,
    )

    assert _authorization_identity(credentials, "GOPSOURCE") == (
        authorizer,
        expected_label,
    )


def test_source_account_authorization_is_bound_to_operation_source():
    credentials = SimpleNamespace(
        type=xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_SOURCE_ACCOUNT,
        address=None,
        address_v2=None,
        address_with_delegates=None,
    )

    assert _authorization_identity(credentials, "GOPSOURCE") == (
        "GOPSOURCE",
        "source-account",
    )


def test_address_with_delegates_keeps_top_level_authorizer_distinct_from_delegates():
    authorizer = Keypair.random().public_key
    credentials = SimpleNamespace(
        type=xdr.SorobanCredentialsType.SOROBAN_CREDENTIALS_ADDRESS_WITH_DELEGATES,
        address=None,
        address_v2=None,
        address_with_delegates=SimpleNamespace(
            address_credentials=SimpleNamespace(
                address=Address(authorizer).to_xdr_sc_address(),
            )
        ),
    )

    assert _authorization_identity(credentials, "GOPSOURCE") == (
        authorizer,
        "address-with-delegates",
    )

def test_adapter_inspects_shared_address_v2_authorization_vector():
    vector = json.loads(SOROBAN_VECTOR_PATH.read_text(encoding="utf-8"))["cases"][0]
    adapter = SorobanRpcAdapter(TESTNET, "https://rpc.example")

    facts = adapter.inspect_authorization_entry_xdr(
        vector["unsigned_entry_xdr_base64"],
        "GOPSOURCE",
        index=0,
    )

    assert facts.authorizer == vector["public_key"]
    assert facts.credential_type == "address-v2"
    assert facts.nonce == vector["nonce"]
    assert facts.signature_expiration_ledger == vector["signature_expiration_ledger"]
    assert facts.entry_xdr == vector["unsigned_entry_xdr_base64"]


def test_adapter_normalizes_rpc_submission_and_terminal_lookup():
    _, _, envelope = build_candidate()
    adapter = SorobanRpcAdapter(TESTNET, "https://rpc.example")
    adapter.server = FakeSorobanServer()

    sent = adapter.send_transaction(envelope)
    terminal = adapter.lookup_transaction(envelope.hash_hex())

    assert sent["hash"] == envelope.hash_hex()
    assert sent["status"] == "PENDING"
    assert terminal == {
        "hash": envelope.hash_hex(),
        "ledger": 123457,
        "successful": True,
        "status": "SUCCESS",
        "result_xdr": "result-xdr",
        "result_meta_xdr": "meta-xdr",
        "envelope_xdr": "envelope-xdr",
        "diagnostic_events_xdr": None,
    }
