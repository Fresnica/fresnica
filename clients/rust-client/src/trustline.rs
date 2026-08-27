use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4, ChangeTrustAsset, ChangeTrustOp,
    OperationBody, TransactionEnvelope,
};

use crate::{
    account_sequence, balance_stroops, build_single_operation_envelope, format_stroops,
    minimum_balance_stroops, parse_stroops, resolve_signing_wallet, sign_and_submit,
    FresnicaClient, HorizonClient, TransactionSubmission, WalletRecord,
};

pub const DEFAULT_TRUSTLINE_LIMIT: &str = "708269837873.6765";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustlineAction {
    Add { limit: Option<String> },
    SetLimit { limit: String },
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustlineRequest {
    pub wallet: Option<String>,
    pub asset: String,
    pub action: TrustlineAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustlineOperation {
    Add,
    SetLimit,
    Remove,
}

impl TrustlineOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::SetLimit => "limit",
            Self::Remove => "remove",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustlineAuthorization {
    Full,
    MaintainLiabilities,
    Unauthorized,
}

impl TrustlineAuthorization {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::MaintainLiabilities => "maintain liabilities only",
            Self::Unauthorized => "unauthorized",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustlineReview {
    pub operation: TrustlineOperation,
    pub wallet_name: String,
    pub source: String,
    pub asset: String,
    pub limit: Option<String>,
    pub authorization: Option<TrustlineAuthorization>,
    pub clawback_enabled: Option<bool>,
    pub fee_xlm: String,
    pub network: String,
}

#[derive(Debug, Clone)]
pub struct PreparedTrustline {
    pub review: TrustlineReview,
    wallet: WalletRecord,
    envelope: TransactionEnvelope,
}

impl FresnicaClient {
    pub fn prepare_trustline(
        &self,
        request: &TrustlineRequest,
    ) -> Result<PreparedTrustline, String> {
        let wallet = resolve_signing_wallet(
            self.storage(),
            self.horizon(),
            self.network(),
            request.wallet.as_deref(),
        )?;
        let asset = IssuedAsset::parse(&request.asset)?;
        if asset.issuer == wallet.address {
            return Err("An asset issuer cannot create a trustline to its own asset".to_owned());
        }

        let account = self.horizon().get_account(&wallet.address)?;
        let ledger = self.horizon().get_ledger_parameters()?;
        let existing = find_trustline(&account, &asset);

        let (operation, limit, authorization, clawback_enabled) = match &request.action {
            TrustlineAction::Add { limit } => {
                if existing.is_some() {
                    return Err(format!(
                        "Trustline already exists for {}; use trust limit to change its limit",
                        asset.display()
                    ));
                }
                if !self.horizon().account_exists(&asset.issuer)? {
                    return Err(format!(
                        "Asset issuer account does not exist: {}",
                        asset.issuer
                    ));
                }
                let issuer = self.horizon().get_account(&asset.issuer)?;
                let (authorization, clawback_enabled) = initial_trustline_state(&issuer)?;
                ensure_native_capacity(
                    &account,
                    ledger.base_reserve_in_stroops,
                    ledger.base_fee_in_stroops,
                    ledger.base_reserve_in_stroops,
                )?;
                (
                    TrustlineOperation::Add,
                    parse_limit(limit.as_deref().unwrap_or(DEFAULT_TRUSTLINE_LIMIT))?,
                    Some(authorization),
                    Some(clawback_enabled),
                )
            }
            TrustlineAction::SetLimit { limit } => {
                let raw = existing.ok_or_else(|| {
                    format!(
                        "Trustline does not exist for {}; use trust add first",
                        asset.display()
                    )
                })?;
                let limit = parse_limit(limit)?;
                let committed = balance_stroops(raw, "balance")?
                    .checked_add(balance_stroops(raw, "buying_liabilities")?)
                    .ok_or_else(|| "trustline committed balance overflow".to_owned())?;
                if limit < committed {
                    return Err(format!(
                        "Trustline limit cannot be below current balance plus buying liabilities ({})",
                        format_stroops(committed)
                    ));
                }
                if !self.horizon().account_exists(&asset.issuer)? {
                    return Err(format!(
                        "Asset issuer account does not exist: {}",
                        asset.issuer
                    ));
                }
                let (authorization, clawback_enabled) = existing_trustline_state(raw)?;
                ensure_native_capacity(
                    &account,
                    ledger.base_reserve_in_stroops,
                    ledger.base_fee_in_stroops,
                    0,
                )?;
                (
                    TrustlineOperation::SetLimit,
                    limit,
                    Some(authorization),
                    Some(clawback_enabled),
                )
            }
            TrustlineAction::Remove => {
                let raw = existing
                    .ok_or_else(|| format!("Trustline does not exist for {}", asset.display()))?;
                let balance = balance_stroops(raw, "balance")?;
                let selling = balance_stroops(raw, "selling_liabilities")?;
                let buying = balance_stroops(raw, "buying_liabilities")?;
                if balance != 0 || selling != 0 || buying != 0 {
                    return Err(
                        "Trustline cannot be removed while balance or liabilities are non-zero"
                            .to_owned(),
                    );
                }
                ensure_not_used_by_liquidity_pool(self.horizon(), &account, &asset)?;
                ensure_native_capacity(
                    &account,
                    ledger.base_reserve_in_stroops,
                    ledger.base_fee_in_stroops,
                    0,
                )?;
                (TrustlineOperation::Remove, 0, None, None)
            }
        };

        let body = OperationBody::ChangeTrust(ChangeTrustOp {
            line: asset.to_xdr()?,
            limit,
        });
        let envelope = build_single_operation_envelope(
            &wallet.address,
            body,
            account_sequence(&account)?,
            ledger.base_fee_in_stroops,
            None,
        )?;
        let review = TrustlineReview {
            operation,
            wallet_name: wallet.name.clone(),
            source: wallet.address.clone(),
            asset: asset.display(),
            limit: (operation != TrustlineOperation::Remove).then(|| format_stroops(limit)),
            authorization,
            clawback_enabled,
            fee_xlm: format_stroops(i64::from(ledger.base_fee_in_stroops)),
            network: wallet.network.clone(),
        };
        Ok(PreparedTrustline {
            review,
            wallet,
            envelope,
        })
    }

    pub fn submit_trustline(
        &self,
        prepared: &PreparedTrustline,
        passcode: String,
    ) -> Result<TransactionSubmission, String> {
        let mut envelope = prepared.envelope.clone();
        sign_and_submit(
            self.storage(),
            &prepared.wallet,
            self.network(),
            &mut envelope,
            self.horizon(),
            passcode,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IssuedAsset {
    code: String,
    issuer: String,
}

impl IssuedAsset {
    fn parse(value: &str) -> Result<Self, String> {
        let (code, issuer) = value
            .split_once(':')
            .ok_or_else(|| "trustline asset must be CODE:GISSUER".to_owned())?;
        if code.is_empty()
            || code.len() > 12
            || !code.is_ascii()
            || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err("issued asset code must be 1-12 ASCII letters or digits".to_owned());
        }
        AccountId::from_str(issuer)
            .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
        Ok(Self {
            code: code.to_owned(),
            issuer: issuer.to_owned(),
        })
    }

    fn display(&self) -> String {
        format!("{}:{}", self.code, self.issuer)
    }

    fn to_xdr(&self) -> Result<ChangeTrustAsset, String> {
        let issuer = AccountId::from_str(&self.issuer)
            .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
        if self.code.len() <= 4 {
            let mut raw = [0u8; 4];
            raw[..self.code.len()].copy_from_slice(self.code.as_bytes());
            Ok(ChangeTrustAsset::CreditAlphanum4(AlphaNum4 {
                asset_code: AssetCode4(raw),
                issuer,
            }))
        } else {
            let mut raw = [0u8; 12];
            raw[..self.code.len()].copy_from_slice(self.code.as_bytes());
            Ok(ChangeTrustAsset::CreditAlphanum12(AlphaNum12 {
                asset_code: AssetCode12(raw),
                issuer,
            }))
        }
    }
}

fn find_trustline<'a>(account: &'a Value, asset: &IssuedAsset) -> Option<&'a Value> {
    account.get("balances")?.as_array()?.iter().find(|raw| {
        text(raw, "asset_type") != Some("native")
            && text(raw, "asset_type") != Some("liquidity_pool_shares")
            && text(raw, "asset_code") == Some(asset.code.as_str())
            && text(raw, "asset_issuer") == Some(asset.issuer.as_str())
    })
}

fn ensure_native_capacity(
    account: &Value,
    base_reserve: i64,
    fee: u32,
    additional_reserve: i64,
) -> Result<(), String> {
    let native = account
        .get("balances")
        .and_then(Value::as_array)
        .and_then(|balances| {
            balances
                .iter()
                .find(|raw| text(raw, "asset_type") == Some("native"))
        })
        .ok_or_else(|| "Insufficient XLM for reserve and fee: available 0".to_owned())?;
    let balance = balance_stroops(native, "balance")?;
    let selling = balance_stroops(native, "selling_liabilities")?;
    let minimum = minimum_balance_stroops(account, base_reserve)?;
    let free = balance
        .saturating_sub(selling)
        .saturating_sub(minimum)
        .max(0);
    let required = i64::from(fee)
        .checked_add(additional_reserve)
        .ok_or_else(|| "required XLM reserve overflow".to_owned())?;
    if free < required {
        return Err(format!(
            "Insufficient XLM for reserve and fee: need {}, available {}",
            format_stroops(required),
            format_stroops(free)
        ));
    }
    Ok(())
}

fn initial_trustline_state(issuer: &Value) -> Result<(TrustlineAuthorization, bool), String> {
    let flags = issuer
        .get("flags")
        .and_then(Value::as_object)
        .ok_or_else(|| "Horizon returned malformed issuer flags".to_owned())?;
    let auth_required = flags
        .get("auth_required")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Horizon returned malformed issuer authorization flags".to_owned())?;
    let clawback_enabled = flags
        .get("auth_clawback_enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Horizon returned malformed issuer clawback flags".to_owned())?;
    Ok((
        if auth_required {
            TrustlineAuthorization::Unauthorized
        } else {
            TrustlineAuthorization::Full
        },
        clawback_enabled,
    ))
}

fn existing_trustline_state(raw: &Value) -> Result<(TrustlineAuthorization, bool), String> {
    let fully_authorized = raw
        .get("is_authorized")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Horizon returned malformed trustline authorization state".to_owned())?;
    let maintain_liabilities = raw
        .get("is_authorized_to_maintain_liabilities")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Horizon returned malformed trustline authorization state".to_owned())?;
    let clawback_enabled = raw
        .get("is_clawback_enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Horizon returned malformed trustline clawback state".to_owned())?;
    let authorization = if fully_authorized {
        TrustlineAuthorization::Full
    } else if maintain_liabilities {
        TrustlineAuthorization::MaintainLiabilities
    } else {
        TrustlineAuthorization::Unauthorized
    };
    Ok((authorization, clawback_enabled))
}

fn ensure_not_used_by_liquidity_pool(
    horizon: &HorizonClient,
    account: &Value,
    asset: &IssuedAsset,
) -> Result<(), String> {
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
    for raw in balances {
        if text(raw, "asset_type") != Some("liquidity_pool_shares") {
            continue;
        }
        let pool_id = text(raw, "liquidity_pool_id")
            .ok_or_else(|| "Horizon returned malformed liquidity-pool balance".to_owned())?;
        let pool = horizon.get_liquidity_pool(pool_id)?;
        if liquidity_pool_uses_asset(&pool, asset)? {
            return Err(format!(
                "Trustline cannot be removed while liquidity pool {pool_id} uses {}",
                asset.display()
            ));
        }
    }
    Ok(())
}

fn liquidity_pool_uses_asset(pool: &Value, asset: &IssuedAsset) -> Result<bool, String> {
    let reserves = pool
        .get("reserves")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed liquidity-pool reserves".to_owned())?;
    let identity = asset.display();
    Ok(reserves
        .iter()
        .any(|reserve| text(reserve, "asset") == Some(identity.as_str())))
}

fn parse_limit(value: &str) -> Result<i64, String> {
    parse_stroops(value, true).map_err(|_| {
        "Trustline limit must be greater than zero with at most 7 decimal places".to_owned()
    })
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn default_limit_matches_reference_policy() {
        assert_eq!(
            parse_limit(DEFAULT_TRUSTLINE_LIMIT).unwrap(),
            7_082_698_378_736_765_000
        );
    }

    #[test]
    fn parses_four_and_twelve_character_assets() {
        assert_eq!(
            IssuedAsset::parse(&format!("USD:{ISSUER}"))
                .unwrap()
                .display(),
            format!("USD:{ISSUER}")
        );
        assert!(IssuedAsset::parse(&format!("LONGASSET12:{ISSUER}"))
            .unwrap()
            .to_xdr()
            .is_ok());
    }

    #[test]
    fn remove_precondition_observes_balance_and_liabilities() {
        let asset = IssuedAsset::parse(&format!("USD:{ISSUER}")).unwrap();
        let account = serde_json::json!({
            "balances": [
                {"asset_type":"native","balance":"5.0000000","selling_liabilities":"0"},
                {"asset_type":"credit_alphanum4","asset_code":"USD","asset_issuer":ISSUER,"balance":"1.0000000","selling_liabilities":"0","buying_liabilities":"0"}
            ]
        });
        let raw = find_trustline(&account, &asset).unwrap();
        assert_ne!(balance_stroops(raw, "balance").unwrap(), 0);
    }

    #[test]
    fn issuer_flags_define_initial_trustline_state() {
        let issuer = serde_json::json!({
            "flags": {
                "auth_required": true,
                "auth_clawback_enabled": true
            }
        });
        assert_eq!(
            initial_trustline_state(&issuer).unwrap(),
            (TrustlineAuthorization::Unauthorized, true)
        );
    }

    #[test]
    fn existing_full_authorization_takes_precedence_over_maintain_flag() {
        let raw = serde_json::json!({
            "is_authorized": true,
            "is_authorized_to_maintain_liabilities": true,
            "is_clawback_enabled": false
        });
        assert_eq!(
            existing_trustline_state(&raw).unwrap(),
            (TrustlineAuthorization::Full, false)
        );
    }

    #[test]
    fn pool_reserves_block_removal_of_referenced_asset() {
        let asset = IssuedAsset::parse(&format!("USD:{ISSUER}")).unwrap();
        let pool = serde_json::json!({
            "reserves": [
                {"asset": "native", "amount": "10.0000000"},
                {"asset": format!("USD:{ISSUER}"), "amount": "20.0000000"}
            ]
        });
        assert!(liquidity_pool_uses_asset(&pool, &asset).unwrap());
    }
}
