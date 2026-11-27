use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_native_sdk::{FresnicaSdkApi, NativeSdkError};
use serde_json::Value;

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
fn native_binding_signs_live_blend_transaction_without_rewriting_it() {
    let fixture = vector();
    let signer_fixture = signing_vector();
    let signer_case = &signer_fixture["cases"][0];
    let public_key = fixture["source_account"].as_str().unwrap();
    let network = fixture["network_passphrase"].as_str().unwrap();
    let wrong_network = fixture["wrong_network_passphrase"].as_str().unwrap();
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

    let api = FresnicaSdkApi::new();
    let protected = api
        .protect_secret(
            signer_case["secret"].as_str().unwrap().to_owned(),
            "test-passphrase".to_owned(),
            Some(public_key.to_owned()),
        )
        .unwrap();

    let testnet_request = api
        .prepare_ed25519_signing(unsigned.clone(), network.to_owned())
        .unwrap();
    assert_eq!(
        testnet_request.transaction_hash,
        decode_hex(
            fixture["transaction"]["testnet_transaction_hash_hex"]
                .as_str()
                .unwrap()
        )
    );
    assert_eq!(testnet_request.transaction_xdr, unsigned);

    let wrong_network_request = api
        .prepare_ed25519_signing(
            testnet_request.transaction_xdr.clone(),
            wrong_network.to_owned(),
        )
        .unwrap();
    assert_eq!(
        wrong_network_request.transaction_hash,
        decode_hex(
            fixture["transaction"]["wrong_network_transaction_hash_hex"]
                .as_str()
                .unwrap()
        )
    );
    assert_ne!(
        testnet_request.transaction_hash,
        wrong_network_request.transaction_hash
    );

    let unlock_key = api
        .derive_unlock_key(
            protected.envelope_json.clone(),
            "test-passphrase".to_owned(),
            public_key.to_owned(),
        )
        .unwrap();
    let signed = api
        .sign_transaction_xdr(
            protected.envelope_json.clone(),
            unlock_key,
            public_key.to_owned(),
            testnet_request.transaction_xdr.clone(),
            network.to_owned(),
        )
        .unwrap();
    assert_eq!(signed, expected_signed);

    let wrong_identity = api.derive_unlock_key(
        protected.envelope_json,
        "test-passphrase".to_owned(),
        fixture["wrong_signer_public_key"]
            .as_str()
            .unwrap()
            .to_owned(),
    );
    assert!(matches!(
        wrong_identity,
        Err(NativeSdkError::IdentityMismatch { .. })
    ));
}

#[test]
fn native_binding_wrong_network_signature_does_not_validate_as_testnet() {
    let fixture = vector();
    let signer_fixture = signing_vector();
    let signer_case = &signer_fixture["cases"][0];
    let public_key = fixture["source_account"].as_str().unwrap();
    let unsigned = STANDARD
        .decode(
            fixture["transaction"]["unsigned_xdr_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();

    let api = FresnicaSdkApi::new();
    let protected = api
        .protect_secret(
            signer_case["secret"].as_str().unwrap().to_owned(),
            "test-passphrase".to_owned(),
            Some(public_key.to_owned()),
        )
        .unwrap();
    let unlock_key = api
        .derive_unlock_key(
            protected.envelope_json.clone(),
            "test-passphrase".to_owned(),
            public_key.to_owned(),
        )
        .unwrap();
    let signed_wrong_network = api
        .sign_transaction_xdr(
            protected.envelope_json,
            unlock_key,
            public_key.to_owned(),
            unsigned,
            fixture["wrong_network_passphrase"]
                .as_str()
                .unwrap()
                .to_owned(),
        )
        .unwrap();

    assert_ne!(
        STANDARD.encode(signed_wrong_network),
        fixture["transaction"]["expected_signed_xdr_base64"]
            .as_str()
            .unwrap()
    );
}
