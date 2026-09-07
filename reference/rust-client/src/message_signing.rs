use fresnica_sdk::FresnicaSdk;

use crate::{WalletRecord, WalletStorage};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageSigningReview {
    pub wallet_name: String,
    pub signer_public_key: String,
    pub message: Vec<u8>,
    pub encoded_message: Vec<u8>,
    pub message_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedMessageSigning {
    pub review: MessageSigningReview,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageSignature {
    pub signer_public_key: String,
    pub message_hash: [u8; 32],
    pub signature: [u8; 64],
}

pub fn prepare_sep53_message(
    storage: &WalletStorage,
    wallet_name: Option<&str>,
    message: &[u8],
) -> Result<PreparedMessageSigning, String> {
    let wallet = storage.resolve(wallet_name)?;
    ensure_local_message_signer(&wallet)?;
    let request = FresnicaSdk::new().prepare_message_signing(message.to_vec());
    let message_hash: [u8; 32] = request
        .message_hash
        .try_into()
        .map_err(|_| "SDK returned an invalid SEP-53 message hash".to_owned())?;

    Ok(PreparedMessageSigning {
        review: MessageSigningReview {
            wallet_name: wallet.name,
            signer_public_key: wallet.address,
            message: request.message,
            encoded_message: request.encoded_message,
            message_hash,
        },
    })
}

pub fn sign_prepared_sep53_message(
    storage: &WalletStorage,
    prepared: &PreparedMessageSigning,
    passcode: &str,
) -> Result<MessageSignature, String> {
    let wallet = storage.load(&prepared.review.wallet_name)?;
    ensure_local_message_signer(&wallet)?;
    if wallet.address != prepared.review.signer_public_key {
        return Err(format!(
            "wallet identity changed while reviewing SEP-53 message: {}",
            wallet.name
        ));
    }

    let sdk = FresnicaSdk::new();
    let current = sdk.prepare_message_signing(prepared.review.message.clone());
    let current_hash: [u8; 32] = current
        .message_hash
        .try_into()
        .map_err(|_| "SDK returned an invalid SEP-53 message hash".to_owned())?;
    if current_hash != prepared.review.message_hash
        || current.encoded_message != prepared.review.encoded_message
    {
        return Err(
            "reviewed SEP-53 message changed; prepare and review the message again before signing"
                .to_owned(),
        );
    }

    let signature = sdk
        .sign_message_with_passcode(
            record_envelope_json(&wallet)?,
            passcode.to_owned(),
            wallet.address.clone(),
            prepared.review.message.clone(),
        )
        .map_err(|error| error.to_string())?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| "SDK returned an invalid SEP-53 signature length".to_owned())?;
    sdk.verify_message_signature(
        prepared.review.message.clone(),
        wallet.address.clone(),
        signature.to_vec(),
    )
    .map_err(|error| error.to_string())?;

    Ok(MessageSignature {
        signer_public_key: wallet.address,
        message_hash: current_hash,
        signature,
    })
}

pub fn verify_sep53_message(
    message: &[u8],
    signer_public_key: &str,
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

fn ensure_local_message_signer(wallet: &WalletRecord) -> Result<(), String> {
    if wallet.watch_only() || wallet.secret.is_none() {
        return Err(format!(
            "wallet {} has no local signing material for SEP-53 message signing",
            wallet.name
        ));
    }
    Ok(())
}

fn record_envelope_json(wallet: &WalletRecord) -> Result<String, String> {
    let envelope = wallet
        .secret
        .as_ref()
        .ok_or_else(|| "wallet has no local signing material".to_owned())?;
    serde_json::to_string(envelope)
        .map_err(|error| format!("unable to serialize protected signer envelope: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::{import_secret_record, import_watch_record};

    const SECRET: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    const PUBLIC: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";
    const OTHER_SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const OTHER_PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const PASSPHRASE: &str = "correct horse battery staple";

    fn storage(label: &str) -> (std::path::PathBuf, WalletStorage) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "fresnica-sep53-{label}-{}-{nonce}",
            std::process::id()
        ));
        let storage = WalletStorage::new(&home).unwrap();
        (home, storage)
    }

    fn decode_hex<const N: usize>(hex: &str) -> [u8; N] {
        assert_eq!(hex.len(), N * 2);
        let mut out = [0u8; N];
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        out
    }

    #[test]
    fn prepare_and_sign_match_final_sep53_vector() {
        let (home, storage) = storage("vector");
        let wallet = import_secret_record("alpha", "mainnet", SECRET, PASSPHRASE).unwrap();
        storage.save(&wallet, false).unwrap();
        storage.set_default("alpha").unwrap();

        let prepared = prepare_sep53_message(&storage, None, b"Hello, World!").unwrap();
        assert_eq!(prepared.review.signer_public_key, PUBLIC);
        assert_eq!(
            prepared.review.message_hash,
            decode_hex::<32>("d52eb59c06bb510d065997ff93077068eed0a486c20215b5e02e1ab0d2ebea5f")
        );
        assert_eq!(
            prepared.review.encoded_message,
            b"Stellar Signed Message:\nHello, World!"
        );

        let signed = sign_prepared_sep53_message(&storage, &prepared, PASSPHRASE).unwrap();
        assert_eq!(signed.signer_public_key, PUBLIC);
        assert_eq!(signed.message_hash, prepared.review.message_hash);
        assert_eq!(
            signed.signature,
            decode_hex::<64>(concat!(
                "7cee5d6d885752104c85eea421dfdcb95abf01f1271d11c4bec3fcbd7874dccd",
                "6e2e98b97b8eb23b643cac4073bb77de5d07b0710139180ae9f3cbba78f2ba04"
            ))
        );
        verify_sep53_message(b"Hello, World!", PUBLIC, &signed.signature).unwrap();
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn binary_message_bytes_are_preserved_exactly() {
        let (home, storage) = storage("binary");
        let wallet = import_secret_record("alpha", "testnet", SECRET, PASSPHRASE).unwrap();
        storage.save(&wallet, false).unwrap();
        let message = [0xff, 0x00, 0x80, 0x41];

        let prepared = prepare_sep53_message(&storage, Some("alpha"), &message).unwrap();
        assert_eq!(prepared.review.message, message);
        assert_eq!(
            &prepared.review.encoded_message[b"Stellar Signed Message:\n".len()..],
            message
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn watch_only_wallet_cannot_sign_message() {
        let (home, storage) = storage("watch");
        let wallet = import_watch_record("observer", "mainnet", PUBLIC).unwrap();
        storage.save(&wallet, false).unwrap();

        let error = prepare_sep53_message(&storage, Some("observer"), b"challenge").unwrap_err();
        assert!(error.contains("no local signing material"));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn prepared_message_is_bound_to_wallet_identity() {
        let (home, storage) = storage("identity");
        let wallet = import_secret_record("alpha", "mainnet", SECRET, PASSPHRASE).unwrap();
        storage.save(&wallet, false).unwrap();
        let prepared = prepare_sep53_message(&storage, Some("alpha"), b"challenge").unwrap();

        let replacement =
            import_secret_record("alpha", "mainnet", OTHER_SECRET, PASSPHRASE).unwrap();
        assert_eq!(replacement.address, OTHER_PUBLIC);
        storage.save(&replacement, true).unwrap();

        assert_eq!(
            sign_prepared_sep53_message(&storage, &prepared, PASSPHRASE).unwrap_err(),
            "wallet identity changed while reviewing SEP-53 message: alpha"
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn tampered_review_is_rejected_before_signing() {
        let (home, storage) = storage("tamper");
        let wallet = import_secret_record("alpha", "mainnet", SECRET, PASSHRASE).unwrap();
        storage.save(&wallet, false).unwrap();
        let mut prepared = prepare_sep53_message(&storage, Some("alpha"), b"challenge").unwrap();
        prepared.review.message_hash[0] x= 0xff;

        assert!(sign_prepared_sep53_message(&storage, &prepared, PASSPHRASE)
            .unwrap_err()
            .contains("reviewed SEP-53 message changed"));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn verification_rejects_wrong_signature() {
        assert!(verify_sep53_message(b"challenge", PUBLIC, &[0u8; 64]).is_err());
    }
}
