use serde_json::Value as JsonValue;

use crate::asset::AssetId;
use crate::transaction::{format_stroops, parse_stroops};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BalanceAsset {
    Native,
    Issued { code: String, issuer: String },
    LiquidityPoolShare { liquidity_pool_id: String },
}

impl BalanceAsset {
    pub fn identity(&self) -> String {
        match self {
            Self::Native => "XLM".to_owned(),
            Self::Issued { code, issuer } => format!("{code}:{issuer}"),
            Self::LiquidityPoolShare { liquidity_pool_id } => {
                format!("LP:{liquidity_pool_id}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetBalance {
    pub asset: BalanceAsset,
    pub balance: String,
    pub selling_liabilities: String,
    pub buying_liabilities: String,
}

impl AssetBalance {
    pub(crate) fn from_horizon(value: &JsonValue) -> Result<Self, String> {
        Ok(Self {
            asset: balance_asset(value)?,
            balance: horizon_amount(value, "balance", true)?,
            selling_liabilities: horizon_amount(value, "selling_liabilities", false)?,
            buying_liabilities: horizon_amount(value, "buying_liabilities", false)?,
        })
    }
}

fn balance_asset(value: &JsonValue) -> Result<BalanceAsset, String> {
    match text(value, "asset_type") {
        Some("native") => Ok(BalanceAsset::Native),
        Some("credit_alphanum4") | Some("credit_alphanum12") => {
            let asset = AssetId::from_horizon(value)?;
            Ok(BalanceAsset::Issued {
                code: asset.code().expect("issued Horizon asset has a code"),
                issuer: asset.issuer().expect("issued Horizon asset has an issuer"),
            })
        }
        Some("liquidity_pool_shares") => {
            let raw = text(value, "liquidity_pool_id")
                .ok_or_else(|| "Horizon liquidity-pool balance is missing liquidity_pool_id".to_owned())?;
            let liquidity_pool_id = normalize_pool_id(raw)?;
            Ok(BalanceAsset::LiquidityPoolShare { liquidity_pool_id })
        }
        Some(other) => Err(format!("Horizon returned unsupported balance asset type: {other}")),
        None => Err("Horizon balance is missing asset_type".to_owned()),
    }
}

fn horizon_amount(value: &JsonValue, field: &str, required: bool) -> Result<String, String> {
    let raw = match text(value, field) {
        Some(value) => value,
        None if !required => return Ok("0".to_owned()),
        None => return Err(format!("Horizon balance is missing {field}")),
    };
    let units = parse_stroops(raw, false)
        .map_err(|_| format!("Horizon balance has invalid {field}: {raw}"))?;
    Ok(format_stroops(units))
}

fn normalize_pool_id(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Horizon liquidity-pool balance has invalid liquidity_pool_id".to_owned());
    }
    Ok(value.to_ascii_lowercase())
}

fn text<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value.get(key).and_then(JsonValue::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn native_balance_normalizes_amounts_and_missing_liabilities() {
        let balance = AssetBalance::from_horizon(&json!({
            "asset_type": "native",
            "balance": "12.5000000"
        }))
        .unwrap();

        assert_eq!(balance.asset, BalanceAsset::Native);
        assert_eq!(balance.asset.identity(), "XLM");
        assert_eq!(balance.balance, "12.5");
        assert_eq!(balance.selling_liabilities, "0");
        assert_eq!(balance.buying_liabilities, "0");
    }

    #[test]
    fn issued_balance_preserves_exact_asset_identity() {
        let balance = AssetBalance::from_horizon(&json!({
            "asset_type": "credit_alphanum4",
            "asset_code": "USD",
            "asset_issuer": ISSUER,
            "balance": "7.0000000",
            "selling_liabilities": "1.2500000",
            "buying_liabilities": "0.5000000"
        }))
        .unwrap();

        assert_eq!(
            balance.asset,
            BalanceAsset::Issued {
                code: "USD".to_owned(),
                issuer: ISSUER.to_owned()
            }
        );
        assert_eq!(balance.asset.identity(), format!("USD:{ISSUER}"));
        assert_eq!(balance.balance, "7");
        assert_eq!(balance.selling_liabilities, "1.25");
        assert_eq!(balance.buying_liabilities, "0.5");
    }

    #[test]
    fn liquidity_pool_share_has_distinct_identity() {
        let pool_id = "A1".repeat(32);
        let balance = AssetBalance::from_horizon(&json!({
            "asset_type": "liquidity_pool_shares",
            "liquidity_pool_id": pool_id,
            "balance": "3.0000000"
        }))
        .unwrap();

        let expected = "a1".repeat(32);
        assert_eq!(
            balance.asset,
            BalanceAsset::LiquidityPoolShare {
                liquidity_pool_id: expected.clone()
            }
        );
        assert_eq!(balance.asset.identity(), format!("LP:{expected}"));
    }

    #[test]
    fn balance_rejects_more_than_seven_decimal_places() {
        let error = AssetBalance::from_horizon(&json!({
            "asset_type": "native",
            "balance": "1.00000001"
        }))
        .unwrap_err();

        assert_eq!(error, "Horizon balance has invalid balance: 1.00000001");
    }

    #[test]
    fn balance_rejects_invalid_pool_id() {
        let error = AssetBalance::from_horizon(&json!({
            "asset_type": "liquidity_pool_shares",
            "liquidity_pool_id": "not-a-pool-id",
            "balance": "1"
        }))
        .unwrap_err();

        assert_eq!(
            error,
            "Horizon liquidity-pool balance has invalid liquidity_pool_id"
        );
    }
}
