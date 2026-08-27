use std::collections::{BTreeMap, BTreeSet};

use fresnica_core::transaction_envelope_xdr;
use stellar_xdr::TransactionEnvelope;

use crate::ledger_authorization::{
    satisfied_transaction_conditions, LedgerAuthorizationPlan, LedgerSignerCondition,
    LedgerSignerKind, WeightedLedgerSigner,
};
use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction::{
    network_passphrase, parse_transaction_xdr, sign_transaction_xdr_with_passcode,
};

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
        let transaction_xdr = transaction_envelope_xdr(envelope)
            .map_err(|error| format!("Unable to encode transaction before signing: {error}"))?;
        *envelope = parse_transaction_xdr(&sign_transaction_xdr_with_passcode(
            record,
            network,
            transaction_xdr,
            passcode.to_owned(),
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
    let mut current = satisfied.clone();
    let mut selected = Vec::new();

    for requirement in &plan.requirements {
        while requirement.available_weight(&current) < u32::from(requirement.required_weight) {
            let available = requirement.available_weight(&current);
            let signer = best_local_signer(requirement.signers.iter(), &current, local_signers)
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
        let signer = best_local_signer(
            plan.requirements
                .iter()
                .flat_map(|requirement| requirement.signers.iter()),
            &current,
            local_signers,
        )
        .ok_or_else(|| {
            "Signing Coordination cannot provide the required Ed25519 proof".to_owned()
        })?;
        current.insert(signer.condition.clone());
        selected.push(signer.condition.key.clone());
    }
    Ok(selected)
}

fn best_local_signer<'a>(
    signers: impl Iterator<Item = &'a WeightedLedgerSigner>,
    current: &BTreeSet<LedgerSignerCondition>,
    local_signers: &BTreeSet<String>,
) -> Option<&'a WeightedLedgerSigner> {
    signers
        .filter(|signer| {
            signer.condition.kind == LedgerSignerKind::Ed25519PublicKey
                && local_signers.contains(&signer.condition.key)
                && !current.contains(&signer.condition)
        })
        .max_by_key(|signer| signer.weight)
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
    const PASSCODE: &str = "passcode";

    fn signer(key: &str) -> WeightedLedgerSigner {
        WeightedLedgerSigner {
            condition: LedgerSignerCondition {
                kind: LedgerSignerKind::Ed25519PublicKey,
                key: key.to_owned(),
            },
            weight: 1,
        }
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
        };
        let local = BTreeSet::from([SIGNER_A.to_owned()]);

        assert_eq!(
            select_local_ed25519_signers(&plan, &BTreeSet::new(), &local, 1).unwrap(),
            vec![SIGNER_A]
        );
    }
}
