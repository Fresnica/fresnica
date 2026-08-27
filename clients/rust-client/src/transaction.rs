use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    parse_transaction_envelope_xdr, transaction_envelope_has_valid_signature,
    transaction_envelope_xdr, transaction_hash,
};
use fresnica_sdk::{FresnicaSdk, SdkErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellar_xdr::{
    AccountId, Memo, MuxedAccount, Operation, OperationBody, Preconditions, PublicKey,
    SequenceNumber, StringM, TimeBounds, TimePoint, Transaction, TransactionEnvelope,
    TransactionExt, TransactionV1Envelope, VecM,
};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::ledger_authorization::load_classic_ledger_authorization_plan;
use crate::signing_coordination::sign_with_local_ed25519;
use crate::{
    HorizonClient, SubmissionError, WalletRecord, WalletStorage, MAINNET_HORIZON_URL,
    TESTNET_HORIZON_URL,
};

pub const STROOPS_PER_XLM: i64 = 10_000_000;
const TX_TIMEOUT_SECONDS: u64 = 30;
const PENDING_TTL_SECONDS: i64 = 210;
const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

pub fn parse_transaction_xdr(xdr: &[u8]) -> Result<TransactionEnvelope, String> {
    parse_transaction_envelope_xdr(xdr)
        .map_err(|error| format!("invalid Stellar transaction XDR: {error}"))
}

pub fn transaction_xdr_bytes(envelope: &TransactionEnvelope) -> Result<Vec<u8>, String> {
    transaction_envelope_xdr(envelope).map_err(|error| error.to_string())
}

pub fn has_valid_transaction_signature(
    envelope: &TransactionEnvelope,
    network: &str,
    signer_public_key: &str,
) -> Result<bool, String> {
    transaction_envelope_has_valid_signature(
        envelope,
        network_passphrase(network)?,
        signer_public_key,
    )
    .map_err(|error| format!("unable to verify transaction signature: {error}"))
}

pub fn network_client(network: &str) -> Result<HorizonClient, String> {
    Ok(HorizonClient::new(match network {
        "mainnet" => MAINNET_HORIZON_URL,
        "testnet" => TESTNET_HORIZON_URL,
        other => return Err(format!("unknown network: {other}")),
    }))
}

pub fn resolve_write_wallet(
    storage: &WalletStorage,
    horizon: &HorizonClient,
    network: &str,
    name: Option<&str>,
) -> Result<WalletRecord, String> {
    let record = resolve_network_wallet(storage, network, name)?;
    PendingTransactionStore::for_home(storage.home()).reconcile_and_ensure_clear(
        network,
        &record.address,
        horizon,
    )?;
    Ok(record)
}

fn resolve_network_wallet(
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
    Ok(record)
}

pub fn build_single_operation_envelope(
    source: &str,
    body: OperationBody,
    current_sequence: i64,
    base_fee: u32,
    memo: Option<&str>,
) -> Result<TransactionEnvelope, String> {
    let memo = text_memo(memo)?;
    build_single_operation_envelope_with_memo(source, body, current_sequence, base_fee, memo)
}

pub fn build_single_operation_envelope_with_memo(
    source: &str,
    body: OperationBody,
    current_sequence: i64,
    base_fee: u32,
    memo: Memo,
) -> Result<TransactionEnvelope, String> {
    build_operation_envelope_with_memo(source, vec![body], current_sequence, base_fee, memo)
}

pub fn build_operation_envelope(
    source: &str,
    bodies: Vec<OperationBody>,
    current_sequence: i64,
    base_fee_per_operation: u32,
    memo: Option<&str>,
) -> Result<TransactionEnvelope, String> {
    let memo = text_memo(memo)?;
    build_operation_envelope_with_memo(
        source,
        bodies,
        current_sequence,
        base_fee_per_operation,
        memo,
    )
}

fn text_memo(memo: Option<&str>) -> Result<Memo, String> {
    match memo {
        Some(value) if !value.is_empty() => Ok(Memo::Text(
            StringM::<28>::try_from(value)
                .map_err(|_| "text memo must be at most 28 bytes".to_owned())?,
        )),
        _ => Ok(Memo::None),
    }
}

fn build_operation_envelope_with_memo(
    source: &str,
    bodies: Vec<OperationBody>,
    current_sequence: i64,
    base_fee_per_operation: u32,
    memo: Memo,
) -> Result<TransactionEnvelope, String> {
    if bodies.is_empty() {
        return Err("transaction must contain at least one operation".to_owned());
    }
    let source = AccountId::from_str(source)
        .map_err(|_| "wallet source must be a Classic G address".to_owned())?;
    let operation_count =
        u32::try_from(bodies.len()).map_err(|_| "too many transaction operations".to_owned())?;
    let operations: VecM<Operation, 100> = bodies
        .into_iter()
        .map(|body| Operation {
            source_account: None,
            body,
        })
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "too many transaction operations".to_owned())?;
    let fee = base_fee_per_operation
        .checked_mul(operation_count)
        .ok_or_else(|| "transaction fee overflow".to_owned())?;
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
        fee,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionSubmission {
    pub hash: String,
    pub ledger: Option<u64>,
}

fn ensure_transaction_not_expired(envelope: &TransactionEnvelope) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?
        .as_secs();
    ensure_transaction_not_expired_at(envelope, now)
}

fn ensure_transaction_not_expired_at(
    envelope: &TransactionEnvelope,
    now_unix: u64,
) -> Result<(), String> {
    let TransactionEnvelope::Tx(transaction) = envelope else {
        return Ok(());
    };
    let max_time = match &transaction.tx.cond {
        Preconditions::None => return Ok(()),
        Preconditions::Time(bounds) => bounds.max_time.0,
        Preconditions::V2(value) => match value.time_bounds.as_ref() {
            Some(bounds) => bounds.max_time.0,
            None => return Ok(()),
        },
    };
    if max_time != 0 && now_unix > max_time {
        return Err(
            "Prepared transaction has expired; prepare and review the transaction again before signing"
                .to_owned(),
        );
    }
    Ok(())
}

pub fn sign_and_submit(
    storage: &WalletStorage,
    record: &WalletRecord,
    network: &str,
    envelope: &mut TransactionEnvelope,
    horizon: &HorizonClient,
    passcode: String,
) -> Result<TransactionSubmission, String> {
    ensure_transaction_not_expired(envelope)?;
    let authorization = load_classic_ledger_authorization_plan(horizon, envelope)?;
    sign_with_local_ed25519(storage, &authorization, network, envelope, &passcode)?;
    let network_passphrase = network_passphrase(network)?;

    let tx_hash = transaction_hash(envelope, network_passphrase)
        .map_err(|error| format!("Unable to hash signed transaction: {error}"))?;
    let tx_hash_hex = hex(&tx_hash);
    let encoded = STANDARD.encode(
        transaction_xdr_bytes(envelope)
            .map_err(|error| format!("Unable to encode signed transaction: {error}"))?,
    );
    match horizon.submit_transaction(&encoded) {
        Ok(response) => Ok(TransactionSubmission {
            hash: response
                .get("hash")
                .and_then(Value::as_str)
                .unwrap_or(&tx_hash_hex)
                .to_owned(),
            ledger: response.get("ledger").and_then(Value::as_u64),
        }),
        Err(SubmissionError::Rejected(message)) => {
            Err(format!("Transaction rejected ({tx_hash_hex}): {message}"))
        }
        Err(SubmissionError::Uncertain(message)) => {
            let persist_result = PendingTransactionStore::for_home(storage.home()).remember(
                network,
                &record.address,
                &tx_hash_hex,
                "transaction",
            );
            match persist_result {
                Ok(()) => Err(format!(
                    "Transaction submission status is uncertain for {tx_hash_hex}: {message}. A pending record was saved; Fresnica will check this hash before allowing another write from the account."
                )),
                Err(persist_error) => Err(format!(
                    "Transaction submission status is uncertain for {tx_hash_hex}: {message}. Fresnica could not persist pending-transaction protection: {persist_error}. Do not retry until you verify the transaction hash manually."
                )),
            }
        }
    }
}

pub fn sign_transaction_xdr_with_passcode(
    record: &WalletRecord,
    network: &str,
    transaction_xdr: Vec<u8>,
    passcode: String,
) -> Result<Vec<u8>, String> {
    if record.watch_only() || record.secret.is_none() {
        return Err(format!("wallet \"{}\" is watch-only", record.name));
    }
    let protected = record
        .secret
        .as_ref()
        .ok_or_else(|| "wallet has no protected signing material".to_owned())?;
    let protected_json = serde_json::to_string(protected)
        .map_err(|error| format!("Unable to encode protected signing material: {error}"))?;
    let signed_xdr = FresnicaSdk::new()
        .sign_transaction_xdr_with_passcode(
            protected_json,
            passcode,
            record.address.clone(),
            transaction_xdr,
            network_passphrase(network)?.to_owned(),
        )
        .map_err(|error| match error.code {
            SdkErrorCode::InvalidPasscode => {
                "Unable to unlock wallet: invalid Fresnica passcode".to_owned()
            }
            _ => format!("Unable to sign transaction: {error}"),
        })?;
    Ok(signed_xdr)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct PendingTransaction {
    network: String,
    account: String,
    tx_hash: String,
    kind: String,
    submitted_at: String,
}

struct PendingTransactionStore {
    path: PathBuf,
    ttl_seconds: i64,
}

impl PendingTransactionStore {
    fn for_home(home: &Path) -> Self {
        Self {
            path: home.join("pending-transactions.json"),
            ttl_seconds: PENDING_TTL_SECONDS,
        }
    }

    fn remember(
        &self,
        network: &str,
        account: &str,
        tx_hash: &str,
        kind: &str,
    ) -> Result<(), String> {
        let submitted_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|error| format!("unable to format pending transaction time: {error}"))?;
        self.put(PendingTransaction {
            network: network.to_owned(),
            account: account.to_owned(),
            tx_hash: tx_hash.to_owned(),
            kind: kind.to_owned(),
            submitted_at,
        })
    }

    fn reconcile_and_ensure_clear(
        &self,
        network: &str,
        account: &str,
        horizon: &HorizonClient,
    ) -> Result<(), String> {
        self.reconcile_with(network, account, |tx_hash| horizon.get_transaction(tx_hash))
    }

    fn reconcile_with<F>(&self, network: &str, account: &str, mut lookup: F) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<Option<Value>, String>,
    {
        let items = self.load()?;
        if !items
            .iter()
            .any(|item| item.network == network && item.account == account)
        {
            return Ok(());
        }

        let now = OffsetDateTime::now_utc();
        let mut retained = Vec::with_capacity(items.len());
        let mut pending_hash = None;
        let mut changed = false;

        for item in items {
            if item.network != network || item.account != account {
                retained.push(item);
                continue;
            }

            match lookup(&item.tx_hash)? {
                Some(_) => changed = true,
                None if pending_age_seconds(&item, now) >= self.ttl_seconds => changed = true,
                None => {
                    if pending_hash.is_none() {
                        pending_hash = Some(item.tx_hash.clone());
                    }
                    retained.push(item);
                }
            }
        }

        if changed {
            self.save(&retained)?;
        }
        if let Some(tx_hash) = pending_hash {
            return Err(format!(
                "A previous transaction is still pending confirmation: {tx_hash}"
            ));
        }
        Ok(())
    }

    fn put(&self, pending: PendingTransaction) -> Result<(), String> {
        validate_pending(&pending)?;
        let mut items = self.load()?;
        items.retain(|item| {
            !(item.network == pending.network
                && item.account == pending.account
                && item.tx_hash == pending.tx_hash)
        });
        items.push(pending);
        self.save(&items)
    }

    fn load(&self) -> Result<Vec<PendingTransaction>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.path).map_err(|error| {
            format!(
                "Unable to read pending transaction state {}: {error}",
                self.path.display()
            )
        })?;
        let items: Vec<PendingTransaction> = serde_json::from_str(&text).map_err(|error| {
            format!(
                "Pending transaction state is malformed {}: {error}",
                self.path.display()
            )
        })?;
        for item in &items {
            validate_pending(item)?;
        }
        Ok(items)
    }

    fn save(&self, items: &[PendingTransaction]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Unable to create pending transaction directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let text = serde_json::to_string_pretty(items)
            .map_err(|error| format!("Unable to encode pending transaction state: {error}"))?
            + "\n";
        let mut temporary_name: OsString = self.path.as_os_str().to_owned();
        temporary_name.push(".tmp");
        let temporary = PathBuf::from(temporary_name);
        let result = (|| {
            fs::write(&temporary, &text).map_err(|error| {
                format!(
                    "Unable to persist pending transaction state {}: {error}",
                    temporary.display()
                )
            })?;
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path).map_err(|error| {
                    format!(
                        "Unable to replace pending transaction state {}: {error}",
                        self.path.display()
                    )
                })?;
            }
            fs::rename(&temporary, &self.path).map_err(|error| {
                format!(
                    "Unable to replace pending transaction state {}: {error}",
                    self.path.display()
                )
            })?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn validate_pending(item: &PendingTransaction) -> Result<(), String> {
    if [
        item.network.as_str(),
        item.account.as_str(),
        item.tx_hash.as_str(),
        item.kind.as_str(),
        item.submitted_at.as_str(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err(
            "Pending transaction state is malformed: fields must be non-empty strings".to_owned(),
        );
    }
    Ok(())
}

fn pending_age_seconds(pending: &PendingTransaction, now: OffsetDateTime) -> i64 {
    let Ok(submitted) = OffsetDateTime::parse(&pending.submitted_at, &Rfc3339) else {
        return i64::MAX;
    };
    (now - submitted).whole_seconds().max(0)
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
    parse_stroops(value, false).map_err(|_| format!("Horizon returned invalid {field}: {value}"))
}

pub fn parse_positive_stroops(value: &str) -> Result<i64, String> {
    parse_stroops(value, true)
        .map_err(|_| "Amount must be greater than zero with at most 7 decimal places".to_owned())
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

pub fn network_passphrase(network: &str) -> Result<&'static str, String> {
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
    use time::Duration;

    const SOURCE: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    fn pending_store(label: &str) -> PendingTransactionStore {
        let nonce = OffsetDateTime::now_utc().unix_timestamp_nanos();
        let home = std::env::temp_dir().join(format!(
            "fresnica-pending-{label}-{}-{nonce}",
            std::process::id()
        ));
        PendingTransactionStore::for_home(&home)
    }

    #[test]
    fn exact_stroop_parser_rejects_rounding() {
        assert_eq!(parse_positive_stroops("1.2345678").unwrap(), 12_345_678);
        assert!(parse_positive_stroops("1.23456789").is_err());
        assert!(parse_positive_stroops("0").is_err());
    }

    #[test]
    fn multi_operation_builder_multiplies_base_fee() {
        let body = OperationBody::BumpSequence(stellar_xdr::BumpSequenceOp {
            bump_to: SequenceNumber(9),
        });
        let envelope =
            build_operation_envelope(SOURCE, vec![body.clone(), body], 7, 100, None).unwrap();
        let TransactionEnvelope::Tx(envelope) = envelope else {
            panic!("expected v1 transaction envelope");
        };
        assert_eq!(envelope.tx.fee, 200);
        assert_eq!(envelope.tx.operations.len(), 2);
        assert_eq!(envelope.tx.seq_num, SequenceNumber(8));
    }

    #[test]
    fn expired_prepared_transaction_is_rejected_before_signing() {
        let body = OperationBody::BumpSequence(stellar_xdr::BumpSequenceOp {
            bump_to: SequenceNumber(9),
        });
        let mut envelope = build_operation_envelope(SOURCE, vec![body], 7, 100, None).unwrap();
        let TransactionEnvelope::Tx(transaction) = &mut envelope else {
            panic!("expected v1 transaction envelope");
        };
        transaction.tx.cond = Preconditions::Time(TimeBounds {
            min_time: TimePoint(0),
            max_time: TimePoint(100),
        });

        assert!(ensure_transaction_not_expired_at(&envelope, 100).is_ok());
        assert_eq!(
            ensure_transaction_not_expired_at(&envelope, 101).unwrap_err(),
            "Prepared transaction has expired; prepare and review the transaction again before signing"
        );
    }

    #[test]
    fn pending_store_persists_public_metadata_only() {
        let store = pending_store("metadata");
        store
            .remember("testnet", "GACCOUNT", "abc123", "transaction")
            .unwrap();
        let text = fs::read_to_string(&store.path).unwrap();
        let raw: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(raw[0]["network"], "testnet");
        assert_eq!(raw[0]["account"], "GACCOUNT");
        assert_eq!(raw[0]["tx_hash"], "abc123");
        assert_eq!(raw[0]["kind"], "transaction");
        assert!(!text.to_ascii_lowercase().contains("secret"));
        assert!(!text.to_ascii_lowercase().contains("xdr"));
    }

    #[test]
    fn recent_not_found_pending_transaction_blocks_retry() {
        let store = pending_store("pending");
        store
            .remember("testnet", "GACCOUNT", "abc123", "transaction")
            .unwrap();
        let error = store
            .reconcile_with("testnet", "GACCOUNT", |_| Ok(None))
            .unwrap_err();
        assert_eq!(
            error,
            "A previous transaction is still pending confirmation: abc123"
        );
        assert_eq!(store.load().unwrap().len(), 1);
    }

    #[test]
    fn confirmed_pending_transaction_is_removed() {
        let store = pending_store("confirmed");
        store
            .remember("testnet", "GACCOUNT", "abc123", "transaction")
            .unwrap();
        store
            .reconcile_with("testnet", "GACCOUNT", |tx_hash| {
                Ok(Some(serde_json::json!({"hash": tx_hash})))
            })
            .unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn old_not_found_pending_transaction_expires() {
        let store = pending_store("expired");
        let submitted_at = (OffsetDateTime::now_utc() - Duration::minutes(10))
            .format(&Rfc3339)
            .unwrap();
        store
            .put(PendingTransaction {
                network: "testnet".to_owned(),
                account: "GACCOUNT".to_owned(),
                tx_hash: "abc123".to_owned(),
                kind: "transaction".to_owned(),
                submitted_at,
            })
            .unwrap();
        store
            .reconcile_with("testnet", "GACCOUNT", |_| Ok(None))
            .unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn pending_lookup_failure_keeps_record() {
        let store = pending_store("lookup-error");
        store
            .remember("testnet", "GACCOUNT", "abc123", "transaction")
            .unwrap();
        assert_eq!(
            store
                .reconcile_with("testnet", "GACCOUNT", |_| Err("offline".to_owned()))
                .unwrap_err(),
            "offline"
        );
        assert_eq!(store.load().unwrap().len(), 1);
    }
}
