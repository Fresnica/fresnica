use std::io::{self, Write};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    derive_verified_unlock_key, sign_protected_transaction_envelope, transaction_envelope_xdr,
    transaction_hash, ProtectionRegistry,
};
use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, CreateAccountOp, Memo,
    MuxedAccount, Operation, OperationBody, PaymentOp, Preconditions, PublicKey, SequenceNumber,
    StringM, TimeBounds, TimePoint, Transaction, TransactionEnvelope, TransactionExt,
    TransactionV1Envelope, VecM,
};

use crate::horizon::{
    HorizonClient, LedgerParameters, SubmissionError, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL,
};
use crate::storage::{WalletRecord, WalletStorage};

const STROOPS_PER_XLM: i64 = 10_000_000;
const TX_TIMEOUT_SECONDS: u64 = 30;
const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

pub fn command_send(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = SendRequest::parse(arguments)?;
    let record = resolve_signing_wallet(storage, network, request.wallet.as_deref())?;
    let asset = PaymentAsset::parse(&request.asset)?;
    let amount = parse_positive_stroops(&request.amount)?;
    let destination_account = AccountId::from_str(&request.destination)
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

    let sequence = account_sequence(&account)?;
    let max_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?
        .as_secs()
        .checked_add(TX_TIMEOUT_SECONDS)
        .ok_or_else(|| "transaction timeout overflow".to_owned())?;
    let mut envelope = build_payment_envelope(
        &record.address,
        destination_account,
        &asset,
        amount,
        sequence,
        ledger.base_fee_in_stroops,
        max_time,
        !destination_exists,
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

    let passcode = super::prompt_hidden("Fresnica passcode: ")?;
    let protected = record
        .secret
        .as_ref()
        .ok_or_else(|| "wallet has no protected signing material".to_owned())?;
    let registry = ProtectionRegistry::new();
    let unlock_key = derive_verified_unlock_key(
        &registry,
        protected,
        &passcode,
        &record.address,
    )
    .map_err(|error| format!("Unable to unlock wallet: {error}"))?;
    let network_passphrase = network_passphrase(network)?;
    sign_protected_transaction_envelope(
        &registry,
        protected,
        &unlock_key,
        &record.address,
        &mut envelope,
        network_passphrase,
    )
    .map_err(|error| format!("Unable to sign transaction: {error}"))?;
    drop(unlock_key);
    drop(passcode);

    let tx_hash = transaction_hash(&envelope, network_passphrase)
        .map_err(|error| format!("Unable to hash signed transaction: {error}"))?;
    let tx_hash_hex = hex(&tx_hash);
    let encoded = STANDARD.encode(
        transaction_envelope_xdr(&envelope)
            .map_err(|error| format!("Unable to encode signed transaction: {error}"))?,
    );
    match horizon.submit_transaction(&encoded) {
        Ok(response) => {
            let returned_hash = response
                .get("hash")
                .and_then(Value::as_str)
                .unwrap_or(&tx_hash_hex);
            println!("Submitted: {returned_hash}");
            if let Some(ledger) = response.get("ledger").and_then(Value::as_u64) {
                println!("Ledger:    {ledger}");
            }
            Ok(())
        }
        Err(SubmissionError::Rejected(message)) => {
            Err(format!("Transaction rejected ({tx_hash_hex}): {message}"))
        }
        Err(SubmissionError::Uncertain(message)) => Err(format!(
            "Transaction submission status is uncertain for {tx_hash_hex}: {message}. Check this transaction hash before retrying."
        )),
    }
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
        if code.is_empty()
            || code.len() > 12
            || !code.is_ascii()
            || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err("issued asset code must be 1-12 ASCII letters or digits".to_owned());
        }
        AccountId::from_str(issuer).map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
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

fn resolve_signing_wallet(
    storage: &WalletStorage,
    network: &str,
    name: Option<&str>,
) -> Result<WalletRecord, String> {
    let record = storage.resolve(name)?;
    if record.network != network {
        return Err(format!(
            "wallet \"{}\" is configured for {}; invoke with --network {}",
            record.name, record.network, record.network
        ));
    }
    if record.watch_only() || record.secret.is_none() {
        return Err(format!("wallet \"{}\" is watch-only", record.name));
    }
    Ok(record)
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
        Some(balance) if asset.is_native() => {
            let balance = balance_stroops(balance, "balance")?;
            let selling = balance_stroops(balance, "selling_liabilities")?;
            let minimum = minimum_balance_stroops(account, ledger.base_reserve_in_stroops)?;
            balance
                .saturating_sub(selling)
                .saturating_sub(minimum)
                .saturating_sub(i64::from(ledger.base_fee_in_stroops))
                .max(0)
        }
        Some(balance) => {
            let balance = balance_stroops(balance, "balance")?;
            let selling = balance_stroops(balance, "selling_liabilities")?;
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

fn minimum_balance_stroops(account: &Value, base_reserve: i64) -> Result<i64, String> {
    let subentries = integer(account.get("subentry_count")).unwrap_or(0);
    let sponsoring = integer(account.get("num_sponsoring")).unwrap_or(0);
    let sponsored = integer(account.get("num_sponsored")).unwrap_or(0);
    let units = (2_i64)
        .saturating_add(subentries)
        .saturating_add(sponsoring)
        .saturating_sub(sponsored)
        .max(0);
    units
        .checked_mul(base_reserve)
        .ok_or_else(|| "minimum balance overflow".to_owned())
}

fn build_payment_envelope(
    source: &str,
    destination: AccountId,
    asset: &PaymentAsset,
    amount: i64,
    current_sequence: i64,
    base_fee: u32,
    max_time: u64,
    create_destination: bool,
    memo: Option<&str>,
) -> Result<TransactionEnvelope, String> {
    let source = AccountId::from_str(source)
        .map_err(|_| "wallet source must be a Classic G address".to_owned())?;
    let source_account = account_id_to_muxed(&source);
    let body = if create_destination {
        OperationBody::CreateAccount(CreateAccountOp {
            destination,
            starting_balance: amount,
        })
    } else {
        OperationBody::Payment(PaymentOp {
            destination: account_id_to_muxed(&destination),
            asset: asset.to_xdr()?,
            amount,
        })
    };
    let operation = Operation {
        source_account: None,
        body,
    };
    let operations: VecM<Operation, 100> = vec![operation]
        .try_into()
        .map_err(|_| "too many transaction operations".to_owned())?;
    let memo = match memo {
        Some(value) if !value.is_empty() => Memo::Text(
            StringM::<28>::try_from(value)
                .map_err(|_| "text memo must be at most 28 bytes".to_owned())?,
        ),
        _ => Memo::None,
    };
    let sequence = current_sequence
        .checked_add(1)
        .ok_or_else(|| "account sequence overflow".to_owned())?;
    let transaction = Transaction {
        source_account,
        fee: base_fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::Time(TimeBounds {
            min_time: TimePoint(0),
            max_time: TimePoint(max_time),
        }),
        memo,
        operations,
        ext: TransactionExt::V0,
    };
    Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: transaction,
        signatures: VecM::default(),
    }))
}

fn account_id_to_muxed(account: &AccountId) -> MuxedAccount {
    match &account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => MuxedAccount::Ed25519(key.clone()),
    }
}

fn account_sequence(account: &Value) -> Result<i64, String> {
    text(account, "sequence")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "Horizon returned invalid account sequence".to_owned())
}

fn balance_stroops(balance: &Value, field: &str) -> Result<i64, String> {
    let value = text(balance, field).unwrap_or("0");
    parse_stroops(value, false)
        .map_err(|_| format!("Horizon returned invalid {field}: {value}"))
}

fn parse_positive_stroops(value: &str) -> Result<i64, String> {
    parse_stroops(value, true).map_err(|_| {
        "Amount must be greater than zero with at most 7 decimal places".to_owned()
    })
}

fn parse_stroops(value: &str, require_positive: bool) -> Result<i64, ()> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') || value.starts_with('+') {
        return Err(());
    }
    let mut parts = value.split('.');
    let whole = parts.next().ok_or(())?;
    let fraction = parts.next();
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(());
    }
    let whole: i64 = whole.parse().map_err(|_| ())?;
    let mut fraction_value = 0_i64;
    if let Some(fraction) = fraction {
        if fraction.len() > 7 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(());
        }
        if !fraction.is_empty() {
            fraction_value = fraction.parse::<i64>().map_err(|_| ())?;
            for _ in fraction.len()..7 {
                fraction_value = fraction_value.checked_mul(10).ok_or(())?;
            }
        }
    }
    let stroops = whole
        .checked_mul(STROOPS_PER_XLM)
        .and_then(|value| value.checked_add(fraction_value))
        .ok_or(())?;
    if require_positive && stroops <= 0 {
        return Err(());
    }
    Ok(stroops)
}

fn format_stroops(value: i64) -> String {
    let whole = value / STROOPS_PER_XLM;
    let fraction = value % STROOPS_PER_XLM;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{:07}", fraction.abs());
    let fraction = fraction.trim_end_matches('0');
    format!("{whole}.{fraction}")
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

fn confirm_submission() -> Result<bool, String> {
    print!("Submit this transaction? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    Ok(matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn network_client(network: &str) -> Result<HorizonClient, String> {
    Ok(HorizonClient::new(match network {
        "mainnet" => MAINNET_HORIZON_URL,
        "testnet" => TESTNET_HORIZON_URL,
        other => return Err(format!("unknown network: {other}")),
    }))
}

fn network_passphrase(network: &str) -> Result<&'static str, String> {
    match network {
        "mainnet" => Ok(MAINNET_PASSPHRASE),
        "testnet" => Ok(TESTNET_PASSPHRASE),
        other => Err(format!("unknown network: {other}")),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
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
    fn parses_stellar_amounts_exactly_to_stroops() {
        assert_eq!(parse_positive_stroops("1").unwrap(), 10_000_000);
        assert_eq!(parse_positive_stroops("1.2345678").unwrap(), 12_345_678);
        assert!(parse_positive_stroops("1.23456789").is_err());
        assert!(parse_positive_stroops("0").is_err());
        assert!(parse_positive_stroops("-1").is_err());
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
    fn builds_v1_payment_with_sequence_fee_timebound_and_memo() {
        let destination = AccountId::from_str(DESTINATION).unwrap();
        let envelope = build_payment_envelope(
            SOURCE,
            destination,
            &PaymentAsset::Native,
            12_345_678,
            7,
            100,
            1_800_000_000,
            false,
            Some("hello"),
        )
        .unwrap();
        let TransactionEnvelope::Tx(envelope) = envelope else {
            panic!("expected v1 transaction envelope");
        };
        assert_eq!(envelope.tx.seq_num.0, 8);
        assert_eq!(envelope.tx.fee, 100);
        assert_eq!(envelope.tx.operations.len(), 1);
        assert!(matches!(envelope.tx.memo, Memo::Text(_)));
        assert!(matches!(envelope.tx.cond, Preconditions::Time(_)));
        assert!(matches!(
            envelope.tx.operations[0].body,
            OperationBody::Payment(_)
        ));
    }

    #[test]
    fn missing_destination_builds_create_account_operation() {
        let destination = AccountId::from_str(DESTINATION).unwrap();
        let envelope = build_payment_envelope(
            SOURCE,
            destination,
            &PaymentAsset::Native,
            10_000_000,
            7,
            100,
            1_800_000_000,
            true,
            None,
        )
        .unwrap();
        let TransactionEnvelope::Tx(envelope) = envelope else {
            panic!("expected v1 transaction envelope");
        };
        assert!(matches!(
            envelope.tx.operations[0].body,
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
