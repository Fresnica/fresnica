use fresnica_sdk::{FresnicaSdk, SdkErrorCode};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PublicKey;
use zeroize::Zeroizing;

use crate::storage::WalletRecord;
use crate::wallet::record_envelope_json;

pub const SYSTEM_AUTH_UNLOCK_KEY_LENGTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemAuthSlot {
    pub signer_public_key: String,
    pub envelope_fingerprint: String,
}

impl SystemAuthSlot {
    pub fn storage_id(&self) -> String {
        format!("{}:{}", self.signer_public_key, self.envelope_fingerprint)
    }
}

pub struct SystemAuthEnrollment {
    pub slot: SystemAuthSlot,
    unlock_key: Zeroizing<Vec<u8>>,
}

impl SystemAuthEnrollment {
    pub fn unlock_key(&self) -> &[u8] {
        self.unlock_key.as_slice()
    }

    pub fn into_unlock_key(self) -> Zeroizing<Vec<u8>> {
        self.unlock_key
    }
}

type ReleaseUnlockKeyFn = dyn Fn(&SystemAuthSlot) -> Result<Vec<u8>, String>;

pub struct SystemAuthUnlockProvider {
    public_key: String,
    release_unlock_key: Box<ReleaseUnlockKeyFn>,
}

impl SystemAuthUnlockProvider {
    pub fn new<F>(public_key: &str, release_unlock_key: F) -> Result<Self, String>
    where
        F: Fn(&SystemAuthSlot) -> Result<Vec<u8>, String> + 'static,
    {
        let public = PublicKey::from_string(public_key.trim()).map_err(|_| {
            "system-auth provider requires a valid Stellar Ed25519 public key".to_owned()
        })?;
        Ok(Self {
            public_key: format!("{public}"),
            release_unlock_key: Box::new(release_unlock_key),
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub(crate) fn release(&self, slot: &SystemAuthSlot) -> Result<Vec<u8>, String> {
        if slot.signer_public_key != self.public_key {
            return Err("system-auth provider slot does not match signer identity".to_owned());
        }
        let key = (self.release_unlock_key)(slot)?;
        if key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
            return Err(format!(
                "system-auth provider returned {} bytes; expected {SYSTEM_AUTH_UNLOCK_KEY_LENGTH}",
                key.len()
            ));
        }
        Ok(key)
    }
}

pub fn system_auth_slot(record: &WalletRecord) -> Result<SystemAuthSlot, String> {
    if record.watch_only() || record.secret.is_none() {
        return Err("watch-only wallet has no software unlock key".to_owned());
    }
    let envelope_json = record_envelope_json(record)?;
    let digest = Sha256::digest(envelope_json.as_bytes());
    Ok(SystemAuthSlot {
        signer_public_key: record.address.clone(),
        envelope_fingerprint: hex(&digest),
    })
}

pub fn prepare_system_auth_enrollment(
    record: &WalletRecord,
    passphrase: &str,
) -> Result<SystemAuthEnrollment, String> {
    let slot = system_auth_slot(record)?;
    let unlock_key = FresnicaSdk::new()
        .derive_unlock_key(
            record_envelope_json(record)?,
            passphrase.to_owned(),
            record.address.clone(),
        )
        .map_err(|error| match error.code {
            SdkErrorCode::InvalidPasscode => "invalid Fresnica passphrase".to_owned(),
            _ => format!("unable to prepare system-auth enrollment: {error}"),
        })?;
    if unlock_key.len() != SYSTEM_AUTH_UNLOCK_KEY_LENGTH {
        return Err("SDK returned an invalid wallet unlock key length".to_owned());
    }
    Ok(SystemAuthEnrollment {
        slot,
        unlock_key: Zeroizing::new(unlock_key),
    })
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::import_secret_record;

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn record() -> WalletRecord {
        import_secret_record("wallet", "testnet", SECRET, "0123456789abcde").unwrap()
    }

    #[test]
    fn enrollment_contains_only_verified_unlock_key_and_exact_slot() {
        let record = record();
        let enrollment = prepare_system_auth_enrollment(&record, "0123456789abcde").unwrap();
        assert_eq!(enrollment.slot.signer_public_key, PUBLIC);
        assert_eq!(enrollment.unlock_key().len(), 32);
        assert!(enrollment.slot.storage_id().starts_with(PUBLIC));
    }

    #[test]
    fn slot_changes_when_the_exact_envelope_changes() {
        let record = record();
        let original = system_auth_slot(&record).unwrap();
        let mut changed = record.clone();
        changed.secret.as_mut().unwrap()["payload"]["nonce"] =
            serde_json::json!("AAAAAAAAAAAAAAAA");
        let changed = system_auth_slot(&changed).unwrap();
        assert_ne!(original.storage_id(), changed.storage_id());
    }

    #[test]
    fn watch_only_cannot_create_system_auth_slot() {
        let mut record = record();
        record.wallet_type = "watch-only".to_owned();
        record.secret = None;
        assert_eq!(
            system_auth_slot(&record).unwrap_err(),
            "watch-only wallet has no software unlock key"
        );
    }

    #[test]
    fn provider_rejects_wrong_identity_and_wrong_key_length() {
        let slot = system_auth_slot(&record()).unwrap();
        let provider = SystemAuthUnlockProvider::new(PUBLIC, |_| Ok(vec![0u8; 31])).unwrap();
        assert!(provider.release(&slot).unwrap_err().contains("expected 32"));
        let other = SystemAuthSlot {
            signer_public_key: "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
                .to_owned(),
            envelope_fingerprint: slot.envelope_fingerprint,
        };
        assert!(provider
            .release(&other)
            .unwrap_err()
            .contains("signer identity"));
    }
}
