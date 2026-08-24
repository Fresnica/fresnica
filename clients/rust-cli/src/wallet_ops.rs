use fresnica_core::{
    detect_mnemonic_language, export_signing_material, generate_protected_mnemonic,
    protect_mnemonic_signing_material, protect_secret_signing_material,
    ExportedSigningMaterial, ProtectionRegistry,
};
use serde_json::{Map, Number, Value};
use zeroize::Zeroizing;

use crate::storage::WalletRecord;

pub fn import_secret_record(
    name: &str,
    network: &str,
    secret: &str,
    passcode: &str,
) -> Result<WalletRecord, String> {
    validate_name_and_network(name, network)?;
    let registry = ProtectionRegistry::new();
    let protected = protect_secret_signing_material(&registry, secret, passcode)
        .map_err(|error| error.to_string())?;
    Ok(WalletRecord {
        name: name.to_owned(),
        address: protected.public_key,
        wallet_type: "secret".to_owned(),
        network: network.to_owned(),
        secret: Some(protected.envelope),
        metadata: Map::new(),
    })
}

pub fn import_mnemonic_record(
    name: &str,
    network: &str,
    mnemonic: &str,
    mnemonic_passphrase: &str,
    index: usize,
    language: Option<&str>,
    passcode: &str,
) -> Result<WalletRecord, String> {
    validate_name_and_network(name, network)?;
    let language = match language {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ => detect_mnemonic_language(mnemonic)
            .map_err(|error| error.to_string())?
            .to_owned(),
    };
    let registry = ProtectionRegistry::new();
    let protected = protect_mnemonic_signing_material(
        &registry,
        mnemonic,
        mnemonic_passphrase,
        index,
        Some(&language),
        passcode,
    )
    .map_err(|error| error.to_string())?;
    Ok(WalletRecord {
        name: name.to_owned(),
        address: protected.public_key,
        wallet_type: "mnemonic".to_owned(),
        network: network.to_owned(),
        secret: Some(protected.envelope),
        metadata: mnemonic_metadata(index, &language),
    })
}

pub fn create_mnemonic_record(
    name: &str,
    network: &str,
    mnemonic_passphrase: &str,
    index: usize,
    language: &str,
    strength: usize,
    passcode: &str,
) -> Result<(WalletRecord, Zeroizing<String>), String> {
    validate_name_and_network(name, network)?;
    let registry = ProtectionRegistry::new();
    let generated = generate_protected_mnemonic(
        &registry,
        language,
        strength,
        mnemonic_passphrase,
        index,
        passcode,
    )
    .map_err(|error| error.to_string())?;
    let record = WalletRecord {
        name: name.to_owned(),
        address: generated.wallet.public_key,
        wallet_type: "mnemonic".to_owned(),
        network: network.to_owned(),
        secret: Some(generated.wallet.envelope),
        metadata: mnemonic_metadata(index, language),
    };
    Ok((record, generated.mnemonic))
}

pub fn reveal_record(
    record: &WalletRecord,
    passcode: &str,
) -> Result<ExportedSigningMaterial, String> {
    let envelope = record
        .secret
        .as_ref()
        .ok_or_else(|| "watch-only wallet has no signing material".to_owned())?;
    export_signing_material(
        &ProtectionRegistry::new(),
        envelope,
        passcode,
        &record.address,
    )
    .map_err(|error| error.to_string())
}

pub fn verify_passcode(record: &WalletRecord, passcode: &str) -> Result<(), String> {
    let envelope = record
        .secret
        .as_ref()
        .ok_or_else(|| "watch-only wallet has no signing material".to_owned())?;
    let key = fresnica_core::derive_verified_unlock_key(
        &ProtectionRegistry::new(),
        envelope,
        passcode,
        &record.address,
    )
    .map_err(|error| error.to_string())?;
    drop(key);
    Ok(())
}

fn validate_name_and_network(name: &str, network: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("wallet name cannot be empty".to_owned());
    }
    if !matches!(network, "mainnet" | "testnet") {
        return Err(format!("unknown network: {network}"));
    }
    Ok(())
}

fn mnemonic_metadata(index: usize, language: &str) -> Map<String, Value> {
    let mut metadata = Map::new();
    if let Ok(index) = u64::try_from(index) {
        metadata.insert("index".to_owned(), Value::Number(Number::from(index)));
    }
    metadata.insert("language".to_owned(), Value::String(language.to_owned()));
    metadata
}

#[cfg(test)]
mod tests {
    use fresnica_core::ExportedSigningMaterial;

    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    #[test]
    fn secret_record_is_protected_by_core_and_revealable() {
        let record = import_secret_record("alpha", "testnet", SECRET, "passcode").unwrap();
        assert_eq!(record.address, PUBLIC);
        assert_eq!(record.wallet_type, "secret");
        assert!(!record.secret.as_ref().unwrap().to_string().contains(SECRET));
        verify_passcode(&record, "passcode").unwrap();

        match reveal_record(&record, "passcode").unwrap() {
            ExportedSigningMaterial::Secret { secret } => assert_eq!(secret.as_str(), SECRET),
            _ => panic!("secret wallet revealed the wrong material kind"),
        }
    }

    #[test]
    fn generated_mnemonic_record_preserves_derivation_metadata() {
        let (record, mnemonic) = create_mnemonic_record(
            "alpha",
            "mainnet",
            "",
            3,
            "english",
            128,
            "passcode",
        )
        .unwrap();
        assert_eq!(mnemonic.split_whitespace().count(), 12);
        assert_eq!(record.wallet_type, "mnemonic");
        assert_eq!(record.metadata.get("index").and_then(Value::as_u64), Some(3));
        assert_eq!(
            record.metadata.get("language").and_then(Value::as_str),
            Some("english")
        );
        verify_passcode(&record, "passcode").unwrap();
    }
}
