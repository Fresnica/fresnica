use std::collections::BTreeSet;

use stellar_xdr::{AccountEntry, SignerKey};

use crate::ledger_authorization::{
    LedgerAccountAuthorization, LedgerSignerCondition, LedgerSignerKind, WeightedLedgerSigner,
};

impl LedgerAccountAuthorization {
    /// Normalize protocol-native account authorization into the same semantic view used by
    /// Horizon-backed Classic authorization planning.
    pub fn from_account_entry(account: &AccountEntry) -> Result<Self, String> {
        let account_id = account.account_id.to_string();
        let [master_weight, low_threshold, medium_threshold, high_threshold] = account.thresholds.0;

        let master_condition = LedgerSignerCondition {
            kind: LedgerSignerKind::Ed25519PublicKey,
            key: account_id.clone(),
        };
        let mut conditions = BTreeSet::from([master_condition.clone()]);
        let mut signers = Vec::with_capacity(account.signers.len() + 1);
        signers.push(WeightedLedgerSigner {
            condition: master_condition,
            weight: master_weight,
        });

        for signer in &account.signers {
            let condition = LedgerSignerCondition {
                kind: signer_kind(&signer.key),
                key: signer.key.to_string(),
            };
            if !conditions.insert(condition.clone()) {
                return Err(
                    "Stellar AccountEntry authorization contains a duplicate signer".to_owned(),
                );
            }
            let weight = u8::try_from(signer.weight)
                .map_err(|_| "Stellar AccountEntry signer weight exceeds 255".to_owned())?;
            signers.push(WeightedLedgerSigner { condition, weight });
        }

        Ok(Self {
            account_id,
            low_threshold,
            medium_threshold,
            high_threshold,
            signers,
        })
    }
}

fn signer_kind(key: &SignerKey) -> LedgerSignerKind {
    match key {
        SignerKey::Ed25519(_) => LedgerSignerKind::Ed25519PublicKey,
        SignerKey::PreAuthTx(_) => LedgerSignerKind::PreauthorizedTransaction,
        SignerKey::HashX(_) => LedgerSignerKind::HashX,
        SignerKey::Ed25519SignedPayload(_) => LedgerSignerKind::Ed25519SignedPayload,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json::json;
    use stellar_xdr::{AccountEntry, AccountId, Signer, SignerKey, Thresholds, VecM};

    use super::*;

    const ACCOUNT_A: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const SIGNER_B: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const PREAUTH: &str = "TA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUPUI";
    const HASH_X: &str = "XA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVLRR";
    const SIGNED_PAYLOAD: &str = "PA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUAAAAAQACAQDAQCQMBYIBEFAWDANBYHRAEISCMKBKFQXDAMRUGY4DUPB6IBZGM";

    fn account_entry(master_weight: u8, signers: Vec<Signer>) -> AccountEntry {
        AccountEntry {
            account_id: AccountId::from_str(ACCOUNT_A).unwrap(),
            thresholds: Thresholds([master_weight, 1, 2, 3]),
            signers: VecM::try_from(signers).unwrap(),
            ..Default::default()
        }
    }

    #[test]
    fn xdr_and_horizon_normalize_to_the_same_authorization() {
        let from_xdr = LedgerAccountAuthorization::from_account_entry(&account_entry(
            1,
            vec![Signer {
                key: SignerKey::from_str(SIGNER_B).unwrap(),
                weight: 2,
            }],
        ))
        .unwrap();
        let from_horizon = LedgerAccountAuthorization::from_horizon(&json!({
            "account_id": ACCOUNT_A,
            "thresholds": {
                "low_threshold": 1,
                "med_threshold": 2,
                "high_threshold": 3
            },
            "signers": [
                {"key": ACCOUNT_A, "weight": 1, "type": "ed25519_public_key"},
                {"key": SIGNER_B, "weight": 2, "type": "ed25519_public_key"}
            ]
        }))
        .unwrap();

        assert_eq!(from_xdr, from_horizon);
        assert_eq!(from_xdr.signers[0].condition.key, ACCOUNT_A);
        assert_eq!(from_xdr.signers[0].weight, 1);
    }

    #[test]
    fn xdr_normalization_preserves_zero_weight_master() {
        let normalized =
            LedgerAccountAuthorization::from_account_entry(&account_entry(0, Vec::new())).unwrap();

        assert_eq!(normalized.signers.len(), 1);
        assert_eq!(normalized.signers[0].condition.key, ACCOUNT_A);
        assert_eq!(normalized.signers[0].weight, 0);
    }

    #[test]
    fn xdr_normalization_preserves_typed_signer_conditions() {
        let normalized = LedgerAccountAuthorization::from_account_entry(&account_entry(
            1,
            vec![
                Signer {
                    key: SignerKey::from_str(PREAUTH).unwrap(),
                    weight: 2,
                },
                Signer {
                    key: SignerKey::from_str(HASH_X).unwrap(),
                    weight: 3,
                },
                Signer {
                    key: SignerKey::from_str(SIGNED_PAYLOAD).unwrap(),
                    weight: 4,
                },
            ],
        ))
        .unwrap();

        assert_eq!(normalized.signers.len(), 4);
        assert_eq!(
            normalized.signers[1].condition.kind,
            LedgerSignerKind::PreauthorizedTransaction
        );
        assert_eq!(
            normalized.signers[2].condition.kind,
            LedgerSignerKind::HashX
        );
        assert_eq!(
            normalized.signers[3].condition.kind,
            LedgerSignerKind::Ed25519SignedPayload
        );
    }

    #[test]
    fn xdr_normalization_rejects_out_of_range_signer_weight() {
        let error = LedgerAccountAuthorization::from_account_entry(&account_entry(
            1,
            vec![Signer {
                key: SignerKey::from_str(SIGNER_B).unwrap(),
                weight: 256,
            }],
        ))
        .unwrap_err();

        assert_eq!(error, "Stellar AccountEntry signer weight exceeds 255");
    }
}
