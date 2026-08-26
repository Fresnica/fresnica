use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, AssetCode12, AssetCode4, ChangeTrustAsset, ChangeTrustOp,
    OperationBody,
};

use fresnica_client::WalletStorage;
use crate::transaction_flow::{
    account_sequence, balance_stroops, build_single_operation_envelope, confirm_submission,
    format_stroops, minimum_balance_stroops, network_client, parse_stroops,
    resolve_signing_wallet, sign_and_submit,
};

const FRESNICA_TRUSTLINE_LIMIT: &str = "708269837873.6765";

pub fn command_trust(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = TrustRequest::parse(arguments)?;
    let record = resolve_signing_wallet(storage, network, request.wallet())?;
    let asset = IssuedAsset::parse(request.asset())?;
    if asset.issuer == record.address {
        return Err("An asset issuer cannot create a trustline to its own asset".to_owned());
    }

    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let ledger = horizon.get_ledger_parameters()?;
    let existing = find_trustline(&account, &asset);

    let (action, limit) = match &request {
        TrustRequest::Add { limit, .. } => {
            if existing.is_some() {
                return Err(format!(
                    "Trustline already exists for {}; use trust limit to change its limit",
                    asset.display()
                ));
            }
            ensure_native_capacity(
                &account,
                ledger.base_reserve_in_stroops,
                ledger.base_fee_in_stroops,
                ledger.base_reserve_in_stroops,
            )?;
            ("add", parse_limit(limit.as_deref().unwrap_or(FRESNICA_TRUSTLINE_LIMIT))?)
        }
        TrustRequest::Limit { limit, .. } => {
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
            ensure_native_capacity(
                &account,
                ledger.base_reserve_in_stroops,
                ledger.base_fee_in_stroops,
                0,
            )?;
            ("limit", limit)
        }
        TrustRequest::Remove { .. } => {
            let raw = existing.ok_or_else(|| {
                format!("Trustline does not exist for {}", asset.display())
            })?;
            let balance = balance_stroops(raw, "balance")?;
            let selling = balance_stroops(raw, "selling_liabilities")?;
            let buying = balance_stroops(raw, "buying_liabilities")?;
            if balance != 0 || selling != 0 || buying != 0 {
                return Err(
                    "Trustline cannot be removed while balance or liabilities are non-zero"
                        .to_owned(),
                );
            }
            ensure_native_capacity(
                &account,
                ledger.base_reserve_in_stroops,
                ledger.base_fee_in_stroops,
                0,
            )?;
            ("remove", 0)
        }
    };

    let body = OperationBody::ChangeTrust(ChangeTrustOp {
        line: asset.to_xdr()?,
        limit,
    });
    let mut envelope = build_single_operation_envelope(
        &record.address,
        body,
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        None,
    )?;

    println!("Review transaction");
    println!("Operation: ChangeTrust ({action})");
    println!("Wallet:    {} ({})", record.name, record.address);
    println!("Asset:     {}", asset.display());
    if action != "remove" {
        println!("Limit:     {}", format_stroops(limit));
    }
    println!(
        "Fee:       {} XLM",
        format_stroops(i64::from(ledger.base_fee_in_stroops))
    );
    println!("Network:   {}", record.network);

    if !request.yes() && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }
    sign_and_submit(&record, network, &mut envelope, &horizon)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrustRequest {
    Add {
        asset: String,
        limit: Option<String>,
        wallet: Option<String>,
        yes: bool,
    },
    Limit {
        asset: String,
        limit: String,
        wallet: Option<String>,
        yes: bool,
    },
    Remove {
        asset: String,
        wallet: Option<String>,
        yes: bool,
    },
}

impl TrustRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "add" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, limit) = parse_options(&arguments[2..], true)?;
                Ok(Self::Add {
                    asset,
                    limit,
                    wallet,
                    yes,
                })
            }
            "limit" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let limit = arguments.get(2).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra) = parse_options(&arguments[3..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                Ok(Self::Limit {
                    asset,
                    limit,
                    wallet,
                    yes,
                })
            }
            "remove" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra) = parse_options(&arguments[2..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                Ok(Self::Remove { asset, wallet, yes })
            }
            _ => Err(usage().to_owned()),
        }
    }

    fn asset(&self) -> &str {
        match self {
            Self::Add { asset, .. } | Self::Limit { asset, .. } | Self::Remove { asset, .. } => {
                asset
            }
        }
    }

    fn wallet(&self) -> Option<&str> {
        match self {
            Self::Add { wallet, .. }
            | Self::Limit { wallet, .. }
            | Self::Remove { wallet, .. } => wallet.as_deref(),
        }
    }

    fn yes(&self) -> bool {
        match self {
            Self::Add { yes, .. } | Self::Limit { yes, .. } | Self::Remove { yes, .. } => *yes,
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_limit: bool,
) -> Result<(Option<String>, bool, Option<String>), String> {
    let mut wallet = None;
    let mut yes = false;
    let mut limit = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(arguments.get(index).ok_or_else(|| usage().to_owned())?.clone());
                index += 1;
            }
            "--limit" if allow_limit => {
                index += 1;
                limit = Some(arguments.get(index).ok_or_else(|| usage().to_owned())?.clone());
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, yes, limit))
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
    account
        .get("balances")?
        .as_array()?
        .iter()
        .find(|raw| {
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

fn parse_limit(value: &str) -> Result<i64, String> {
    parse_stroops(value, true)
        .map_err(|_| "Trustline limit must be greater than zero with at most 7 decimal places".to_owned())
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn usage() -> &'static str {
    "usage: fresnica trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]\n       fresnica trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]\n       fresnica trust remove CODE:GISSUER [--wallet NAME] [-y]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn default_limit_matches_python_reference_policy() {
        assert_eq!(
            parse_limit(FRESNICA_TRUSTLINE_LIMIT).unwrap(),
            7_082_698_378_736_765_000
        );
    }

    #[test]
    fn add_parser_accepts_limit_wallet_and_yes() {
        let args = [
            "add",
            &format!("USD:{ISSUER}"),
            "--limit",
            "1000",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = TrustRequest::parse(&args).unwrap();
        assert_eq!(request.wallet(), Some("alpha"));
        assert!(request.yes());
        let TrustRequest::Add { limit, .. } = request else {
            panic!("expected add request");
        };
        assert_eq!(limit.as_deref(), Some("1000"));
    }

    #[test]
    fn remove_requires_zero_balance_and_liabilities() {
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
}
