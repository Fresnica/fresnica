use std::collections::BTreeSet;

use crate::ledger_authorization::{
    LedgerAuthorizationPlan, LedgerSignerCondition, LedgerSignerKind,
};

pub fn select_local_ed25519_signers(
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    local_signers: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let mut current = satisfied.clone();
    let mut remaining = local_signers.clone();
    let mut selected = Vec::new();

    while !plan.is_satisfiable_by(&current) {
        let next = remaining
            .iter()
            .map(|key| (key, signer_gain(plan, &current, key)))
            .filter(|(_, gain)| *gain > 0)
            .max_by(|(left_key, left_gain), (right_key, right_gain)| {
                left_gain
                    .cmp(right_gain)
                    .then_with(|| right_key.cmp(left_key))
            });
        let Some((key, _)) = next else {
            return Err(format!(
                "Signing Coordination cannot satisfy ledger authorization: {}",
                authorization_gaps(plan, &current).join("; ")
            ));
        };
        let key = key.clone();
        remaining.remove(&key);
        current.insert(LedgerSignerCondition {
            kind: LedgerSignerKind::Ed25519PublicKey,
            key: key.clone(),
        });
        selected.push(key);
    }
    Ok(selected)
}

fn signer_gain(
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
    key: &str,
) -> u32 {
    let condition = LedgerSignerCondition {
        kind: LedgerSignerKind::Ed25519PublicKey,
        key: key.to_owned(),
    };
    plan.requirements
        .iter()
        .map(|requirement| {
            let available = requirement.available_weight(satisfied);
            let missing = u32::from(requirement.required_weight).saturating_sub(available);
            requirement
                .signers
                .iter()
                .find(|signer| signer.condition == condition)
                .map(|signer| missing.min(u32::from(signer.weight)))
                .unwrap_or(0)
        })
        .sum()
}

fn authorization_gaps(
    plan: &LedgerAuthorizationPlan,
    satisfied: &BTreeSet<LedgerSignerCondition>,
) -> Vec<String> {
    plan.requirements
        .iter()
        .filter_map(|requirement| {
            let available = requirement.available_weight(satisfied);
            (available < u32::from(requirement.required_weight)).then(|| {
                format!(
                    "{} requires weight {} but has {}",
                    requirement.account_id, requirement.required_weight, available
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountAuthorizationRequirement, WeightedLedgerSigner};

    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const SIGNER_A: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const SIGNER_B: &str = "GDRXE2BQUC3AZNPVFSCEZ76NJ3WWL25FYFK6RGZGIEKWE4SOOHSUJUJ6";
    const SIGNER_C: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";

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

        let selected = select_local_ed25519_signers(&plan, &BTreeSet::new(), &local).unwrap();

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
}
