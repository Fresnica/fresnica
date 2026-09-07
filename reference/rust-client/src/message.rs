use fresnica_sdk::{FresnicaSdk, SdkError, SdkErrorCode};

use crate::storage::WalletRecord;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sep53MessageSignature {
    pub signer_public_key: String,
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],
}

pub fn sign_sep53_message(
    record: &WalletRecord,
    passphrase: &str,
    message: &[u8],
) -> Result<Sep53MessageSignature, String> {
    let envelope = record
        .secret
        .as_ref()
        .ok_or_else(|| "watch-only wallet has no signing material".to_owned())?;
    let envelope_json = serde_json::to_string(envelope)
        .map_err(|error| format!("unable to serialize protected signer envelope: {error}"))?;
    let sdk = FresnicaSdk::new();
    let prepared = sdk.prepare_message_signing(message.to_vec());
    let message_hash: [u8; 32] = prepared
        .message_hash
        .try_into()
        .map_err(|_| "SDK returned an invalid SEP-53 message hash".to_owned())?;
    let signature = sdk
        .sign_message_with_passcode(
            envelope_json,
            passphrase.to_owned(),
            record.address.clone(),
            message.to_vec(),
        )
        .map_err(map_message_signing_error)?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| "SDK returned an invalid SEP-53 signature length".to_owned())?;
    Ok(Sep53MessageSignature {
        signer_public_key: record.address.clone(),
        message_hash,
        signature,
    })
}

pub fn verify_sep53_message(
    signer_public_key: &str,
    message: &[u8],
    signature: &[u8],
) -> Result<(), String> {
    FresnicaSdk::new()
        .verify_message_signature(
            message.to_vec(),
            signer_public_key.to_owned(),
            signature.to_vec(),
        )
        .map_err(|error| error.to_string())
}

fn map_message_signing_error(error: SdkError) -> String {
    if matches!(
        error.code,
        SdkErrorCode::InvalidPasscode | SdkErrorCode::InvalidUnlockKey
    ) {
        "invalid Fresnica passphrase".to_owned()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::{import_secret_record, import_watch_record};

    const SECRET: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    const PUBLIC: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";
    const PASSPHRASE: &str = "correct horse battery staple";

    #[test]
    fn protected_wallet_signs_and_verifies_sep53_message() {
        let record = import_secret_record("signer", "testnet", SECRET, PASSPHRASE).unwrap();
        let signed = sign_sep53_message(&record, PASSPHRASE, b"Hello, World!").unwrap();

        assert_eq!(signed.signer_public_key, PUBLIC);
        assert_eq!(signed.message_hash.len(), 32);
        assert_eq!(signed.signature.len(), 64);
        verify_sep53_message(PUBLIC, b"Hello, World!", &signed.signature).unwrap();
    }

    #[test]
    fn signing_fails_closed_for_watch_only_or_wrong_passphrase() {
        let watch = import_watch_record("observer", "testnet", PUBLIC).unwrap();
        assert_eq!(
            sign_sep53_message(&watch, PASSPHRASE, b"challenge").unwrap_err(),
            "watch-only wallet has no signing material"
        );

        let record = import_secret_record("signer", "testnet", SECRET, PASSPHRASE).unwrap();
        assert_eq!(
            sign_sep53_message(&record, "different passphrase value", b"challenge").unwrap_err(),
            "invalid Fresnica passphrase"
        );
    }

    #[test]
    fn verification_rejects_mutated_message_or_signature() {
        let record = import_secret_record("signer", "testnet", SECRET, PASSPHRASE).unwrap();
        let signed = sign_sep53_message(&record, PASSPHRASE, b"challenge").unwrap();

        assert!(verify_sep53_message(PUBLIC, b"changed", &signed.signature).is_err());
        let mut mutated = signed.signature;
        mutated[0] ^= 1;
        assert!(verify_sep53_message(PUBLIC, b"challenge", &mutated).is_err());
    }
}
