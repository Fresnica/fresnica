use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use fresnica_core::{transaction_envelope_has_valid_signature, transaction_hash};
use serde_json::Value as JsonValue;
use stellar_xdr::{
    AccountId, MuxedAccount, OperationBody, Preconditions, PublicKey, SignerKey,
    TransactionEnvelope,
};

use crate::horizon::HorizonClient;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationThreshold {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LedgerSignerKind {
    Ed25519PublicKey,
    PreauthorizedTransaction,
    HashX,
    Ed25519SignedPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LedgerSignerCondition {
    pub kind: LedgerSignerKind,
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeightedLedgerSigner {
    pub condition: LedgerSignerCondition,
    pub weight: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerAccountAuthorization {
    pub account_id: String,
    pub low_threshold: u8,
    pub medium_threshold: u8,
    pub high_threshold: u8,
    pub signers: Vec<WeightedLedgerSigner>,
}

impl LedgerAccountAuthorization {
    pub fn from_horizon(account: &JsonValue) -> Result<Self, String> {
        let account_id = json_string(account, "account_id", "Horizon account authorization")?;
        AccountId::from_str(account_id)
            .map_err(|_| "Horizon account authorization has an invalid account_id".to_owned())?;

        let thresholds = account
            .get("thresholds")
            .ok_or_else(|| "Horizon account authorization is missing thresholds".to_owned())?;
        let low_threshold = json_u8(thresholds, "low_threshold", "Horizon account thresholds")?;
        let medium_threshold = json_u8(thresholds, "med_threshold", "Horizon account thresholds")?;
        let high_threshold = json_u8(thresholds, "high_threshold", "Horizon account thresholds")?;

        let raw_signers = account
            .get("signers")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| "Horizon account authorization is missing signers".to_owned())?;
        let mut conditions = BTreeSet::new();
        let mut signers = Vec::with_capacity(raw_signers.len());
        for signer in raw_signers {
            let key = json_string(signer, "key", "Horizon signer")?.to_owned();
            let signer_type = json_string(signer, "type", "Horizon signer")?;
            let expected_kind = match signer_type {
                "ed25519_public_key" => LedgerSignerKind::Ed25519PublicKey,
                "preauth_tx" => LedgerSignerKind::PreauthorizedTransaction,
                "sha256_hash" => LedgerSignerKind::HashX,
                "ed25519_signed_payload" => LedgerSignerKind::Ed25519SignedPayload,
                other => return Err(format!("unsupported Horizon signer type: {other}")),
            };
            let actual_kind = match SignerKey::from_str(&key)
                .map_err(|_| "Horizon signer has an invalid StrKey".to_owned())?
            {
                SignerKey::Ed25519(_) => LedgerSignerKind::Ed25519PublicKey,
                SignerKey::PreAuthTx(_) => LedgerSignerKind::PreauthorizedTransaction,
                SignerKey::HashX(_) => LedgerSignerKind::HashX,
                SignerKey::Ed25519SignedPayload(_) => LedgerSignerKind::Ed25519SignedPayload,
            };
            if actual_kind != expected_kind {
                return Err(format!(
                    "Horizon signer type {signer_type} does not match signer key"
                ));
            }
            let kind = expected_kind;
            let weight = json_u8(signer, "weight", "Horizon signer")?;
            let condition = LedgerSignerCondition { kind, key };
            if !conditions.insert(condition.clone()) {
                return Err("Horizon account authorization contains a duplicate signer".to_owned());
            }
            signers.push(WeightedLedgerSigner { condition, weight });
        }

        Ok(Self {
            account_id: account_id.to_owned(),
            low_threshold,
            medium_threshold,
            high_threshold,
            signers,
        })
    }

    pub fn required_weight(&self, threshold: AuthorizationThreshold) -> u8 {
        match threshold {
            AuthorizationThreshold::Low => self.low_threshold,
            AuthorizationThreshold::Medium => self.medium_threshold,
            AuthorizationThreshold::High => self.high_threshold,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicOperationKind {
    CreateAccount,
    Payment,
    ManageSellOffer,
    ManageBuyOffer,
    ChangeTrust,
    ManageData,
    BumpSequence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizationScope {
    TransactionSource,
    Operation {
        index: usize,
        kind: ClassicOperationKind,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationUse {
    pub scope: AuthorizationScope,
    pub threshold: AuthorizationThreshold,
    pub required_weight: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAuthorizationRequirement {
    pub account_id: String,
    pub required_weight: u8,
    pub uses: Vec<AuthorizationUse>,
    pub signers: Vec<WeightedLedgerSigner>,
}

impl AccountAuthorizationRequirement {
    pub fn available_weight(&self, available: &BTreeSet<LedgerSignerCondition>) -> u32 {
        self.signers
            .iter()
            .filter(|signer| available.contains(&signer.condition))
            .map(|signer| u32::from(signer.weight))
            .sum()
    }

    pub fn is_satisfiable_by(&self, available: &BTreeSet<LedgerSignerCondition>) -> bool {
        self.available_weight(available) >= u32::from(self.required_weight)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerAuthorizationPlan {
    pub requirements: Vec<AccountAuthorizationRequirement>,
}

impl LedgerAuthorizationPlan {
    pub fn is_satisfiable_by(&self, available: &BTreeSet<LedgerSignerCondition>) -> bool {
        self.requirements
            .iter()
            .all(|requirement| requirement.is_satisfiable_by(available))
    }
}

pub fn satisfied_ed25519_conditions(
    plan: &LedgerAuthorizationPlan,
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
) -> Result<BTreeSet<LedgerSignerCondition>, String> {
    let mut satisfied = BTreeSet::new();
    for condition in signer_conditions(plan) {
        if condition.kind == LedgerSignerKind::Ed25519PublicKey
            && transaction_envelope_has_valid_signature(
                envelope,
                network_passphrase,
                &condition.key,
            )
            .map_err(|error| format!("Unable to verify existing transaction signature: {error}"))?
        {
            satisfied.insert(condition);
        }
    }
    Ok(satisfied)
}

pub fn satisfied_transaction_conditions(
    plan: &LedgerAuthorizationPlan,
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
) -> Result<BTreeSet<LedgerSignerCondition>, String> {
    let mut satisfied = satisfied_ed25519_conditions(plan, envelope, network_passphrase)?;
    let hash = transaction_hash(envelope, network_passphrase)
        .map_err(|error| format!("Unable to hash transaction for ledger authorization: {error}"))?;
    for condition in signer_conditions(plan) {
        if condition.kind == LedgerSignerKind::PreauthorizedTransaction
            && matches!(
                SignerKey::from_str(&condition.key),
                Ok(SignerKey::PreAuthTx(value)) if value.0 == hash
            )
        {
            satisfied.insert(condition);
        }
    }
    Ok(satisfied)
}

fn signer_conditions(plan: &LedgerAuthorizationPlan) -> BTreeSet<LedgerSignerCondition> {
    plan.requirements
        .iter()
        .flat_map(|requirement| &requirement.signers)
        .map(|signer| signer.condition.clone())
        .collect()
}

pub fn plan_classic_ledger_authorization(
    envelope: &TransactionEnvelope,
    accounts: &[LedgerAccountAuthorization],
) -> Result<LedgerAuthorizationPlan, String> {
    let uses = classic_authorization_uses(envelope)?;
    let mut account_index = BTreeMap::new();
    for account in accounts {
        if account_index
            .insert(account.account_id.as_str(), account)
            .is_some()
        {
            return Err(format!(
                "duplicate ledger authorization state for {}",
                account.account_id
            ));
        }
    }

    let mut requirements = Vec::new();
    for authorization_use in uses {
        add_requirement(
            &mut requirements,
            &account_index,
            &authorization_use.account_id,
            authorization_use.scope,
            authorization_use.threshold,
        )?;
    }
    Ok(LedgerAuthorizationPlan { requirements })
}

pub fn load_classic_ledger_authorization_plan(
    horizon: &HorizonClient,
    envelope: &TransactionEnvelope,
) -> Result<LedgerAuthorizationPlan, String> {
    load_classic_ledger_authorization_plan_with(envelope, |account_id| {
        horizon.get_account(account_id)
    })
}

#[derive(Clone, Debug)]
struct RequiredAuthorizationUse {
    account_id: String,
    scope: AuthorizationScope,
    threshold: AuthorizationThreshold,
}

fn classic_authorization_uses(
    envelope: &TransactionEnvelope,
) -> Result<Vec<RequiredAuthorizationUse>, String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err(
            "Ledger Authorization currently supports TransactionV1Envelope only".to_owned(),
        );
    };
    if let Preconditions::V2(preconditions) = &envelope.tx.cond {
        if !preconditions.extra_signers.is_empty() {
            return Err(
                "Ledger Authorization does not yet support PreconditionsV2.extraSigners".to_owned(),
            );
        }
    }

    let transaction_source = authorization_account_id(&envelope.tx.source_account);
    let mut uses = vec![RequiredAuthorizationUse {
        account_id: transaction_source.clone(),
        scope: AuthorizationScope::TransactionSource,
        threshold: AuthorizationThreshold::Low,
    }];
    for (index, operation) in envelope.tx.operations.iter().enumerate() {
        let source = operation
            .source_account
            .as_ref()
            .map(authorization_account_id)
            .unwrap_or_else(|| transaction_source.clone());
        let (kind, threshold) = operation_authorization(&operation.body).ok_or_else(|| {
            format!("Ledger Authorization does not yet support Classic operation #{index}")
        })?;
        uses.push(RequiredAuthorizationUse {
            account_id: source,
            scope: AuthorizationScope::Operation { index, kind },
            threshold,
        });
    }
    Ok(uses)
}

fn load_classic_ledger_authorization_plan_with<F>(
    envelope: &TransactionEnvelope,
    mut fetch_account: F,
) -> Result<LedgerAuthorizationPlan, String>
where
    F: FnMut(&str) -> Result<JsonValue, String>,
{
    let uses = classic_authorization_uses(envelope)?;
    let mut source_accounts = BTreeSet::new();
    for authorization_use in &uses {
        source_accounts.insert(authorization_use.account_id.clone());
    }

    let mut accounts = Vec::with_capacity(source_accounts.len());
    for account_id in source_accounts {
        let raw = fetch_account(&account_id)?;
        let account = LedgerAccountAuthorization::from_horizon(&raw).map_err(|error| {
            format!("Unable to interpret ledger authorization for {account_id}: {error}")
        })?;
        if account.account_id != account_id {
            return Err(format!(
                "Horizon returned ledger authorization for {} while loading {account_id}",
                account.account_id
            ));
        }
        accounts.push(account);
    }
    plan_classic_ledger_authorization(envelope, &accounts)
}

fn add_requirement(
    requirements: &mut Vec<AccountAuthorizationRequirement>,
    accounts: &BTreeMap<&str, &LedgerAccountAuthorization>,
    account_id: &str,
    scope: AuthorizationScope,
    threshold: AuthorizationThreshold,
) -> Result<(), String> {
    let account = accounts.get(account_id).copied().ok_or_else(|| {
        format!("missing ledger authorization state for source account {account_id}")
    })?;
    let required_weight = account.required_weight(threshold);
    let authorization_use = AuthorizationUse {
        scope,
        threshold,
        required_weight,
    };

    if let Some(requirement) = requirements
        .iter_mut()
        .find(|requirement| requirement.account_id == account_id)
    {
        requirement.required_weight = requirement.required_weight.max(required_weight);
        requirement.uses.push(authorization_use);
    } else {
        requirements.push(AccountAuthorizationRequirement {
            account_id: account_id.to_owned(),
            required_weight,
            uses: vec![authorization_use],
            signers: account.signers.clone(),
        });
    }
    Ok(())
}

fn operation_authorization(
    operation: &OperationBody,
) -> Option<(ClassicOperationKind, AuthorizationThreshold)> {
    let medium = AuthorizationThreshold::Medium;
    match operation {
        OperationBody::CreateAccount(_) => Some((ClassicOperationKind::CreateAccount, medium)),
        OperationBody::Payment(_) => Some((ClassicOperationKind::Payment, medium)),
        OperationBody::ManageSellOffer(_) => Some((ClassicOperationKind::ManageSellOffer, medium)),
        OperationBody::ManageBuyOffer(_) => Some((ClassicOperationKind::ManageBuyOffer, medium)),
        OperationBody::ChangeTrust(_) => Some((ClassicOperationKind::ChangeTrust, medium)),
        OperationBody::ManageData(_) => Some((ClassicOperationKind::ManageData, medium)),
        OperationBody::BumpSequence(_) => Some((
            ClassicOperationKind::BumpSequence,
            AuthorizationThreshold::Low,
        )),
        _ => None,
    }
}

fn authorization_account_id(account: &MuxedAccount) -> String {
    let key = match account {
        MuxedAccount::Ed25519(ed25519) => ed25519.clone(),
        MuxedAccount::MuxedEd25519(muxed) => muxed.ed25519.clone(),
    };
    format!("{}", PublicKey::PublicKeyTypeEd25519(key))
}

fn json_string<'a>(value: &'a JsonValue, field: &str, label: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{label} is missing {field}"))
}

fn json_u8(value: &JsonValue, field: &str, label: &str) -> Result<u8, String> {
    let raw = value
        .get(field)
        .and_then(JsonValue::as_u64)
        .filter(|value| *value <= u64::from(u8::MAX))
        .ok_or_else(|| format!("{label} is missing {field}"))?;
    Ok(raw as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_operation_envelope;
    use fresnica_core::{sign_transaction_envelope, SoftwareSigner};
    use stellar_xdr::{BumpSequenceOp, OperationBody, SequenceNumber, String64, Uint256};

    const ACCOUNT_A: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const ACCOUNT_B: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const SIGNER_C: &str = "GDRXE2BQUC3AZNPVFSCEZ76NJ3WWL25FYFK6RGZGIEKWE4SOOHSUJUJ6";
    const ACCOUNT_B_SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

    fn horizon_account(
        account_id: &str,
        low: u8,
        medium: u8,
        high: u8,
        signers: &[(&str, u8, &str)],
    ) -> LedgerAccountAuthorization {
        let signers = signers
            .iter()
            .map(|(key, weight, kind)| {
                serde_json::json!({"key": key, "weight": weight, "type": kind})
            })
            .collect::<Vec<_>>();
        LedgerAccountAuthorization::from_horizon(&serde_json::json!({
            "account_id": account_id,
            "thresholds": {
                "low_threshold": low,
                "med_threshold": medium,
                "high_threshold": high
            },
            "signers": signers
        }))
        .unwrap()
    }

    fn ed25519(key: &str) -> LedgerSignerCondition {
        LedgerSignerCondition {
            kind: LedgerSignerKind::Ed25519PublicKey,
            key: key.to_owned(),
        }
    }

    #[test]
    fn normalizes_typed_horizon_signers_without_flattening_identity() {
        let account = horizon_account(
            ACCOUNT_A,
            1,
            2,
            3,
            &[
                (ACCOUNT_A, 1, "ed25519_public_key"),
                (
                    "TA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUPUI",
                    2,
                    "preauth_tx",
                ),
                (
                    "XA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVLRR",
                    3,
                    "sha256_hash",
                ),
                (
                    "PA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUAAAAAQACAQDAQCQMBYIBEFAWDANBYHRAEISCMKBKFQXDAMRUGY4DUPB6IBZGM",
                    4,
                    "ed25519_signed_payload",
                ),
            ],
        );

        assert_eq!(account.account_id, ACCOUNT_A);
        assert_eq!(account.required_weight(AuthorizationThreshold::Medium), 2);
        assert_eq!(account.signers.len(), 4);
        assert_eq!(
            account.signers[1].condition.kind,
            LedgerSignerKind::PreauthorizedTransaction
        );
        assert_eq!(account.signers[2].condition.kind, LedgerSignerKind::HashX);
        assert_eq!(
            account.signers[3].condition.kind,
            LedgerSignerKind::Ed25519SignedPayload
        );
    }

    #[test]
    fn rejects_horizon_signer_type_key_mismatch() {
        let error = LedgerAccountAuthorization::from_horizon(&serde_json::json!({
            "account_id": ACCOUNT_A,
            "thresholds": {
                "low_threshold": 1,
                "med_threshold": 2,
                "high_threshold": 3
            },
            "signers": [{
                "key": ACCOUNT_A,
                "weight": 1,
                "type": "preauth_tx"
            }]
        }))
        .unwrap_err();

        assert_eq!(
            error,
            "Horizon signer type preauth_tx does not match signer key"
        );
    }

    #[test]
    fn transaction_satisfaction_recognizes_matching_preauth() {
        let envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![OperationBody::ManageData(stellar_xdr::ManageDataOp {
                data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let hash = transaction_hash(&envelope, TESTNET_PASSPHRASE).unwrap();
        let preauth = SignerKey::PreAuthTx(Uint256(hash)).to_string();
        let account = horizon_account(ACCOUNT_A, 1, 2, 3, &[(preauth.as_str(), 2, "preauth_tx")]);
        let plan = plan_classic_ledger_authorization(&envelope, &[account]).unwrap();

        let satisfied =
            satisfied_transaction_conditions(&plan, &envelope, TESTNET_PASSPHRASE).unwrap();

        assert!(plan.is_satisfiable_by(&satisfied));
        assert_eq!(satisfied.len(), 1);
        assert_eq!(
            satisfied.iter().next().unwrap().kind,
            LedgerSignerKind::PreauthorizedTransaction
        );
    }

    #[test]
    fn transaction_satisfaction_recognizes_existing_ed25519_signature() {
        let mut envelope = build_operation_envelope(
            ACCOUNT_B,
            vec![OperationBody::ManageData(stellar_xdr::ManageDataOp {
                data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        sign_transaction_envelope(
            &mut envelope,
            TESTNET_PASSPHRASE,
            &SoftwareSigner::from_secret(ACCOUNT_B_SECRET).unwrap(),
        )
        .unwrap();
        let account = horizon_account(ACCOUNT_B, 1, 1, 1, &[(ACCOUNT_B, 1, "ed25519_public_key")]);
        let plan = plan_classic_ledger_authorization(&envelope, &[account]).unwrap();

        let satisfied =
            satisfied_transaction_conditions(&plan, &envelope, TESTNET_PASSPHRASE).unwrap();

        assert!(plan.is_satisfiable_by(&satisfied));
        assert_eq!(satisfied, BTreeSet::from([ed25519(ACCOUNT_B)]));
    }

    #[test]
    fn transaction_source_low_and_medium_operation_share_one_account_requirement() {
        let account = horizon_account(
            ACCOUNT_A,
            1,
            2,
            3,
            &[
                (ACCOUNT_A, 1, "ed25519_public_key"),
                (SIGNER_C, 1, "ed25519_public_key"),
            ],
        );
        let envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![OperationBody::ManageData(stellar_xdr::ManageDataOp {
                data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();

        let plan = plan_classic_ledger_authorization(&envelope, &[account]).unwrap();
        assert_eq!(plan.requirements.len(), 1);
        let requirement = &plan.requirements[0];
        assert_eq!(requirement.required_weight, 2);
        assert_eq!(requirement.uses.len(), 2);

        let master_only = BTreeSet::from([ed25519(ACCOUNT_A)]);
        assert_eq!(requirement.available_weight(&master_only), 1);
        assert!(!requirement.is_satisfiable_by(&master_only));

        let both = BTreeSet::from([ed25519(ACCOUNT_A), ed25519(SIGNER_C)]);
        assert!(requirement.is_satisfiable_by(&both));
        assert!(plan.is_satisfiable_by(&both));
    }

    #[test]
    fn mixed_operation_sources_produce_independent_requirements() {
        let account_a =
            horizon_account(ACCOUNT_A, 1, 2, 3, &[(ACCOUNT_A, 1, "ed25519_public_key")]);
        let account_b = horizon_account(
            ACCOUNT_B,
            1,
            2,
            3,
            &[
                (ACCOUNT_B, 1, "ed25519_public_key"),
                (SIGNER_C, 1, "ed25519_public_key"),
            ],
        );
        let mut envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![
                OperationBody::ManageData(stellar_xdr::ManageDataOp {
                    data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                    data_value: None,
                }),
                OperationBody::BumpSequence(BumpSequenceOp {
                    bump_to: SequenceNumber(2),
                }),
            ],
            1,
            100,
            None,
        )
        .unwrap();
        let TransactionEnvelope::Tx(transaction) = &mut envelope else {
            unreachable!();
        };
        let mut operations: Vec<_> = transaction.tx.operations.clone().into();
        operations[0].source_account = Some(
            AccountId::from_str(ACCOUNT_B)
                .map(|account| match account.0 {
                    PublicKey::PublicKeyTypeEd25519(key) => MuxedAccount::Ed25519(key),
                })
                .unwrap(),
        );
        transaction.tx.operations = operations.try_into().unwrap();

        let plan = plan_classic_ledger_authorization(&envelope, &[account_a, account_b]).unwrap();
        assert_eq!(plan.requirements.len(), 2);
        assert_eq!(plan.requirements[0].account_id, ACCOUNT_A);
        assert_eq!(plan.requirements[0].required_weight, 1);
        assert_eq!(plan.requirements[1].account_id, ACCOUNT_B);
        assert_eq!(plan.requirements[1].required_weight, 2);

        let available = BTreeSet::from([ed25519(ACCOUNT_A), ed25519(ACCOUNT_B), ed25519(SIGNER_C)]);
        assert!(plan.is_satisfiable_by(&available));
    }

    #[test]
    fn loader_fetches_each_source_once_before_planning() {
        let envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![OperationBody::ManageData(stellar_xdr::ManageDataOp {
                data_name: String64::try_from(b"auth".to_vec()).unwrap(),
                data_value: None,
            })],
            1,
            100,
            None,
        )
        .unwrap();
        let mut fetched = Vec::new();
        let plan = load_classic_ledger_authorization_plan_with(&envelope, |account_id| {
            fetched.push(account_id.to_owned());
            Ok(serde_json::json!({
                "account_id": ACCOUNT_A,
                "thresholds": {
                    "low_threshold": 1,
                    "med_threshold": 2,
                    "high_threshold": 3
                },
                "signers": [
                    {"key": ACCOUNT_A, "weight": 2, "type": "ed25519_public_key"}
                ]
            }))
        })
        .unwrap();

        assert_eq!(fetched, vec![ACCOUNT_A]);
        assert_eq!(plan.requirements.len(), 1);
        assert_eq!(plan.requirements[0].required_weight, 2);
    }

    #[test]
    fn loader_rejects_horizon_account_identity_mismatch() {
        let envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![OperationBody::BumpSequence(BumpSequenceOp {
                bump_to: SequenceNumber(2),
            })],
            1,
            100,
            None,
        )
        .unwrap();

        let error = load_classic_ledger_authorization_plan_with(&envelope, |_| {
            Ok(serde_json::json!({
                "account_id": ACCOUNT_B,
                "thresholds": {
                    "low_threshold": 1,
                    "med_threshold": 2,
                    "high_threshold": 3
                },
                "signers": [
                    {"key": ACCOUNT_B, "weight": 1, "type": "ed25519_public_key"}
                ]
            }))
        })
        .unwrap_err();

        assert_eq!(
            error,
            format!(
                "Horizon returned ledger authorization for {ACCOUNT_B} while loading {ACCOUNT_A}"
            )
        );
    }

    #[test]
    fn missing_source_state_fails_closed() {
        let envelope = build_operation_envelope(
            ACCOUNT_A,
            vec![OperationBody::BumpSequence(BumpSequenceOp {
                bump_to: SequenceNumber(2),
            })],
            1,
            100,
            None,
        )
        .unwrap();

        assert_eq!(
            plan_classic_ledger_authorization(&envelope, &[]).unwrap_err(),
            format!("missing ledger authorization state for source account {ACCOUNT_A}")
        );
    }
}
