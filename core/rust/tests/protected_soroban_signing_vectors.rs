use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    derive_verified_unlock_key, parse_transaction_envelope_xdr, protect_secret_signing_material,
    sign_protected_transaction_envelope, transaction_envelope_has_valid_signature,
    transaction_envelope_xdr, transaction_hash, ProtectionRegistry,
};
use serde_json::Value;
use stellar_strkey::{ed25519::PublicKey as StrkeyPublicKey, Contract as StrkeyContract};
use stellar_xdr::{ContractId, Hash, HostFunction, MuxedAccount, OperationBody, ScAddress, TransactionEnvelope};

fn vector() -> Value {
    serde_json::from_str(include_str!(
        "../../../spec/test-vectors/protected-soroban-signing-v1.json"
    ))
    .unwrap()
}

fn signing_vector() -> Value {
    serde_json::from_str(include_str!(
        "../../../spec/test-vectors/transaction-signing-v1.json"
    ))
    .unwrap()
}

fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn protected_signer_matches_live_blend_testnet_transaction() {
    let fixture = vector();
    let signer_fixture = signing_vector();
    let signer_case = &signer_fixture["cases"][0];

    let unsigned = STANDARD
        .decode(
            fixture["transaction"]["unsigned_xdr_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let expected_signed = STANDARD
        .decode(
            fixture["transaction"]["expected_signed_xdr_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let network = fixture["network_passphrase"].as_str().unwrap();
    let wrong_network = fixture["wrong_network_passphrase"].as_str().unwrap();
    let expected_public = fixture["source_account"].as_str().unwrap();
    let secret = signer_case["secret"].as_str().unwrap();

    let mut envelope = parse_transaction_envelope_xdr(&unsigned).unwrap();
    let TransactionEnvelope::Tx(transaction) = &envelope else {
        panic!("Blend fixture must be a TransactionV1Envelope");
    };

    let expected_public_bytes = StrkeyPublicKey::from_string(expected_public).unwrap().0;
    match &transaction.tx.source_account {
        MuxedAccount::Ed25519(value) => assert_eq!(value.0, expected_public_bytes),
        other => panic!("unexpected Blend fixture source account: {other:?}"),
    }

    assert_eq!(transaction.tx.operations.len(), 1);
    let OperationBody::InvokeHostFunction(invoke) = &transaction.tx.operations[0].body else {
        panic!("Blend fixture must contain InvokeHostFunction");
    };
    let HostFunction::InvokeContract(call) = &invoke.host_function else {
        panic!("Blend fixture must invoke a contract");
    };
    assert_eq!(
        call.function_name.to_utf8_string_lossy(),
        fixture["blend"]["function"].as_str().unwrap()
    );
    let expected_contract =
        StrkeyContract::from_string(fixture["blend"]["contract_id"].as_str().unwrap())
            .unwrap()
            .0;
    match &call.contract_address {
        ScAddress::Contract(ContractId(Hash(value))) => assert_eq!(*value, expected_contract),
        other => panic!("unexpected Blend fixture contract address: {other:?}"),
    }

    let expected_testnet_hash = decode_hex(
        fixture["transaction"]["testnet_transaction_hash_hex"]
            .as_str()
            .unwrap(),
    );
    let expected_wrong_network_hash = decode_hex(
        fixture["transaction"]["wrong_network_transaction_hash_hex"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(
        transaction_hash(&envelope, network).unwrap().as_slice(),
        expected_testnet_hash.as_slice()
    );
    assert_eq!(
        transaction_hash(&envelope, wrong_network).unwrap().as_slice(),
        expected_wrong_network_hash.as_slice()
    );
    assert_ne!(expected_testnet_hash, expected_wrong_network_hash);

    let registry = ProtectionRegistry::new();
    let protected = protect_secret_signing_material(&registry, secret, "test-passphrase").unwrap();
    assert_eq!(protected.public_key, expected_public);
    let unlock_key = derive_verified_unlock_key(
        &registry,
        &protected.envelope,
        "test-passphrase",
        expected_public,
    )
    .unwrap();

    sign_protected_transaction_envelope(
        &registry,
        &protected.envelope,
        &unlock_key,
        expected_public,
        &mut envelope,
        network,
    )
    .unwrap();

    assert_eq!(transaction_envelope_xdr(&envelope).unwrap(), expected_signed);
    assert!(
        transaction_envelope_has_valid_signature(&envelope, network, expected_public).unwrap()
    );
    assert!(
        !transaction_envelope_has_valid_signature(&envelope, wrong_network, expected_public)
            .unwrap()
    );
}

#[test]
fn protected_signer_rejects_wrong_expected_identity_for_blend_transaction() {
    let fixture = vector();
    let signer_fixture = signing_vector();
    let signer_case = &signer_fixture["cases"][0];
    let unsigned = STANDARD
        .decode(
            fixture["transaction"]["unsigned_xdr_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

    let registry = ProtectionRegistry::new();
    let protected = protect_secret_signing_material(
        &registry,
        signer_case["secret"].as_str().unwrap(),
        "test-passphrase",
    )
    .unwrap();

    let error = derive_verified_unlock_key(
        &registry,
        &protected.envelope,
        "test-passphrase",
        fixture["wrong_signer_public_key"].as_str().unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "decrypted wallet identity does not match metadata");

    let envelope = parse_transaction_envelope_xdr(&unsigned).unwrap();
    assert_eq!(
        transaction_envelope_xdr(&envelope).unwrap(),
        unsigned,
        "identity rejection must not mutate the original transaction"
    );
}
