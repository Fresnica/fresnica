use serde_json::Value as JsonValue;

use crate::ledger_authorization::{LedgerAccountAuthorization, WeightedLedgerSigner};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountThresholds {
    pub low: u8,
    pub medium: u8,
    pub high: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountState {
    pub account_id: String,
    pub sequence: i64,
    pub subentry_count: u32,
    pub num_sponsoring: u32,
    pub num_sponsored: u32,
    pub home_domain: Option<String>,
    pub thresholds: AccountThresholds,
    pub signers: Vec<WeightedLedgerSigner>,
}

impl AccountState {
    pub(crate) fn from_horizon(account: &JsonValue) -> Result<Self, String> {
        let LedgerAccountAuthorization {
            account_id,
            low_threshold,
            medium_threshold,
            high_threshold,
            signers,
        } = LedgerAccountAuthorization::from_horizon(account).map_err(|error| {
            format!("Unable to normalize Horizon account authorization: {error}")
        })?;

        Ok(Self {
            account_id,
            sequence: horizon_i64(account, "sequence")?,
            subentry_count: horizon_u32(account, "subentry_count")?,
            num_sponsoring: horizon_u32(account, "num_sponsoring")?,
            num_sponsored: horizon_u32(account, "num_sponsored")?,
            home_domain: account
                .get("home_domain")
                .and_then(JsonValue::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            thresholds: AccountThresholds {
                low: low_threshold,
                medium: medium_threshold,
                high: high_threshold,
            },
            signers,
        })
    }

    pub fn authorization(&self) -> LedgerAccountAuthorization {
        LedgerAccountAuthorization {
            account_id: self.account_id.clone(),
            low_threshold: self.thresholds.low,
            medium_threshold: self.thresholds.medium,
            high_threshold: self.thresholds.high,
            signers: self.signers.clone(),
        }
    }
}

fn horizon_i64(account: &JsonValue, field: &str) -> Result<i64, String> {
    let value = account
        .get(field)
        .ok_or_else(|| format!("Horizon account is missing {field}"))?;
    match value {
        JsonValue::String(value) => value
            .parse::<i64>()
            .map_err(|_| format!("Horizon account has invalid {field}")),
        JsonValue::Number(value) => value
            .as_i64()
            .ok_or_else(|| format!("Horizon account has invalid {field}")),
        _ => Err(format!("Horizon account has invalid {field}")),
    }
}

fn horizon_u32(account: &JsonValue, field: &str) -> Result<u32, String> {
    let value = account
        .get(field)
        .ok_or_else(|| format!("Horizon account is missing {field}"))?;
    let value = match value {
        JsonValue::String(value) => value
            .parse::<u64>()
            .map_err(|_| format!("Horizon account has invalid {field}"))?,
        JsonValue::Number(value) => value
            .as_u64()
            .ok_or_else(|| format!("Horizon account has invalid {field}"))?,
        _ => return Err(format!("Horizon account has invalid {field}")),
    };
    u32::try_from(value).map_err(|_| format!("Horizon account has invalid {field}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::LedgerSignerKind;

    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const SIGNER: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn horizon_account() -> JsonValue {
        json!({
            "account_id": ACCOUNT,
            "sequence": "42",
            "subentry_count": 7,
            "num_sponsoring": 2,
            "num_sponsored": 1,
            "home_domain": " example.com ",
            "thresholds": {
                "low_threshold": 1,
                "med_threshold": 2,
                "high_threshold": 3
            },
            "signers": [
                {"key": ACCOUNT, "weight": 1, "type": "ed25519_public_key"},
                {"key": SIGNER, "weight": 2, "type": "ed25519_public_key"}
            ]
        })
    }

    #[test]
    fn horizon_account_normalizes_to_provider_neutral_state() {
        let raw = horizon_account();
        let state = AccountState::from_horizon(&raw).unwrap();

        assert_eq!(state.account_id, ACCOUNT);
        assert_eq!(state.sequence, 42);
        assert_eq!(state.subentry_count, 7);
        assert_eq!(state.num_sponsoring, 2);
        assert_eq!(state.num_sponsored, 1);
        assert_eq!(state.home_domain.as_deref(), Some("example.com"));
        assert_eq!(
            state.thresholds,
            AccountThresholds {
                low: 1,
                medium: 2,
                high: 3
            }
        );
        assert_eq!(state.signers.len(), 2);
        assert_eq!(
            state.signers[0].condition.kind,
            LedgerSignerKind::Ed25519PublicKey
        );
        assert_eq!(
            state.authorization(),
            LedgerAccountAuthorization::from_horizon(&raw).unwrap()
        );
    }

    #[test]
    fn horizon_account_rejects_invalid_sequence() {
        let mut raw = horizon_account();
        raw["sequence"] = json!("not-a-sequence");

        assert_eq!(
            AccountState::from_horizon(&raw).unwrap_err(),
            "Horizon account has invalid sequence"
        );
    }

    #[test]
    fn empty_home_domain_normalizes_to_none() {
        let mut raw = horizon_account();
        raw["home_domain"] = json!("");

        assert_eq!(AccountState::from_horizon(&raw).unwrap().home_domain, None);
    }
}
