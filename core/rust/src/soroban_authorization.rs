use ed25519_dalek::{Signature as Ed25519Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
use stellar_xdr::{
    Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization,
    HashIdPreimageSorobanAuthorizationWithAddress, Limits, ReadXdr, ScAddress, ScBytes, ScMap,
    ScMapEntry, ScSymbol, ScVal, ScVec, SorobanAddressCredentials, SorobanAuthorizationEntry,
    SorobanCredentials, WriteXdr,
};
use thiserror::Error;

use crate::{
    network_id, SignerError, SorobanAuthorizationSigner, SorobanAuthorizationSigningRequest,
};

// Keep untrusted authorization-entry parsing aligned with the transaction parser's finite depth.
const XDR_DECODE_MAX_DEPTH: u32 = 500;

fn xdr_decode_limits(encoded_len: usize) -> Limits {
    Limits {
        depth: XDR_DECODE_MAX_DEPTH,
        len: encoded_len,
    }
}

pub fn parse_soroban_authorization_entry_xdr(
    xdr: &[u8],
) -> Result<SorobanAuthorizationEntry, SorobanAuthorizationSigningError> {
    SorobanAuthorizationEntry::from_xdr(xdr, xdr_decode_limits(xdr.len()))
        .map_err(SorobanAuthorizationSigningError::Xdr)
}

pub fn soroban_authorization_entry_xdr(
    entry: &SorobanAuthorizationEntry,
) -> Result<Vec<u8>, SorobanAuthorizationSigningError> {
    entry
        .to_xdr(Limits::none())
        .map_err(SorobanAuthorizationSigningError::Xdr)
}

pub fn soroban_authorization_preimage(
    entry: &SorobanAuthorizationEntry,
    network_passphrase: &str,
) -> Result<HashIdPreimage, SorobanAuthorizationSigningError> {
    let network_id = Hash(network_id(network_passphrase));
    match &entry.credentials {
        SorobanCredentials::SourceAccount => {
            Err(SorobanAuthorizationSigningError::SourceAccountCredential)
        }
        SorobanCredentials::Address(credentials) => Ok(HashIdPreimage::SorobanAuthorization(
            HashIdPreimageSorobanAuthorization {
                network_id,
                nonce: credentials.nonce,
                signature_expiration_ledger: credentials.signature_expiration_ledger,
                invocation: entry.root_invocation.clone(),
            },
        )),
        SorobanCredentials::AddressV2(credentials) => {
            Ok(address_bound_preimage(entry, credentials, network_id))
        }
        SorobanCredentials::AddressWithDelegates(credentials) => Ok(address_bound_preimage(
            entry,
            &credentials.address_credentials,
            network_id,
        )),
    }
}

fn address_bound_preimage(
    entry: &SorobanAuthorizationEntry,
    credentials: &SorobanAddressCredentials,
    network_id: Hash,
) -> HashIdPreimage {
    HashIdPreimage::SorobanAuthorizationWithAddress(HashIdPreimageSorobanAuthorizationWithAddress {
        network_id,
        nonce: credentials.nonce,
        signature_expiration_ledger: credentials.signature_expiration_ledger,
        address: credentials.address.clone(),
        invocation: entry.root_invocation.clone(),
    })
}

pub fn prepare_soroban_authorization_signing(
    entry: &SorobanAuthorizationEntry,
    network_passphrase: &str,
) -> Result<SorobanAuthorizationSigningRequest, SorobanAuthorizationSigningError> {
    let authorization_entry_xdr = soroban_authorization_entry_xdr(entry)?;
    let preimage = soroban_authorization_preimage(entry, network_passphrase)?;
    let authorization_preimage_xdr = preimage
        .to_xdr(Limits::none())
        .map_err(SorobanAuthorizationSigningError::Xdr)?;
    let authorization_hash = Sha256::digest(&authorization_preimage_xdr).into();

    Ok(SorobanAuthorizationSigningRequest {
        authorization_hash,
        authorization_entry_xdr,
        authorization_preimage_xdr,
        network_passphrase: network_passphrase.to_owned(),
    })
}

pub fn sign_soroban_authorization_entry<S: SorobanAuthorizationSigner + ?Sized>(
    entry: &mut SorobanAuthorizationEntry,
    network_passphrase: &str,
    signer: &S,
) -> Result<(), SorobanAuthorizationSigningError> {
    ensure_direct_account_credential(entry)?;
    let request = prepare_soroban_authorization_signing(entry, network_passphrase)?;
    let signature = signer.sign_soroban_authorization(&request)?;
    verify_signature(signer.public_key(), &request.authorization_hash, &signature)?;
    let signed_value = ed25519_signature_value(signer.public_key(), &signature)?;
    append_signature(entry, signed_value)?;
    Ok(())
}

fn ensure_direct_account_credential(
    entry: &SorobanAuthorizationEntry,
) -> Result<(), SorobanAuthorizationSigningError> {
    let credentials = match &entry.credentials {
        SorobanCredentials::SourceAccount => {
            return Err(SorobanAuthorizationSigningError::SourceAccountCredential)
        }
        SorobanCredentials::Address(credentials) | SorobanCredentials::AddressV2(credentials) => {
            credentials
        }
        SorobanCredentials::AddressWithDelegates(_) => {
            return Err(SorobanAuthorizationSigningError::DelegatedCredentialRequiresProvider)
        }
    };

    match credentials.address {
        ScAddress::Account(_) => Ok(()),
        _ => Err(SorobanAuthorizationSigningError::NonAccountCredential),
    }
}

fn verify_signature(
    public_key: &str,
    authorization_hash: &[u8; 32],
    signature: &[u8; 64],
) -> Result<(), SorobanAuthorizationSigningError> {
    let public =
        StrkeyPublicKey::from_string(public_key).map_err(|_| SignerError::InvalidPublicKey)?;
    let verifying_key =
        VerifyingKey::from_bytes(&public.0).map_err(|_| SignerError::InvalidPublicKey)?;
    verifying_key
        .verify_strict(authorization_hash, &Ed25519Signature::from_bytes(signature))
        .map_err(|_| SorobanAuthorizationSigningError::InvalidSignature)
}

fn ed25519_signature_value(
    public_key: &str,
    signature: &[u8; 64],
) -> Result<ScVal, SorobanAuthorizationSigningError> {
    let public =
        StrkeyPublicKey::from_string(public_key).map_err(|_| SignerError::InvalidPublicKey)?;
    let map = ScMap::try_from(vec![
        ScMapEntry {
            key: ScVal::Symbol(ScSymbol::try_from(b"public_key".to_vec())?),
            val: ScVal::Bytes(ScBytes::try_from(public.0.to_vec())?),
        },
        ScMapEntry {
            key: ScVal::Symbol(ScSymbol::try_from(b"signature".to_vec())?),
            val: ScVal::Bytes(ScBytes::try_from(signature.to_vec())?),
        },
    ])?;
    Ok(ScVal::Map(Some(map)))
}

fn append_signature(
    entry: &mut SorobanAuthorizationEntry,
    signed_value: ScVal,
) -> Result<(), SorobanAuthorizationSigningError> {
    let signature = match &mut entry.credentials {
        SorobanCredentials::Address(credentials) | SorobanCredentials::AddressV2(credentials) => {
            &mut credentials.signature
        }
        SorobanCredentials::SourceAccount => {
            return Err(SorobanAuthorizationSigningError::SourceAccountCredential)
        }
        SorobanCredentials::AddressWithDelegates(_) => {
            return Err(SorobanAuthorizationSigningError::DelegatedCredentialRequiresProvider)
        }
    };

    let mut existing = match signature {
        ScVal::Void => Vec::new(),
        ScVal::Vec(Some(values)) => values.clone().into(),
        ScVal::Vec(None) => return Err(SorobanAuthorizationSigningError::InvalidSignaturePayload),
        _ => return Err(SorobanAuthorizationSigningError::UnsupportedSignaturePayload),
    };

    if existing.iter().any(|value| value == &signed_value) {
        return Err(SorobanAuthorizationSigningError::DuplicateSignature);
    }
    existing.push(signed_value);
    *signature = ScVal::Vec(Some(ScVec::try_from(existing)?));
    Ok(())
}

#[derive(Debug, Error)]
pub enum SorobanAuthorizationSigningError {
    #[error("invalid Soroban authorization-entry XDR")]
    Xdr(#[from] stellar_xdr::Error),
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error("source-account authorization is satisfied by the transaction envelope signer")]
    SourceAccountCredential,
    #[error("direct Ed25519 authorization requires a classic account address credential")]
    NonAccountCredential,
    #[error("delegated Soroban authorization requires an explicit provider/target signer path")]
    DelegatedCredentialRequiresProvider,
    #[error("signer returned a signature for the wrong authorization payload")]
    InvalidSignature,
    #[error("signer has already added this Soroban authorization signature")]
    DuplicateSignature,
    #[error("Soroban authorization signature vector is malformed")]
    InvalidSignaturePayload,
    #[error("Soroban authorization uses a custom signature payload that Core must not overwrite")]
    UnsupportedSignaturePayload,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::Value;
    use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
    use stellar_xdr::{
        AccountId, PublicKey, SorobanAddressCredentials, SorobanCredentials, Uint256,
    };

    use super::*;
    use crate::{ExternalSorobanEd25519Signer, SoftwareSigner};

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const MAINNET: &str = "Public Global Stellar Network ; September 2015";

    fn account_address(public_key: &str) -> ScAddress {
        let public = StrkeyPublicKey::from_string(public_key).unwrap();
        ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            public.0,
        ))))
    }

    fn entry(v2: bool) -> SorobanAuthorizationEntry {
        let credentials = SorobanAddressCredentials {
            address: account_address(PUBLIC),
            nonce: 123_456_789_101_112,
            signature_expiration_ledger: 4242,
            signature: ScVal::Vec(Some(ScVec::default())),
        };
        SorobanAuthorizationEntry {
            credentials: if v2 {
                SorobanCredentials::AddressV2(credentials)
            } else {
                SorobanCredentials::Address(credentials)
            },
            root_invocation: Default::default(),
        }
    }

    fn signature_values(entry: &SorobanAuthorizationEntry) -> &[ScVal] {
        let signature = match &entry.credentials {
            SorobanCredentials::Address(credentials)
            | SorobanCredentials::AddressV2(credentials) => &credentials.signature,
            _ => panic!("test expected direct address credentials"),
        };
        match signature {
            ScVal::Vec(Some(values)) => values.as_ref(),
            _ => panic!("test expected signature vector"),
        }
    }

    fn authorization_vector() -> Value {
        serde_json::from_str(include_str!(
            "../../../spec/test-vectors/soroban-authorization-signing-v1.json"
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
    fn shared_authorization_vector_matches_official_xdr_and_signature() {
        let vector = authorization_vector();
        let case = &vector["cases"][0];
        let unsigned = STANDARD
            .decode(case["unsigned_entry_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_preimage = STANDARD
            .decode(case["authorization_preimage_xdr_base64"].as_str().unwrap())
            .unwrap();
        let expected_hash = decode_hex(case["authorization_hash_hex"].as_str().unwrap());
        let expected_signed = STANDARD
            .decode(case["signed_entry_xdr_base64"].as_str().unwrap())
            .unwrap();
        let parsed = parse_soroban_authorization_entry_xdr(&unsigned).unwrap();
        let prepared = prepare_soroban_authorization_signing(&parsed, TESTNET).unwrap();
        assert_eq!(prepared.authorization_entry_xdr, unsigned);
        assert_eq!(prepared.authorization_preimage_xdr, expected_preimage);
        assert_eq!(prepared.authorization_hash.as_slice(), expected_hash);

        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let mut signed = parsed;
        sign_soroban_authorization_entry(&mut signed, TESTNET, &signer).unwrap();
        assert_eq!(
            soroban_authorization_entry_xdr(&signed).unwrap(),
            expected_signed
        );
    }

    #[test]
    fn round_trips_authorization_entry_with_bounded_xdr_parser() {
        let entry = entry(true);
        let encoded = soroban_authorization_entry_xdr(&entry).unwrap();
        let decoded = parse_soroban_authorization_entry_xdr(&encoded).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn legacy_and_address_v2_select_the_official_preimage_variants() {
        let legacy = soroban_authorization_preimage(&entry(false), TESTNET).unwrap();
        let v2 = soroban_authorization_preimage(&entry(true), TESTNET).unwrap();

        assert!(matches!(legacy, HashIdPreimage::SorobanAuthorization(_)));
        let HashIdPreimage::SorobanAuthorizationWithAddress(v2) = v2 else {
            panic!("AddressV2 must use the address-bound preimage");
        };
        assert_eq!(v2.address, account_address(PUBLIC));
        assert_eq!(v2.nonce, 123_456_789_101_112);
        assert_eq!(v2.signature_expiration_ledger, 4242);
    }

    #[test]
    fn authorization_hash_is_bound_to_network_and_address_v2_variant() {
        let legacy = prepare_soroban_authorization_signing(&entry(false), TESTNET).unwrap();
        let v2_testnet = prepare_soroban_authorization_signing(&entry(true), TESTNET).unwrap();
        let v2_mainnet = prepare_soroban_authorization_signing(&entry(true), MAINNET).unwrap();

        assert_ne!(legacy.authorization_hash, v2_testnet.authorization_hash);
        assert_ne!(v2_testnet.authorization_hash, v2_mainnet.authorization_hash);
        assert_eq!(
            v2_testnet.authorization_entry_xdr,
            soroban_authorization_entry_xdr(&entry(true)).unwrap()
        );
    }

    #[test]
    fn software_signer_adds_standard_ed25519_signature_without_changing_authorized_invocation() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let mut entry = entry(true);
        let invocation = entry.root_invocation.clone();

        sign_soroban_authorization_entry(&mut entry, TESTNET, &signer).unwrap();

        assert_eq!(entry.root_invocation, invocation);
        assert_eq!(signature_values(&entry).len(), 1);
    }

    #[test]
    fn external_signer_receives_exact_entry_preimage_and_network() {
        let software = SoftwareSigner::from_secret(SECRET).unwrap();
        let expected = prepare_soroban_authorization_signing(&entry(true), TESTNET).unwrap();
        let captured = Arc::new(Mutex::new(None));
        let capture = Arc::clone(&captured);
        let signer = ExternalSorobanEd25519Signer::new(PUBLIC, move |request| {
            *capture.lock().unwrap() = Some(request.clone());
            software.sign_soroban_authorization(request)
        })
        .unwrap();
        let mut value = entry(true);

        sign_soroban_authorization_entry(&mut value, TESTNET, &signer).unwrap();

        assert_eq!(captured.lock().unwrap().as_ref(), Some(&expected));
        assert_eq!(signature_values(&value).len(), 1);
    }

    #[test]
    fn invalid_external_signature_does_not_mutate_entry() {
        let signer = ExternalSorobanEd25519Signer::new(PUBLIC, |_| Ok([0u8; 64])).unwrap();
        let mut value = entry(true);
        let before = value.clone();

        let error = sign_soroban_authorization_entry(&mut value, TESTNET, &signer).unwrap_err();

        assert!(matches!(
            error,
            SorobanAuthorizationSigningError::InvalidSignature
        ));
        assert_eq!(value, before);
    }

    #[test]
    fn contract_and_source_account_credentials_fail_closed_for_direct_ed25519_signing() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let mut source = SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: Default::default(),
        };
        assert!(matches!(
            sign_soroban_authorization_entry(&mut source, TESTNET, &signer),
            Err(SorobanAuthorizationSigningError::SourceAccountCredential)
        ));

        let mut contract = entry(true);
        let contract_id = stellar_xdr::ContractId(Hash([7u8; 32]));
        if let SorobanCredentials::AddressV2(credentials) = &mut contract.credentials {
            credentials.address = ScAddress::Contract(contract_id);
        }
        assert!(matches!(
            sign_soroban_authorization_entry(&mut contract, TESTNET, &signer),
            Err(SorobanAuthorizationSigningError::NonAccountCredential)
        ));
    }

    #[test]
    fn duplicate_signature_is_rejected_without_adding_another_entry() {
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        let mut value = entry(true);
        sign_soroban_authorization_entry(&mut value, TESTNET, &signer).unwrap();
        assert_eq!(signature_values(&value).len(), 1);

        let error = sign_soroban_authorization_entry(&mut value, TESTNET, &signer).unwrap_err();
        assert!(matches!(
            error,
            SorobanAuthorizationSigningError::DuplicateSignature
        ));
        assert_eq!(signature_values(&value).len(), 1);
    }
}
