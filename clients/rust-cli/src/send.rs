use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, CreateAccountOp,
    MuxedAccount, OperationBody, PaymentOp, PublicKey,
};

use crate::horizon::LedgerParameters;
use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction_flow::{
    account_sequence, balance_stroops, build_single_operation_envelope, confirm_submission,
    format_stroops, minimum_balance_stroops, network_client, parse_positive_stroops,
    resolve_signing_wallet, sign_and_submit,
};

pub fn command_send(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = SendRequest::parse(arguments)?;
    let record = resolve_signing_wallet(storage, network, request.wallet.as_deref())?;
    let asset = PaymentAsset::parse(&request.asset)?;
    let amount = parse_positive_stroops(&request.amount)?;
    let destination = AccountId::from_str(&request.destination)
        .map_err(|_| "destination must be a Classic Stellar G address".to_owned())?;

    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let destination_exists = horizon.account_exists(&request.destination)?;
    if !destination_exists && !asset.is_native() {
        return Err(
            "Destination account does not exist. Only XLM can create a new Stellar account; issued assets require an existing account and trustline."
                .to_owned(),
        );
    }
    let ledger = horizon.get_ledger_parameters()?;
    validate_transfer(&account, &asset, amount, ledger)?;
    if !destination_exists {
        let minimum = 2_i64
            .checked_mul(ledger.base_reserve_in_stroops)
            .ok_or_else(|| "base reserve overflow".to_owned())?;
        if amount < minimum {
            return Err(format!(
                "Creating a Stellar account requires at least {} XLM at the current base reserve; requested {} XLM",
                format_stroops(minimum),
                format_stroops(amount)
            ));
        }
    }

    let body = payment_body(destination, &asset, amount, !destination_exists)?;
    let mut envelope = build_single_operation_envelope(
        &record.address,
        body,
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        request.memo.as_deref(),
    )?;

    render_review(
        &record,
        &request.destination,
        &asset,
        amount,
        ledger.base_fee_in_stroops,
        !destination_exists,
        request.memo.as_deref(),
    );
    if !request.yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }

    sign_and_submit(&record, network, &mut envelope, &horizon)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SendRequest {
    amount: String,
    asset: String,
    destination: String,
    wallet: Option<String>,
    memo: Option<String>,
    yes: bool,
}

impl SendRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        const USAGE: &str =
            "usage: fresnica send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]";
        if arguments.len() < 4 || arguments[2].to_lowercase() != "to" {
            return Err(USAGE.to_owned());
        }
        let mut wallet = None;
        let mut memo = None;
        let mut yes = false;
        let mut index = 4;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(arguments.get(index).ok_or_else(|| USAGE.to_owned())?.clone());
                    index += 1;
                }
                "--memo" => {
                    index += 1;
                    memo = Some(arguments.get(index).ok_or_else(|| USAGE.to_owned())?.clone());
                    index += 1;
                }
                "-y" | "--yes" => {
                    yes = true;
                    index += 1;
                }
                _ => return Err(USAGE.to_owned()),
            }
        }
        Ok(Self {
            amount: arguments[0].clone(),
            asset: arguments[1].clone(),
            destination: arguments[3].clone(),
            wallet,
            memo,
            yes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PaymentAsset {
    Native,
    Credit { code: String, issuer: String },
}

impl PaymentAsset {
    fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("XLM") {
            return Ok(Self::Native);
        }
        let (code, issuer) = value
            .split_once(':')
            .ok_or_else(|| "asset must be XLM or CODE:GISSUER".to_owned())?;
        validate_asset_code(code)?;
        AccountId::from_str(issuer)
            .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
        Ok(Self::Credit {
            code: code.to_owned(),
            issuer: issuer.to_owned(),
        })
    }

    fn is_native(&self) -> bool {
        matches!(self, Self::Native)
    }

    fn display(&self) -> String {
        match self {
            Self::Native => "XLM".to_owned(),
            Self::Credit { code, issuer } => format!("{code}:{issuer}"),
        }
    }

    fn to_xdr(&self) -> Result<Asset, String> {
        match self {
            Self::Native => Ok(Asset::Native),
            Self::Credit { code, issuer } => {
                let issuer = AccountId::from_str(issuer)
                    .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
                if code.len() <= 4 {
                    let mut raw = [0u8; 4];
                    raw[..code.len()].copy_from_slice(code.as_bytes());
                    Ok(Asset::CreditAlphanum4(AlphaNum4 {
                        asset_code: AssetCode4(raw),
                        issuer,
                    }))
                } else {
                    let mut raw = [0u8; 12];
                    raw[..code.len()].copy_from_slice(code.as_bytes());
                    Ok(Asset::CreditAlphanum12(AlphaNum12 {
                        asset_code: AssetCode12(raw),
                        issuer,
                    }))
                }
            }
        }
    }

    fn matches_balance(&self, balance: &Value) -> bool {
        match self {
            Self::Native => text(balance, "asset_type") == Some("native"),
            Self::Credit { code, issuer } => {
                text(balance, "asset_code") == Some(code.as_str())
                    && text(balance, "asset_issuer") == Some(issuer.as_str())
            }
        }
    }
}

fn payment_body(
    destination: AccountId,
    asset: &PaymentAsset,
    amount: i64,
    create_destination: bool,
) -> Result<OperationBody, String> {
    if create_destination {
        return Ok(OperationBody::CreateAccount(CreateAccountOp {
            destination,
            starting_balance: amount,
        }));
    }
    Ok(OperationBody::Payment(PaymentOp {
        destination: account_id_to_muxed(&destination),
        asset: asset.to_xdr()?,
        amount,
    }))
}

fn validate_transfer(
    account: &Value,
    asset: &PaymentAsset,
    requested: i64,
    ledger: LedgerParameters,
) -> Result<(), String> {
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
    let raw = balances
        .iter()
        .find(|balance| asset.matches_balance(balance));
    let available = match raw {
        Some(raw_balance) if asset.is_native() => {
            let balance = balance_stroops(raw_balance, "balance")?;
            let selling = balance_stroops(raw_balance, "selling_liabilities")?;
            let minimum = minimum_balance_stroops(account, ledger.base_reserve_in_stroops)?;
            balance
                .saturating_sub(selling)
                .saturating_sub(minimum)
                .saturating_sub(i64::from(ledger.base_fee_in_stroops))
                .max(0)
        }
        Some(raw_balance) => {
            let balance = balance_stroops(raw_balance, "balance")?;
            let selling = balance_stroops(raw_balance, "selling_liabilities")?;
            let native = balances
                .iter()
                .find(|balance| text(balance, "asset_type") == Some("native"))
                .ok_or_else(|| "No XLM balance is available to pay the transaction fee".to_owned())?;
            let native_balance = balance_stroops(native, "balance")?;
            let native_selling = balance_stroops(native, "selling_liabilities")?;
            let minimum = minimum_balance_stroops(account, ledger.base_reserve_in_stroops)?;
            let free_before_fee = native_balance
                .saturating_sub(native_selling)
                .saturating_sub(minimum)
                .max(0);
            if free_before_fee < i64::from(ledger.base_fee_in_stroops) {
                return Err(format!(
                    "Insufficient XLM for transaction fee: need {}, available {}",
                    format_stroops(i64::from(ledger.base_fee_in_stroops)),
                    format_stroops(free_before_fee)
                ));
            }
            balance.saturating_sub(selling).max(0)
        }
        None => 0,
    };
    if requested > available {
        return Err(format!(
            "Insufficient {} balance: requested {}, available {}",
            asset.display(),
            format_stroops(requested),
            format_stroops(available)
        ));
    }
    Ok(())
}

fn validate_asset_code(code: &str) -> Result<(), String> {
    if code.is_empty()
        || code.len() > 12
        || !code.is_ascii()
        || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("issued asset code must be 1-12 ASCII letters or digits".to_owned());
    }
    Ok(())
}

fn account_id_to_muxed(account: &AccountId) -> MuxedAccount {
    match &account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => MuxedAccount::Ed25519(key.clone()),
    }
}

fn render_review(
    record: &WalletRecord,
    destination: &str,
    asset: &PaymentAsset,
    amount: i64,
    base_fee: u32,
    create_destination: bool,
    memo: Option<&str>,
) {
    println!("Review transaction");
    println!(
        "Operation: {}",
        if create_destination {
            "CreateAccount"
        } else {
            "Payment"
        }
    );
    println!("From:      {} ({})", record.name, record.address);
    println!("To:        {destination}");
    println!("Amount:    {} {}", format_stroops(amount), asset.display());
    println!("Fee:       {} XLM", format_stroops(i64::from(base_fee)));
    println!("Network:   {}", record.network);
    if let Some(memo) = memo.filter(|memo| !memo.is_empty()) {
        println!("Memo:      {memo}");
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESTINATION: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn account(native_balance: &str, subentries: i64) -> Value {
        serde_json::json!({
            "sequence": "7",
            "subentry_count": subentries,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [{
                "asset_type": "native",
                "balance": native_balance,
                "selling_liabilities": "0.0000000",
                "buying_liabilities": "0.0000000"
            }]
        })
    }

    #[test]
    fn native_availability_reserves_minimum_balance_and_fee() {
        let account = account("10.0000000", 2);
        let ledger = LedgerParameters {
            base_fee_in_stroops: 100,
            base_reserve_in_stroops: 5_000_000,
        };
        assert!(validate_transfer(
            &account,
            &PaymentAsset::Native,
            parse_positive_stroops("7.99999").unwrap(),
            ledger,
        )
        .is_ok());
        assert!(validate_transfer(
            &account,
            &PaymentAsset::Native,
            parse_positive_stroops("8").unwrap(),
            ledger,
        )
        .is_err());
    }

    #[test]
    fn payment_body_switches_to_create_account_for_missing_destination() {
        let destination = AccountId::from_str(DESTINATION).unwrap();
        assert!(matches!(
            payment_body(destination.clone(), &PaymentAsset::Native, 10_000_000, false).unwrap(),
            OperationBody::Payment(_)
        ));
        assert!(matches!(
            payment_body(destination, &PaymentAsset::Native, 10_000_000, true).unwrap(),
            OperationBody::CreateAccount(_)
        ));
    }

    #[test]
    fn send_parser_matches_python_cli_shape() {
        let args = [
            "1.5", "XLM", "to", DESTINATION, "--memo", "hello", "--wallet", "alpha", "-y",
        ]
        .map(str::to_owned);
        let request = SendRequest::parse(&args).unwrap();
        assert_eq!(request.amount, "1.5");
        assert_eq!(request.asset, "XLM");
        assert_eq!(request.destination, DESTINATION);
        assert_eq!(request.memo.as_deref(), Some("hello"));
        assert_eq!(request.wallet.as_deref(), Some("alpha"));
        assert!(request.yes);
    }
}
