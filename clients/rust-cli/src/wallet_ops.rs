use fresnica_core::detect_mnemonic_language;
use fresnica_sdk::{FresnicaSdk, SdkSigningMaterialKind};
use serde_json::{Map, Number, Value};
use zeroize::{Zeroize, Zeroizing};

use crate::storage::WalletRecord;

pub enum RevealedSigningMaterial {
    Secret {
        secret: Zeroizing<String>,
    },
    Mnemonic {
        mnemonic: Zeroizing<String>,
        mnemonic_passphrase: Zeroizing<String>,
        index: usize,
        language: String,
    },
}

pub fn import_secret_record(
    name: &str,
    network: &str,
    secret: &str,
    passcode: &str,
) -> Result<WalletRecord, String> {
    validate_name_and_network(name, network)?;
    let protected = FresnicaSdk::new()
        .protect_secret(secret.to_owned(), passcode.to_owned(), None)
        .map_err(|error| error.to_string())?;
    Ok(WalletRecord {
        name: name.to_owned(),
        address: protected.signer_public_key,
        wallet_type: "secret".to_owned(),
        network: network.to_owned(),
        secret: Some(parse_envelope(&protected.envelope_json)?),
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
    let protected = FresnicaSdk::new()
        .protect_mnemonic(
            mnemonic.to_owned(),
            mnemonic_passphrase.to_owned(),
            sdk_u32(index, "index")?,
            Some(language.clone()),
            passcode.to_owned(),
            None,
        )
        .map_err(|error| error.to_string())?;
    Ok(WalletRecord {
        name: name.to_owned(),
        address: protected.signer_public_key,
        wallet_type: "mnemonic".to_owned(),
        network: network.to_owned(),
        secret: Some(parse_envelope(&protected.envelope_json)?),
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
    let generated = FresnicaSdk::new()
        .generate_mnemonic(
            language.to_owned(),
            sdk_u32(strength, "strength")?,
            mnemonic_passphrase.to_owned(),
            sdk_u32(index, "index")?,
            passcode.to_owned(),
        )
        .map_err(|error| error.to_string())?;
    let record = WalletRecord {
        name: name.to_owned(),
        address: generated.signer.signer_public_key,
        wallet_type: "mnemonic".to_owned(),
        network: network.to_owned(),
        secret: Some(parse_envelope(&generated.signer.envelope_json)?),
        metadata: mnemonic_metadata(index, &generated.language),
    };
    Ok((record, Zeroizing::new(generated.mnemonic)))
}

pub fn verify_passcode(record: &WalletRecord, passcode: &str) -> Result<(), String> {
    let envelope_json = record_envelope_json(record)?;
    let mut unlock_key = FresnicaSdk::new()
        .derive_unlock_key(envelope_json, passcode.to_owned(), record.address.clone())
        .map_err(|_| "invalid Fresnica passcode".to_owned())?;
    unlock_key.zeroize();
    Ok(())
}

pub fn reveal_record(
    record: &WalletRecord,
    passcode: &str,
) -> Result<RevealedSigningMaterial, String> {
    let material = FresnicaSdk::new()
        .reveal(
            record_envelope_json(record)?,
            passcode.to_owned(),
            record.address.clone(),
        )
        .map_err(|error| error.to_string())?;
    match material.kind {
        SdkSigningMaterialKind::Secret => {
            let secret = material
                .secret
                .ok_or_else(|| "SDK returned secret material without a secret".to_owned())?;
            Ok(RevealedSigningMaterial::Secret {
                secret: Zeroizing::new(secret),
            })
        }
        SdkSigningMaterialKind::Mnemonic => {
            let mnemonic = material
                .mnemonic
                .ok_or_else(|| "SDK returned mnemonic material without a mnemonic".to_owned())?;
            let mnemonic_passphrase = material.mnemonic_passphrase.ok_or_else(|| {
                "SDK returned mnemonic material without a mnemonic passphrase field".to_owned()
            })?;
            let index = material
                .index
                .ok_or_else(|| "SDK returned mnemonic material without an index".to_owned())?;
            let language = material
                .language
                .ok_or_else(|| "SDK returned mnemonic material without a language".to_owned())?;
            Ok(RevealedSigningMaterial::Mnemonic {
                mnemonic: Zeroizing::new(mnemonic),
                mnemonic_passphrase: Zeroizing::new(mnemonic_passphrase),
                index: usize::try_from(index)
                    .map_err(|_| "mnemonic index is unsupported on this platform".to_owned())?,
                language,
            })
        }
    }
}

fn record_envelope_json(record: &WalletRecord) -> Result<String, String> {
    let envelope = record
        .secret
        .as_ref()
        .ok_or_else(|| "watch-only wallet has no signing material".to_owned())?;
    serde_json::to_string(envelope)
        .map_err(|error| format!("unable to serialize protected signer envelope: {error}"))
}

fn parse_envelope(envelope_json: &str) -> Result<Value, String> {
    serde_json::from_str(envelope_json)
        .map_err(|error| format!("SDK returned an invalid protected signer envelope: {error}"))
}

fn sdk_u32(value: usize, field: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("{field} is too large"))
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
    use super::*;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    #[test]
    fn secret_record_is_protected_by_sdk_and_revealable() {
        let record = import_secret_record("alpha", "testnet", SECRET, "passcode").unwrap();
        assert_eq!(record.address, PUBLIC);
        assert_eq!(record.wallet_type, "secret");
        assert!(!record.secret.as_ref().unwrap().to_string().contains(SECRET));
        verify_passcode(&record, "passcode").unwrap();
        assert_eq!(
            verify_passcode(&record, "different").unwrap_err(),
            "invalid Fresnica passcode"
        );

        match reveal_record(&record, "passcode").unwrap() {
            RevealedSigningMaterial::Secret { secret } => assert_eq!(secret.as_str(), SECRET),
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
        match reveal_record(&record, "passcode").unwrap() {
            RevealedSigningMaterial::Mnemonic {
                mnemonic: revealed,
                index,
                language,
                ..
            } => {
                assert_eq!(revealed.as_str(), mnemonic.as_str());
                assert_eq!(index, 3);
                assert_eq!(language, "english");
            }
            _ => panic!("mnemonic wallet revealed the wrong material kind"),
        }
    }
}
