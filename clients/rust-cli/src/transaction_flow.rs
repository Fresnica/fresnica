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
    AccountId, Memo, MuxedAccount, Operation, OperationBody, Preconditions, PublicKey,
    SequenceNumber, StringM, TimeBounds, TimePoint, Transaction, TransactionEnvelope,
    TransactionExt, TransactionV1Envelope, VecM,
};

use crate::horizon::{
    HorizonClient, SubmissionError, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL,
};
use crate::storage::{WalletRecord, WalletStorage};

pub const STROOPS_PER_XLM: i64 = 10_000_000;
const TX_TIMEOUT_SECONDS: u64 = 30;
const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

pub fn network_client(network: &str) -> Result<HorizonClient, String> {
    Ok(HorizonClient::new(match network {
        "mainnet" => MAINNET_HORIZON_URL,
        "testnet" => TESTNET_HORIZON_URL,
        other => return Err(format!("unknown network: {other}")),
    }))
}

pub fn resolve_signing_wallet(
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

pub fn build_single_operation_envelope(
    source: &str,
    body: OperationBody,
    current_sequence: i64,
    base_fee: u32,
    memo: Option<&str>,
) -> Result<TransactionEnvelope, String> {
    let source = AccountId::from_str(source)
        .map_err(|_| "wallet source must be a Classic G address".to_owned())?;
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
    let max_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?
        .as_secs()
        .checked_add(TX_TIMEOUT_SECONDS)
        .ok_or_else(|| "transaction timeout overflow".to_owned())?;
    let transaction = Transaction {
        source_account: account_id_to_muxed(&source),
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

pub fn confirm_submission() -> Result<bool, String> {
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

pub fn sign_and_submit(
    record: &WalletRecord,
    network: &str,
    envelope: &mut TransactionEnvelope,
    horizon: &HorizonClient,
) -> Result<(), String> {
    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
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
        envelope,
        network_passphrase,
    )
    .map_err(|error| format!("Unable to sign transaction: {error}"))?;
    drop(unlock_key);
    drop(passcode);

    let tx_hash = transaction_hash(envelope, network_passphrase)
        .map_err(|error| format!("Unable to hash signed transaction: {error}"))?;
    let tx_hash_hex = hex(&tx_hash);
    let encoded = STANDARD.encode(
        transaction_envelope_xdr(envelope)
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

pub fn account_sequence(account: &Value) -> Result<i64, String> {
    text(account, "sequence")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "Horizon returned invalid account sequence".to_owned())
}

pub fn minimum_balance_stroops(account: &Value, base_reserve: i64) -> Result<i64, String> {
    let subentries = integer(account.get("subentry_count")).unwrap_or(0);
    let sponsoring = integer(account.get("num_sponsoring")).unwrap_or(0);
    let sponsored = integer(account.get("num_sponsored")).unwrap_or(0);
    let units = 2_i64
        .saturating_add(subentries)
        .saturating_add(sponsoring)
        .saturating_sub(sponsored)
        .max(0);
    units
        .checked_mul(base_reserve)
        .ok_or_else(|| "minimum balance overflow".to_owned())
}

pub fn balance_stroops(balance: &Value, field: &str) -> Result<i64, String> {
    let value = text(balance, field).unwrap_or("0");
    parse_stroops(value, false)
        .map_err(|_| format!("Horizon returned invalid {field}: {value}"))
}

pub fn parse_positive_stroops(value: &str) -> Result<i64, String> {
    parse_stroops(value, true).map_err(|_| {
        "Amount must be greater than zero with at most 7 decimal places".to_owned()
    })
}

pub fn parse_stroops(value: &str, require_positive: bool) -> Result<i64, ()> {
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

pub fn format_stroops(value: i64) -> String {
    let whole = value / STROOPS_PER_XLM;
    let fraction = value % STROOPS_PER_XLM;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{:07}", fraction.abs());
    let fraction = fraction.trim_end_matches('0');
    format!("{whole}.{fraction}")
}

fn account_id_to_muxed(account: &AccountId) -> MuxedAccount {
    match &account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => MuxedAccount::Ed25519(key.clone()),
    }
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

    #[test]
    fn exact_stroop_parser_rejects_rounding() {
        assert_eq!(parse_positive_stroops("1.2345678").unwrap(), 12_345_678);
        assert!(parse_positive_stroops("1.23456789").is_err());
        assert!(parse_positive_stroops("0").is_err());
    }
}
