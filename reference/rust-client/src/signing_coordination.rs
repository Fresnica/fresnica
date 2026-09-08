use std::collections::{BTreeMap, BTreeSet};

use fresnica_sdk::{FresnicaSdk, SdkEd25519SigningRequest};
use stellar_strkey::ed25519::PublicKey;
use stellar_xdr::TransactionEnvelope;

use crate::ledger_authorization::{
    satisfied_transaction_conditions, summarize_ledger_authorization, LedgerAuthorizationPlan,
    LedgerAuthorizationSnapshot, LedgerSignerCondition, LedgerSignerKind, WeightedLedgerSigner,
};
use crate::storage::{WalletRecord, WalletStorage};
use crate::system_auth::{system_auth_slot, SystemAuthUnlockProvider};
use crate::transaction::{
    network_passphrase, parse_transaction_xdr, sign_transaction_xdr_with_passcode,
    sign_transaction_xdr_with_unlock_key, transaction_hash_bytes, transaction_xdr_bytes,
};
use crate::wallet::verify_passcode;

type ExternalEd25519SigningFn = dyn Fn(&SdkEd25519SigningRequest) -> Result<Vec<u8>, String>;

pub struct ExternalEd25519SigningProvider {
    public_key: String,
    sign_request: Box<ExternalEd25519SigningFn>,
}

impl ExternalEd25519SigningProvider {
    pub fn new<F>(public_key: &str, sign_request: F) -> Result<Self, String>
    where
        F: Fn(&SdkEd25519SigningRequest) -> Result<Vec<u8>, String> + 'static,
    {
        let public = PublicKey::from_string(public_key.trim()).map_err(|_| {
            "external signer must use a valid Stellar Ed25519 public key".to_owned()
        })?;
        Ok(Self {
            public_key: format!("{public}"),
            sign_request: Box::new(sign_request),
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    fn sign(&self, request: &SdkEd25519SigningRequest) -> Result<Vec<u8>, String> {
        (self.sign_request)(request)
    }
}

pub fn review_ledger_authorization(
    storage: &WalletStorage,
    plan: &LedgerAuthorizationPlan,
    network: &str,
    envelope: &TransactionEnvelope,
) -> Result<LedgerAuthorizationSnapshot, String> {
    let satisfied = satisfied_transaction_conditions(plan, envelope, network_passphrase(network)?)?;
    let local_ed25519_keys = local_signing_records(storage, network)?
        .into_keys()
        .collect::<BTreeSet<_>>();
    let transaction_hash = hex(&transaction_hash_bytes(envelope, network)?);
    Ok(summarize_ledger_authorization(
        plan,
        &satisfied,
        &local_ed25519_keys,
        transaction_hash,
    ))
}

pub fn sign_with_ed25519_providers(
    storage: &WalletStorage,
    plan: &LedgerAuthorizationPlan,
    network: &str,
    envelope: &mut TransactionEnvelope,
    passcode: Option<&str>,
    system_auth_providers: &[SystemAuthUnlockProvider],
    external_providers: &[ExternalEd25519SigningProvider],
) -> Result<(), String> {
    let network_passphrase = network_passphrase(network)?;
    let satisfied = satisfied_transaction_conditions(plan, envelope, network_passphrase)?;
    sign_needed_with_ed25519_providers(
        storage,
        plan,
        &satisfied,
        &BTreeSet::new(),
        0,
        network,
        envelope,
        passcode,
        system_auth_providers,
        external_providers,
    )?;

    let satisfied = satisfied_transaction_conditions(plan, envelope, network_passphrase)?;
    if plan.is_satisfiable_by(&satisfied) {
        Ok(())
    } else {
        Err("Signing Coordination did not satisfy ledger authorization".to_owned())
    }
}

pub fn sign_needed_with_ed25519_providers(
    storage: &WalletStorage,
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    excluded_keys: &BTreeSet<String>,
    minimum_signatures: usize,
    network: &str,
    envelope: &mut TransactionEnvelope,
    passcode: Option<&str>,
    system_auth_providers: &[SystemAuthUnlockProvider],
    external_providers: &[ExternalEd25519SigningProvider],
) -> Result<(), String> {
    let network_passphrase = network_passphrase(network)?;
    let records = local_signing_records(storage, network)?;

    let mut system_auth = BTreeMap::new();
    for provider in system_auth_providers {
        if !records.contains_key(provider.public_key()) {
            return Err(format!(
                "system-auth provider {} has no matching local protected software signer",
                provider.public_key()
            ));
        }
        if system_auth
            .insert(provider.public_key().to_owned(), provider)
            .is_some()
        {
            return Err(format!(
                "duplicate system-auth provider: {}",
                provider.public_key()
            ));
        }
    }

    let mut providers = BTreeMap::new();
    for provider in external_providers {
        if providers
            .insert(provider.public_key().to_owned(), provider)
            .is_some()
        {
            return Err(format!(
                "duplicate external Ed25519 signer provider: {}",
                provider.public_key()
            ));
        }
    }

    if let Some(key) = providers.keys().find(|key| system_auth.contains_key(*key)) {
        return Err(format!(
            "signer {key} is configured as both system-auth and external provider"
        ));
    }

    let available = records
        .keys()
        .chain(providers.keys())
        .filter(|key| !excluded_keys.contains(*key))
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected = select_ed25519_signers(plan, satisfied, &available, minimum_signatures)?;

    let local_selected = selected
        .iter()
        .filter(|key| !providers.contains_key(*key) && !system_auth.contains_key(*key))
        .collect::<Vec<_>>();
    if !local_selected.is_empty() {
        let passcode = passcode.ok_or_else(|| {
            "Fresnica passphrase is required for selected local software signers".to_owned()
        })?;
        for key in &local_selected {
            let record = records
                .get(*key)
                .expect("selected local signer must come from local records");
            verify_passcode(record, passcode)?;
        }
    }

    let sdk = FresnicaSdk::new();
    for key in selected {
        if let Some(provider) = providers.get(&key) {
            let transaction_xdr = transaction_xdr_bytes(envelope)
                .map_err(|error| format!("Unable to encode transaction before signing: {error}"))?;
            let request = sdk
                .prepare_ed25519_signing(transaction_xdr.clone(), network_passphrase.to_owned())
                .map_err(|error| format!("Unable to prepare external signing request: {error}"))?;
            let signature = provider
                .sign(&request)
                .map_err(|error| format!("External signer {key} failed: {error}"))?;
            let signed_xdr = sdk
                .apply_ed25519_signature(
                    transaction_xdr,
                    network_passphrase.to_owned(),
                    key.clone(),
                    signature,
                )
                .map_err(|error| {
                    format!("External signer {key} returned an invalid signature: {error}")
                })?;
            *envelope = parse_transaction_xdr(&signed_xdr)?;
            continue;
        }

        let record = records
            .get(&key)
            .expect("selected signer must be available locally or externally");
        let transaction_xdr = transaction_xdr_bytes(envelope)
            .map_err(|error| format!("Unable to encode transaction before signing: {error}"))?;
        if let Some(provider) = system_auth.get(&key) {
            let slot = system_auth_slot(record)?;
            let unlock_key = provider
                .release(&slot)
                .map_err(|error| format!("System authentication for {key} failed: {error}"))?;
            *envelope = parse_transaction_xdr(&sign_transaction_xdr_with_unlock_key(
                record,
                network,
                transaction_xdr,
                unlock_key,
            )?)?;
            continue;
        }
        *envelope = parse_transaction_xdr(&sign_transaction_xdr_with_passcode(
            record,
            network,
            transaction_xdr,
            passcode.expect("local signer passcode was preflighted"),
        )?)?;
    }
    Ok(())
}

pub fn sign_with_local_ed25519(
    storage: &WalletStorage,
    plan: &LedgerAuthorizationPlan,
    network: &str,
    envelope: &mut TransactionEnvelope,
    passcode: &str,
) -> Result<(), String> {
    let network_passphrase = network_passphrase(network)?;
    let satisfied = satisfied_transaction_conditions(plan, envelope, network_passphrase)?;
    sign_needed_local_ed25519(
        storage,
        plan,
        &satisfied,
        &BTreeSet::new(),
        0,
        network,
        envelope,
        passcode,
    )?;

    let satisfied = satisfied_transaction_conditions(plan, envelope, network_passphrase)?;
    if plan.is_satisfiable_by(&satisfied) {
        Ok(())
    } else {
        Err("Signing Coordination did not satisfy ledger authorization".to_owned())
    }
}

pub fn sign_needed_local_ed25519(
    storage: &WalletStorage,
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    excluded_keys: &BTreeSet<String>,
    minimum_signatures: usize,
    network: &str,
    envelope: &mut TransactionEnvelope,
    passcode: &str,
) -> Result<(), String> {
    let records = local_signing_records(storage, network)?;
    let local_signers = records
        .keys()
        .filter(|key| !excluded_keys.contains(*key))
        .cloned()
        .collect();
    let selected =
        select_local_ed25519_signers(plan, satisfied, &local_signers, minimum_signatures)?;

    for key in selected {
        let record = records
            .get(&key)
            .expect("selected signer must come from local records");
        let transaction_xdr = transaction_xdr_bytes(envelope)
            .map_err(|error| format!("Unable to encode transaction before signing: {error}"))?;
        *envelope = parse_transaction_xdr(&sign_transaction_xdr_with_passcode(
            record,
            network,
            transaction_xdr,
            passcode,
        )?)?;
    }
    Ok(())
}

fn local_signing_records(
    storage: &WalletStorage,
    network: &str,
) -> Result<BTreeMap<String, WalletRecord>, String> {
    Ok(storage
        .list()?
        .into_iter()
        .filter(|record| {
            record.network == network && !record.watch_only() && record.secret.is_some()
        })
        .map(|record| (record.address.clone(), record))
        .collect())
}

pub fn select_local_ed25519_signers(
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    local_signers: &BTreeSet<String>,
    minimum_signatures: usize,
) -> Result<Vec<String>, String> {
    select_ed25519_signers(plan, satisfied, local_signers, minimum_signatures)
}

pub fn select_ed25519_signers(
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    available_signers: &BTreeSet<String>,
    minimum_signatures: usize,
) -> Result<Vec<String>, String> {
    let mut current = satisfied.clone();
    let mut selected = Vec::new();

    for condition in &plan.extra_signers {
        if current.contains(condition) {
            continue;
        }
        if condition.kind != LedgerSignerKind::Ed25519PublicKey
            || !available_signers.contains(&condition.key)
        {
            return Err(format!(
                "Signing Coordination cannot satisfy required extra signer: {}",
                condition.key
            ));
        }
        current.insert(condition.clone());
        selected.push(condition.key.clone());
    }

    for requirement in &plan.requirements {
        while requirement.available_weight(&current) < u32::from(requirement.required_weight) {
            let available = requirement.available_weight(&current);
            let signer = best_available_signer(requirement.signers.iter(), &current, available_signers)
                .ok_or_else(|| {
                    format!(
                        "Signing Coordination cannot satisfy ledger authorization: {} requires weight {} but has {}",
                        requirement.account_id, requirement.required_weight, available
                    )
                })?;
            current.insert(signer.condition.clone());
            selected.push(signer.condition.key.clone());
        }
    }

    while current
        .iter()
        .filter(|condition| condition.kind == LedgerSignerKind::Ed25519PublicKey)
        .count()
        < minimum_signatures
    {
        let signer = best_available_signer(
            plan.requirements
                .iter()
                .flat_map(|requirement| requirement.signers.iter()),
            &current,
            available_signers,
        )
        .ok_or_else(|| {
            "Signing Coordination cannot provide the required Ed25519 proof".to_owned()
        })?;
        current.insert(signer.condition.clone());
        selected.push(signer.condition.key.clone());
    }
    Ok(selected)
}

fn best_available_signer<'a>(
    signers: impl Iterator<Item = &'a WeightedLedgerSigner>,
    current: &BTreeSet<LedgerSignerCondition>,
    available_signers: &BTreeSet<String>,
) -> Option<&'a WeightedLedgerSigner> {
    signers
        .filter(|signer| {
            signer.condition.kind == LedgerSignerKind::Ed25519PublicKey
                && available_signers.contains(&signer.condition.key)
                && !current.contains(&signer.condition)
        })
        .max_by_key(|signer| signer.weight)
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
    use crate::{
        build_operation_envelope, import_mnemonic_record, import_secret_record,
        AccountAuthorizationRequirement, AuthorizationThreshold, AuthorizationUse,
        ClassicOperationKind, WalletRecord, WeightedLedgerSigner,
    };
    use serde_json::Map;
    use stellar_xdr::{ManageDataOp, OperationBody, String64};

    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const SIGNER_A: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const SIGNER_B: &str = "GDRXE2BQUC3AZNPVFSCEZ76NJ3WWL25FYFK6RGZGIEKWE4SOOHSUJUJ6";
    const SIGNER_C: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";
    const SECRET_A: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const MNEMONIC_B: &str =
        "illness spike retreat truth genius clock brain pass fit cave bargain toe";
    const PASSCODE: &str = "correct horse battery staple";

    fn signer(key: &str) -> WeightedLedgerSigner {
        WeightedLedgerSigner {
            condition: LedgerSignerCondition {
                kind: LedgerSignerKind::Ed25519PublicKey,
                key: key.to_owned(),
            },
            weight: 1,
        }
    }

    fn sdk_backed_external_provider() -> ExternalEd25519SigningProvider {
        let protected = FresnicaSdk::new()
            .protect_secret(
                SECRET_A.to_owned(),
                PASSCODE.to_owned(),
                Some(SIGNER_A.to_owned()),
            )
            .unwrap();
        let envelope_json = protected.envelope_json;
        ExternalEd25519SigningProvider::new(SIGNER_A, move |request| {
            let signed = FresnicaSdk::new()
                .sign_transaction_xdr_with_passcode(
                    envelope_json.clone(),
                    PASSCODE.to_owned(),
                    SIGNER_A.to_owned(),
                    request.transaction_xdr.clone(),
                    request.network_passphrase.clone(),
                )
                .map_err(|error| error.to_string())?;
            let envelope = parse_transaction_xdr(&signed)?;
            let signatures = match envelope {
                TransactionEnvelope::TxV0(value) => value.signatures,
                TransactionEnvelope::Tx(value) => value.signatures,
                TransactionEnvelope::TxFeeBump(value) => value.signatures,
            };
            signatures
                .last()
                .map(|signature| signature.signature.0.to_vec())
                .ok_or_else(|| "SDK signer returned no transaction signature".to_owned())
        })
        .unwrap()
    }

    #[test]
    fn external_provider_satisfies_watch_only_authorization_without_passcode() {
        let root =
            std::env::temp_dir().join(format!("fresnica-external-signing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"external".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };
        let provider = sdk_backed_external_provider();

        sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[],
            &[provider],
        )
        .unwrap();

        let satisfied = satisfied_transaction_conditions(
            &plan,
            &envelope,
            network_passphrase("testnet").unwrap(),
        )
        .unwrap();
        assert!(plan.is_satisfiable_by(&satisfied));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn system_auth_provider_signs_local_software_signer_without_passphrase() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-system-auth-signing-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let record = import_secret_record("signer-a", "testnet", SECRET_A, PASSCODE).unwrap();
        storage.save(&record, false).unwrap();
        let enrollment = crate::prepare_system_auth_enrollment(&record, PASSCODE).unwrap();
        let expected_slot = enrollment.slot.storage_id();
        let unlock_key = enrollment.unlock_key().to_vec();
        let provider = SystemAuthUnlockProvider::new(SIGNER_A, move |slot| {
            if slot.storage_id() != expected_slot {
                return Err("unexpected system-auth slot".to_owned());
            }
            Ok(unlock_key.clone())
        })
        .unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"system-auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };

        sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[provider],
            &[],
        )
        .unwrap();

        let satisfied = satisfied_transaction_conditions(
            &plan,
            &envelope,
            network_passphrase("testnet").unwrap(),
        )
        .unwrap();
        assert!(plan.is_satisfiable_by(&satisfied));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn system_auth_provider_fails_closed_on_stale_unlock_key() {
        let root =
            std::env::temp_dir().join(format!("fresnica-system-auth-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let record = import_secret_record("signer-a", "testnet", SECRET_A, PASSCODE).unwrap();
        storage.save(&record, false).unwrap();
        let provider = SystemAuthUnlockProvider::new(SIGNER_A, |_| Ok(vec![0u8; 32])).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"stale".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };

        let error = sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[provider],
            &[],
        )
        .unwrap_err();
        assert!(error.contains("invalid system-auth unlock key"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn system_auth_provider_cannot_impersonate_nonlocal_signer() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-system-auth-nonlocal-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let provider = SystemAuthUnlockProvider::new(SIGNER_A, |_| Ok(vec![0u8; 32])).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"nonlocal".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };

        let error = sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[provider],
            &[],
        )
        .unwrap_err();
        assert!(error.contains("no matching local protected software signer"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn needed_provider_signing_honors_excluded_keys() {
        let root =
            std::env::temp_dir().join(format!("fresnica-external-excluded-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"excluded".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };
        let provider = sdk_backed_external_provider();
        let excluded = BTreeSet::from([SIGNER_A.to_owned()]);

        let error = sign_needed_with_ed25519_providers(
            &storage,
            &plan,
            &BTreeSet::new(),
            &excluded,
            1,
            "testnet",
            &mut envelope,
            None,
            &[],
            &[provider],
        )
        .unwrap_err();

        assert!(error.contains("cannot satisfy ledger authorization"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn needed_provider_signing_can_require_one_client_signature() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-external-minimum-proof-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"proof".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 0,
                uses: Vec::new(),
                signers: vec![WeightedLedgerSigner {
                    condition: LedgerSignerCondition {
                        kind: LedgerSignerKind::Ed25519PublicKey,
                        key: SIGNER_A.to_owned(),
                    },
                    weight: 0,
                }],
            }],
            extra_signers: BTreeSet::new(),
        };
        let provider = sdk_backed_external_provider();

        sign_needed_with_ed25519_providers(
            &storage,
            &plan,
            &BTreeSet::new(),
            &BTreeSet::new(),
            1,
            "testnet",
            &mut envelope,
            None,
            &[],
            &[provider],
        )
        .unwrap();

        let TransactionEnvelope::Tx(transaction) = envelope else {
            unreachable!();
        };
        assert_eq!(transaction.signatures.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_external_signature_is_rejected_before_authorization_is_satisfied() {
        let root =
            std::env::temp_dir().join(format!("fresnica-external-invalid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"invalid".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };
        let provider =
            ExternalEd25519SigningProvider::new(SIGNER_A, |_| Ok(vec![0u8; 64])).unwrap();

        let error = sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[],
            &[provider],
        )
        .unwrap_err();

        assert!(error.contains("invalid signature"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_external_provider_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-external-duplicate-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"duplicate".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::new(),
        };
        let first = ExternalEd25519SigningProvider::new(SIGNER_A, |_| Ok(vec![0u8; 64])).unwrap();
        let second = ExternalEd25519SigningProvider::new(SIGNER_A, |_| Ok(vec![0u8; 64])).unwrap();

        let error = sign_with_ed25519_providers(
            &storage,
            &plan,
            "testnet",
            &mut envelope,
            None,
            &[],
            &[first, second],
        )
        .unwrap_err();

        assert!(error.contains("duplicate external Ed25519 signer provider"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signs_watch_only_account_with_two_local_signer_records() {
        let root = std::env::temp_dir().join(format!(
            "fresnica-signing-coordination-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        storage
            .save(
                &WalletRecord {
                    name: "account".to_owned(),
                    address: ACCOUNT.to_owned(),
                    wallet_type: "watch-only".to_owned(),
                    network: "testnet".to_owned(),
                    secret: None,
                    metadata: Map::new(),
                },
                false,
            )
            .unwrap();
        let signer_a = import_secret_record("signer-a", "testnet", SECRET_A, PASSCODE).unwrap();
        let signer_b = import_mnemonic_record(
            "signer-b",
            "testnet",
            MNEMONIC_B,
            "",
            0,
            Some("english"),
            PASSCODE,
        )
        .unwrap();
        assert_eq!(signer_a.address, SIGNER_A);
        assert_eq!(signer_b.address, SIGNER_B);
        storage.save(&signer_a, false).unwrap();
        storage.save(&signer_b, false).unwrap();

        let mut envelope = build_operation_envelope(
            ACCOUNT,
            vec![OperationBody::ManageData(ManageDataOp {
                data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 2,
                uses: vec![AuthorizationUse {
                    scope: crate::AuthorizationScope::Operation {
                        index: 0,
                        kind: ClassicOperationKind::ManageData,
                    },
                    threshold: AuthorizationThreshold::Medium,
                    required_weight: 2,
                }],
                signers: vec![signer(SIGNER_A), signer(SIGNER_B)],
            }],
            extra_signers: BTreeSet::new(),
        };

        sign_with_local_ed25519(&storage, &plan, "testnet", &mut envelope, PASSCODE).unwrap();

        let satisfied = satisfied_transaction_conditions(
            &plan,
            &envelope,
            network_passphrase("testnet").unwrap(),
        )
        .unwrap();
        assert!(plan.is_satisfiable_by(&satisfied));
        let TransactionEnvelope::Tx(transaction) = envelope else {
            unreachable!();
        };
        assert_eq!(transaction.signatures.len(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selects_only_enough_local_ed25519_keys() {
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 2,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A), signer(SIGNER_B), signer(SIGNER_C)],
            }],
            extra_signers: BTreeSet::new(),
        };
        let local = BTreeSet::from([
            SIGNER_A.to_owned(),
            SIGNER_B.to_owned(),
            SIGNER_C.to_owned(),
        ]);

        let selected = select_local_ed25519_signers(&plan, &BTreeSet::new(), &local, 0).unwrap();

        assert_eq!(selected.len(), 2);
        let selected = selected
            .into_iter()
            .map(|key| LedgerSignerCondition {
                kind: LedgerSignerKind::Ed25519PublicKey,
                key,
            })
            .collect();
        assert!(plan.is_satisfiable_by(&selected));
    }

    #[test]
    fn selects_required_local_extra_signer() {
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 1,
                uses: Vec::new(),
                signers: vec![signer(SIGNER_A)],
            }],
            extra_signers: BTreeSet::from([LedgerSignerCondition {
                kind: LedgerSignerKind::Ed25519PublicKey,
                key: SIGNER_C.to_owned(),
            }]),
        };
        let local = BTreeSet::from([SIGNER_A.to_owned(), SIGNER_C.to_owned()]);

        let selected = select_local_ed25519_signers(&plan, &BTreeSet::new(), &local, 0).unwrap();

        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&SIGNER_A.to_owned()));
        assert!(selected.contains(&SIGNER_C.to_owned()));
    }

    #[test]
    fn unsupported_extra_signer_fails_closed() {
        let plan = LedgerAuthorizationPlan {
            requirements: Vec::new(),
            extra_signers: BTreeSet::from([LedgerSignerCondition {
                kind: LedgerSignerKind::HashX,
                key: "XUNAVAILABLE".to_owned(),
            }]),
        };

        assert!(
            select_local_ed25519_signers(&plan, &BTreeSet::new(), &BTreeSet::new(), 0,)
                .unwrap_err()
                .contains("cannot satisfy required extra signer")
        );
    }

    #[test]
    fn minimum_signature_proof_can_use_zero_weight_signer() {
        let plan = LedgerAuthorizationPlan {
            requirements: vec![AccountAuthorizationRequirement {
                account_id: ACCOUNT.to_owned(),
                required_weight: 0,
                uses: Vec::new(),
                signers: vec![WeightedLedgerSigner {
                    condition: LedgerSignerCondition {
                        kind: LedgerSignerKind::Ed25519PublicKey,
                        key: SIGNER_A.to_owned(),
                    },
                    weight: 0,
                }],
            }],
            extra_signers: BTreeSet::new(),
        };
        let local = BTreeSet::from([SIGNER_A.to_owned()]);

        assert_eq!(
            select_local_ed25519_signers(&plan, &BTreeSet::new(), &local, 1).unwrap(),
            vec![SIGNER_A]
        );
    }
}
