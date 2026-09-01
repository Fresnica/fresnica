use fresnica_core::{derive_classic_public_key, derive_classic_signer, detect_mnemonic_language};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WalletVectors {
    schema: String,
    derivation: Vec<DerivationVector>,
}

#[derive(Debug, Deserialize)]
struct DerivationVector {
    name: String,
    language: String,
    mnemonic: String,
    passphrase: String,
    index: usize,
    expected_public_key: String,
}

#[test]
fn wallet_derivation_matches_cross_language_vectors() {
    let raw = include_str!("../../../spec/test-vectors/wallet-v1.json");
    let vectors: WalletVectors = serde_json::from_str(raw).unwrap();

    assert_eq!(vectors.schema, "fresnica-wallet-v1");
    assert!(!vectors.derivation.is_empty());

    for vector in vectors.derivation {
        let public_key = derive_classic_public_key(
            &vector.mnemonic,
            &vector.passphrase,
            vector.index,
            &vector.language,
        )
        .unwrap_or_else(|error| panic!("{} failed: {error}", vector.name));
        let signer = derive_classic_signer(
            &vector.mnemonic,
            &vector.passphrase,
            vector.index,
            &vector.language,
        )
        .unwrap_or_else(|error| panic!("{} signer failed: {error}", vector.name));

        assert_eq!(public_key, vector.expected_public_key, "{}", vector.name);
        assert_eq!(
            signer.public_key(),
            vector.expected_public_key,
            "{}",
            vector.name
        );

        if vector.language.starts_with("chinese_") {
            assert_eq!(
                detect_mnemonic_language(&vector.mnemonic).unwrap(),
                vector.language,
                "{}",
                vector.name
            );
        }
    }
}
