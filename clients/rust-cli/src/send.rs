use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, CreateAccountOp, Hash, Memo,
    MuxedAccount, OperationBody, PaymentOp, PublicKey, StringM,
};

use crate::transaction_flow::{
    account_sequence, balance_stroops, build_single_operation_envelope_with_memo,
    confirm_submission, format_stroops, minimum_balance_stroops, network_client,
    parse_positive_stroops, resolve_signing_wallet, sign_and_submit,
};
use fresnica_client::{resolve_destination, LedgerParameters, WalletRecord, WalletStorage};

pub fn command_send(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = SendRequest::parse(arguments)?;
    let record = resolve_signing_wallet(storage, network, request.wallet.as_deref())?;
    let resolved = resolve_destination(storage, &request.destination, request.memo.as_deref())?;
    review_and_submit_payment(
        &record,
        network,
        &request.amount,
        &request.asset,
        &resolved.address,
        resolved.contact_name.as_deref(),
        PaymentMemo::from_text(resolved.memo.as_deref()),
        request.yes,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PaymentMemo {
    None,
    Text(String),
    Id(u64),
    Hash([u8; 32]),
}

impl PaymentMemo {
    fn from_text(value: Option<&str>) -> Self {
        match value.filter(|value| !value.is_empty()) {
            Some(value) => Self::Text(value.to_owned()),
            None => Self::None,
        }
    }

    pub(crate) fn from_anchor_fields(
        memo_type: Option<&str>,
        memo: Option<&str>,
    ) -> Result<Self, String> {
        match (memo_type, memo) {
            (None, None) => Ok(Self::None),
            (Some(_), None) | (None, Some(_)) => {
                Err("anchor withdrawal memo_type and memo must be supplied together".to_owned())
            }
            (Some("text"), Some(value)) => Ok(Self::Text(value.to_owned())),
            (Some("id"), Some(value)) => value.parse::<u64>().map(Self::Id).map_err(|_| {
                "anchor withdrawal id memo must be an unsigned 64-bit integer".to_owned()
            }),
            (Some("hash"), Some(value)) => {
                let decoded = STANDARD
                    .decode(value)
                    .map_err(|_| "anchor withdrawal hash memo must be valid base64".to_owned())?;
                let hash: [u8; 32] = decoded.try_into().map_err(|_| {
                    "anchor withdrawal hash memo must decode to exactly 32 bytes".to_owned()
                })?;
                Ok(Self::Hash(hash))
            }
            (Some(other), Some(_)) => Err(format!(
                "unsupported anchor withdrawal memo type: {other}; expected text, id, or hash"
            )),
        }
    }

    fn to_xdr(&self) -> Result<Memo, String> {
        match self {
            Self::None => Ok(Memo::None),
            Self::Text(value) => Ok(Memo::Text(
                StringM::<28>::try_from(value.as_str())
                    .map_err(|_| "text memo must be at most 28 bytes".to_owned())?,
            )),
            Self::Id(value) => Ok(Memo::Id(*value)),
            Self::Hash(value) => Ok(Memo::Hash(Hash(*value))),
        }
    }

    fn review_text(&self) -> Option<(&'static str, String)> {
        match self {
            Self::None => None,
            Self::Text(value) => Some(("text", value.clone())),
            Self::Id(value) => Some(("id", value.to_string())),
            Self::Hash(value) => Some(("hash", STANDARD.encode(value))),
        }
    }
}

pub(crate) fn review_and_submit_payment(
    record: &WalletRecord,
    network: &str,
    amount_text: &str,
    asset_text: &str,
    destination_address: &str,
    contact_name: Option<&str>,
    memo: PaymentMemo,
    yes: bool,
) -> Result<(), String> {
    let asset = PaymentAsset::parse(asset_text)?;
    let amount = parse_positive_stroops(amount_text)?;
    let destination = AccountId::from_str(destination_address)
        .map_err(|_| "destination must be a Classic Stellar G address".to_owned())?;
    let memo_xdr = memo.to_xdr()?;

    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let destination_exists = horizon.account_exists(destination_address)?;
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
    let mut envelope = build_single_operation_envelope_with_memo(
        &record.address,
        body,
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        memo_xdr,
    )?;

    render_review(
        record,
        destination_address,
        contact_name,
        &asset,
        amount,
        ledger.base_fee_in_stroops,
        !destination_exists,
        &memo,
    );
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }

    sign_and_submit(record, network, &mut envelope, &horizon)
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
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "--memo" => {
                    index += 1;
                    memo = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
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
                .ok_or_else(|| {
                    "No XLM balance is available to pay the transaction fee".to_owned()
                })?;
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
    contact_name: Option<&str>,
    asset: &PaymentAsset,
    amount: i64,
    base_fee: u32,
    create_destination: bool,
    memo: &PaymentMemo,
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
    if let Some(name) = contact_name {
        println!("To:        {name} ({destination})");
    } else {
        println!("To:        {destination}");
    }
    println!("Amount:    {} {}", format_stroops(amount), asset.display());
    println!("Fee:       {} XLM", format_stroops(i64::from(base_fee)));
    println!("Network:   {}", record.network);
    if let Some((memo_type, value)) = memo.review_text() {
        println!("Memo:      {value} ({memo_type})");
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
            payment_body(
                destination.clone(),
                &PaymentAsset::Native,
                10_000_000,
                false
            )
            .unwrap(),
            OperationBody::Payment(_)
        ));
        assert!(matches!(
            payment_body(destination, &PaymentAsset::Native, 10_000_000, true).unwrap(),
            OperationBody::CreateAccount(_)
        ));
    }

    #[test]
    fn anchor_payment_memo_supports_text_id_and_hash() {
        let text = PaymentMemo::from_anchor_fields(Some("text"), Some("anchor")).unwrap();
        assert!(matches!(text.to_xdr().unwrap(), Memo::Text(_)));

        let id = PaymentMemo::from_anchor_fields(Some("id"), Some("42")).unwrap();
        assert_eq!(id, PaymentMemo::Id(42));
        assert_eq!(id.to_xdr().unwrap(), Memo::Id(42));

        let bytes = [7_u8; 32];
        let encoded = STANDARD.encode(bytes);
        let hash = PaymentMemo::from_anchor_fields(Some("hash"), Some(&encoded)).unwrap();
        assert_eq!(hash, PaymentMemo::Hash(bytes));
        assert_eq!(hash.to_xdr().unwrap(), Memo::Hash(Hash(bytes)));
    }

    #[test]
    fn anchor_payment_memo_rejects_malformed_values() {
        assert!(PaymentMemo::from_anchor_fields(Some("id"), None).is_err());
        assert!(PaymentMemo::from_anchor_fields(None, Some("42")).is_err());
        assert!(PaymentMemo::from_anchor_fields(Some("id"), Some("-1")).is_err());
        assert!(PaymentMemo::from_anchor_fields(Some("hash"), Some("not-base64")).is_err());
        assert!(
            PaymentMemo::from_anchor_fields(Some("hash"), Some(&STANDARD.encode([1_u8; 31])))
                .is_err()
        );
        assert!(PaymentMemo::Text("x".repeat(29)).to_xdr().is_err());
    }

    #[test]
    fn send_parser_matches_python_cli_shape() {
        let args = [
            "1.5",
            "XLM",
            "to",
            DESTINATION,
            "--memo",
            "hello",
            "--wallet",
            "alpha",
            "-y",
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
