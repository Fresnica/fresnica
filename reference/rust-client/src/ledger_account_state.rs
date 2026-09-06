use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value as JsonValue;

use crate::account_state::AccountState;
use crate::asset::AssetId;
use crate::balance_state::{AssetBalance, BalanceAsset};
use crate::transaction::parse_stroops;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LedgerTrustlineAuthorization {
    Full,
    MaintainLiabilities,
    Unauthorized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LedgerBalanceState {
    pub(crate) asset: BalanceAsset,
    pub(crate) balance: i64,
    pub(crate) selling_liabilities: i64,
    pub(crate) buying_liabilities: i64,
    pub(crate) limit: Option<i64>,
    pub(crate) authorization: Option<LedgerTrustlineAuthorization>,
    pub(crate) clawback_enabled: Option<bool>,
}

impl LedgerBalanceState {
    fn from_horizon(value: &JsonValue) -> Result<Self, String> {
        let normalized = AssetBalance::from_horizon(value)?;
        let balance = normalized_amount(&normalized.balance, "balance")?;
        let selling_liabilities =
            normalized_amount(&normalized.selling_liabilities, "selling_liabilities")?;
        let buying_liabilities =
            normalized_amount(&normalized.buying_liabilities, "buying_liabilities")?;

        let (limit, authorization, clawback_enabled) = match &normalized.asset {
            BalanceAsset::Issued { .. } => {
                let limit = horizon_amount(value, "limit")?;
                let fully_authorized = horizon_bool(value, "is_authorized")?;
                let maintain_liabilities =
                    horizon_bool(value, "is_authorized_to_maintain_liabilities")?;
                let clawback_enabled = horizon_bool(value, "is_clawback_enabled")?;
                let authorization = if fully_authorized {
                    LedgerTrustlineAuthorization::Full
                } else if maintain_liabilities {
                    LedgerTrustlineAuthorization::MaintainLiabilities
                } else {
                    LedgerTrustlineAuthorization::Unauthorized
                };
                (Some(limit), Some(authorization), Some(clawback_enabled))
            }
            BalanceAsset::Native | BalanceAsset::LiquidityPoolShare { .. } => (None, None, None),
        };

        Ok(Self {
            asset: normalized.asset,
            balance,
            selling_liabilities,
            buying_liabilities,
            limit,
            authorization,
            clawback_enabled,
        })
    }

    pub(crate) fn available_after_selling_liabilities(&self) -> i64 {
        self.balance.saturating_sub(self.selling_liabilities).max(0)
    }

    pub(crate) fn committed_for_receiving(&self) -> Result<i64, String> {
        self.balance
            .checked_add(self.buying_liabilities)
            .ok_or_else(|| "ledger balance receiving capacity overflow".to_owned())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LedgerAccountFlags {
    pub(crate) auth_required: Option<bool>,
    pub(crate) auth_clawback_enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LedgerAccountState {
    pub(crate) account: AccountState,
    pub(crate) balances: Vec<LedgerBalanceState>,
    pub(crate) flags: LedgerAccountFlags,
    pub(crate) memo_required: bool,
}

impl LedgerAccountState {
    pub(crate) fn from_horizon(value: &JsonValue) -> Result<Self, String> {
        let account = AccountState::from_horizon(value)?;
        let raw_balances = value
            .get("balances")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
        let balances = raw_balances
            .iter()
            .map(LedgerBalanceState::from_horizon)
            .collect::<Result<Vec<_>, _>>()?;
        let flags = LedgerAccountFlags {
            auth_required: horizon_optional_flag(value, "auth_required")?,
            auth_clawback_enabled: horizon_optional_flag(value, "auth_clawback_enabled")?,
        };
        let memo_required = horizon_memo_required(value)?;
        Ok(Self {
            account,
            balances,
            flags,
            memo_required,
        })
    }

    pub(crate) fn balance(&self, asset: &AssetId) -> Option<&LedgerBalanceState> {
        self.balances.iter().find(|balance| match &balance.asset {
            BalanceAsset::Native => asset.is_native(),
            BalanceAsset::Issued { code, issuer } => {
                !asset.is_native()
                    && asset.code().as_deref() == Some(code.as_str())
                    && asset.issuer().as_deref() == Some(issuer.as_str())
            }
            BalanceAsset::LiquidityPoolShare { .. } => false,
        })
    }

    pub(crate) fn minimum_balance_stroops(&self, base_reserve: i64) -> Result<i64, String> {
        let units = 2_i64
            .saturating_add(i64::from(self.account.subentry_count))
            .saturating_add(i64::from(self.account.num_sponsoring))
            .saturating_sub(i64::from(self.account.num_sponsored))
            .max(0);
        units
            .checked_mul(base_reserve)
            .ok_or_else(|| "minimum balance overflow".to_owned())
    }
}

fn normalized_amount(value: &str, field: &str) -> Result<i64, String> {
    parse_stroops(value, false)
        .map_err(|_| format!("normalized ledger balance has invalid {field}: {value}"))
}

fn horizon_amount(value: &JsonValue, field: &str) -> Result<i64, String> {
    let raw = value
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("Horizon trustline is missing {field}"))?;
    parse_stroops(raw, false).map_err(|_| format!("Horizon trustline has invalid {field}: {raw}"))
}

fn horizon_bool(value: &JsonValue, field: &str) -> Result<bool, String> {
    value
        .get(field)
        .and_then(JsonValue::as_bool)
        .ok_or_else(|| format!("Horizon trustline has invalid {field}"))
}

fn horizon_optional_flag(value: &JsonValue, field: &str) -> Result<Option<bool>, String> {
    let Some(flags) = value.get("flags") else {
        return Ok(None);
    };
    let flags = flags
        .as_object()
        .ok_or_else(|| "Horizon account has malformed flags".to_owned())?;
    match flags.get(field) {
        None => Ok(None),
        Some(JsonValue::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("Horizon account has invalid {field} flag")),
    }
}

fn horizon_memo_required(value: &JsonValue) -> Result<bool, String> {
    let Some(encoded) = value
        .get("data")
        .and_then(JsonValue::as_object)
        .and_then(|data| data.get("config.memo_required"))
        .and_then(JsonValue::as_str)
    else {
        return Ok(false);
    };
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| "Horizon returned malformed config.memo_required data".to_owned())?;
    Ok(decoded.as_slice() == b"1")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const ISSUER: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";

    fn account_json() -> JsonValue {
        json!({
            "account_id": ACCOUNT,
            "sequence": "42",
            "subentry_count": 2,
            "num_sponsoring": 1,
            "num_sponsored": 0,
            "thresholds": {
                "low_threshold": 1,
                "med_threshold": 2,
                "high_threshold": 3
            },
            "signers": [{"key": ACCOUNT, "weight": 1, "type": "ed25519_public_key"}],
            "flags": {
                "auth_required": true,
                "auth_clawback_enabled": true
            },
            "data": {"config.memo_required": "MQ=="},
            "balances": [
                {
                    "asset_type": "native",
                    "balance": "10.0000000",
                    "selling_liabilities": "1.0000000",
                    "buying_liabilities": "0.5000000"
                },
                {
                    "asset_type": "credit_alphanum4",
                    "asset_code": "USD",
                    "asset_issuer": ISSUER,
                    "balance": "7.0000000",
                    "selling_liabilities": "1.2500000",
                    "buying_liabilities": "0.5000000",
                    "limit": "1000.0000000",
                    "is_authorized": false,
                    "is_authorized_to_maintain_liabilities": true,
                    "is_clawback_enabled": true
                }
            ]
        })
    }

    #[test]
    fn horizon_account_normalizes_write_preflight_state() {
        let state = LedgerAccountState::from_horizon(&account_json()).unwrap();
        assert_eq!(state.account.sequence, 42);
        assert!(state.memo_required);
        assert_eq!(state.flags.auth_required, Some(true));
        assert_eq!(state.flags.auth_clawback_enabled, Some(true));
        assert_eq!(state.minimum_balance_stroops(5_000_000).unwrap(), 25_000_000);

        let native = state.balance(&AssetId::native()).unwrap();
        assert_eq!(native.balance, 100_000_000);
        assert_eq!(native.available_after_selling_liabilities(), 90_000_000);

        let usd = AssetId::parse(&format!("USD:{ISSUER}")).unwrap();
        let trustline = state.balance(&usd).unwrap();
        assert_eq!(trustline.limit, Some(10_000_000_000));
        assert_eq!(
            trustline.authorization,
            Some(LedgerTrustlineAuthorization::MaintainLiabilities)
        );
        assert_eq!(trustline.clawback_enabled, Some(true));
    }

    #[test]
    fn malformed_memo_required_data_fails_at_provider_normalization_boundary() {
        let mut value = account_json();
        value["data"]["config.memo_required"] = json!("not base64 !!");
        assert_eq!(
            LedgerAccountState::from_horizon(&value).unwrap_err(),
            "Horizon returned malformed config.memo_required data"
        );
    }
}
