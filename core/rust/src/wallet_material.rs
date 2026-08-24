use serde_json::{json, Value};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    derive_classic_public_key, detect_mnemonic_language, generate_mnemonic_phrase, ClassicSigner,
    ProtectionCredential, ProtectionError, ProtectionRegistry, SignerError, SoftwareSigner,
    WalletDerivationError,
};

pub struct ProtectedWalletMaterial {
    pub public_key: String,
    pub envelope: Value,
}

pub struct GeneratedProtectedMnemonic {
    pub wallet: ProtectedWalletMaterial,
    pub mnemonic: Zeroizing<String>,
}

pub fn protect_secret_signing_material(
    registry: &ProtectionRegistry,
    secret: &str,
    passcode: &str,
) -> Result<ProtectedWalletMaterial, WalletMaterialError> {
    let secret = Zeroizing::new(secret.trim().to_owned());
    let signer = SoftwareSigner::from_secret(&secret)?;
    let public_key = signer.public_key().to_owned();
    drop(signer);

    let mut payload = json!({
        "kind": "secret",
        "secret": secret.as_str(),
    });
    let envelope = protect_and_zeroize_payload(registry, &mut payload, passcode)?;
    Ok(ProtectedWalletMaterial {
        public_key,
        envelope,
    })
}

pub fn protect_mnemonic_signing_material(
    registry: &ProtectionRegistry,
    mnemonic: &str,
    mnemonic_passphrase: &str,
    index: usize,
    language: Option<&str>,
    passcode: &str,
) -> Result<ProtectedWalletMaterial, WalletMaterialError> {
    let mnemonic = Zeroizing::new(mnemonic.trim().to_owned());
    let mnemonic_passphrase = Zeroizing::new(mnemonic_passphrase.to_owned());
    let language = match language {
        Some(language) if !language.is_empty() => language.to_owned(),
        _ => detect_mnemonic_language(&mnemonic)?.to_owned(),
    };
    let public_key = derive_classic_public_key(
        &mnemonic,
        &mnemonic_passphrase,
        index,
        &language,
    )?;

    let mut payload = json!({
        "kind": "mnemonic",
        "mnemonic": mnemonic.as_str(),
        "mnemonic_passphrase": mnemonic_passphrase.as_str(),
        "index": index,
        "language": language,
    });
    let envelope = protect_and_zeroize_payload(registry, &mut payload, passcode)?;
    Ok(ProtectedWalletMaterial {
        public_key,
        envelope,
    })
}

pub fn generate_protected_mnemonic(
    registry: &ProtectionRegistry,
    language: &str,
    strength: usize,
    mnemonic_passphrase: &str,
    index: usize,
    passcode: &str,
) -> Result<GeneratedProtectedMnemonic, WalletMaterialError> {
    let mnemonic = generate_mnemonic_phrase(language, strength)?;
    let wallet = protect_mnemonic_signing_material(
        registry,
        &mnemonic,
        mnemonic_passphrase,
        index,
        Some(language),
        passcode,
    )?;
    Ok(GeneratedProtectedMnemonic { wallet, mnemonic })
}

fn protect_and_zeroize_payload(
    registry: &ProtectionRegistry,
    payload: &mut Value,
    passcode: &str,
) -> Result<Value, ProtectionError> {
    let credential = ProtectionCredential::password(passcode);
    let result = registry.protect(payload, &credential);
    zeroize_sensitive_payload(payload);
    result
}

fn zeroize_sensitive_payload(payload: &mut Value) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    for key in ["secret", "mnemonic", "mnemonic_passphrase"] {
        if let Some(Value::String(mut value)) = object.remove(key) {
            value.zeroize();
        }
    }
}

#[derive(Debug, Error)]
pub enum WalletMaterialError {
    #[error(transparent)]
    Protection(#[from] ProtectionError),
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error(transparent)]
    Derivation(#[from] WalletDerivationError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{derive_verified_unlock_key, export_signing_material, ExportedSigningMaterial};

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    #[test]
    fn protects_secret_and_preserves_identity() {
        let registry = ProtectionRegistry::new();
        let protected = protect_secret_signing_material(&registry, SECRET, "passcode").unwrap();

        assert_eq!(protected.public_key, PUBLIC);
        assert!(!protected.envelope.to_string().contains(SECRET));
        assert!(derive_verified_unlock_key(
            &registry,
            &protected.envelope,
            "passcode",
            PUBLIC,
        )
        .is_ok());
    }

    #[test]
    fn generates_and_exports_same_mnemonic() {
        let registry = ProtectionRegistry::new();
        let generated = generate_protected_mnemonic(
            &registry,
            "english",
            128,
            "",
            0,
            "passcode",
        )
        .unwrap();
        let expected = generated.mnemonic.to_string();
        let exported = export_signing_material(
            &registry,
            &generated.wallet.envelope,
            "passcode",
            &generated.wallet.public_key,
        )
        .unwrap();

        match exported {
            ExportedSigningMaterial::Mnemonic { mnemonic, .. } => {
                assert_eq!(mnemonic.as_str(), expected);
            }
            _ => panic!("generated wallet did not export mnemonic material"),
        }
    }
}
