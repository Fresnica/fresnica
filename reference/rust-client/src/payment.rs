use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use stellar_xdr::{
    AccountId, CreateAccountOp, Hash, Memo, MuxedAccount, OperationBody, PaymentOp, PublicKey,
    StringM, TransactionEnvelope,
};

use crate::asset::AssetId;
use crate::{
    account_sequence, balance_stroops, build_single_operation_envelope_with_memo, format_stroops,
    minimum_balance_stroops, parse_positive_stroops, resolve_destination, resolve_write_wallet,
    sign_and_submit, FresnicaClient, LedgerParameters, TransactionSubmission, WalletRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentMemo {
    None,
    Text(String),
    Id(u64),
    Hash([u8; 32]),
}

impl PaymentMemo {
    pub fn from_text(value: Option<&str>) -> Self {
        match value.filter(|value| !value.is_empty()) {
            Some(value) => Self::Text(value.to_owned()),
            None => Self::None,
        }
    }

    pub fn from_anchor_fields(memo_type: Option<&str>, memo: Option<&str>) -> Result<Self, String> {
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

    fn review(&self) -> Option<PaymentMemoReview> {
        match self {
            Self::None => None,
            Self::Text(value) => Some(PaymentMemoReview {
                memo_type: "text".to_owned(),
                value: value.clone(),
            }),
            Self::Id(value) => Some(PaymentMemoReview {
                memo_type: "id".to_owned(),
                value: value.to_string(),
            }),
            Self::Hash(value) => Some(PaymentMemoReview {
                memo_type: "hash".to_owned(),
                value: STANDARD.encode(value),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentMemoReview {
    pub memo_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentOperation {
    Payment,
    CreateAccount,
}

impl PaymentOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Payment => "Payment",
            Self::CreateAccount => "CreateAccount",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentRequest {
    pub wallet: Option<String>,
    pub amount: String,
    pub asset: String,
    pub destination: String,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentReview {
    pub operation: PaymentOperation,
    pub wallet_name: String,
    pub source: String,
    pub destination: String,
    pub contact_name: Option<String>,
    pub amount: String,
    pub asset: String,
    pub fee_xlm: String,
    pub network: String,
    pub memo: Option<PaymentMemoReview>,
}

#[derive(Debug, Clone)]
pub struct PreparedPayment {
    pub review: PaymentReview,
    wallet: WalletRecord,
    envelope: TransactionEnvelope,
}

impl FresnicaClient {
    pub fn prepare_payment(&self, request: &PaymentRequest) -> Result<PreparedPayment, String> {
        let wallet = resolve_write_wallet(
            self.storage(),
            self.gateway(),
            self.network(),
            request.wallet.as_deref(),
        )?;
        let resolved = resolve_destination(
            self.storage(),
            &request.destination,
            request.memo.as_deref(),
        )?;
        self.prepare_payment_with_wallet(
            wallet,
            &request.amount,
            &request.asset,
            &resolved.address,
            resolved.contact_name.as_deref(),
            PaymentMemo::from_text(resolved.memo.as_deref()),
        )
    }

    pub fn prepare_payment_to_address(
        &self,
        wallet: &WalletRecord,
        amount_text: &str,
        asset_text: &str,
        destination_address: &str,
        contact_name: Option<&str>,
        memo: PaymentMemo,
    ) -> Result<PreparedPayment, String> {
        let current = resolve_write_wallet(
            self.storage(),
            self.gateway(),
            self.network(),
            Some(&wallet.name),
        )?;
        if current.address != wallet.address {
            return Err(format!(
                "wallet identity changed while preparing payment: {}",
                wallet.name
            ));
        }
        self.prepare_payment_with_wallet(
            current,
            amount_text,
            asset_text,
            destination_address,
            contact_name,
            memo,
        )
    }

    fn prepare_payment_with_wallet(
        &self,
        current: WalletRecord,
        amount_text: &str,
        asset_text: &str,
        destination_address: &str,
        contact_name: Option<&str>,
        memo: PaymentMemo,
    ) -> Result<PreparedPayment, String> {
        let asset = AssetId::parse(asset_text)?;
        let amount = parse_positive_stroops(amount_text)?;
        let destination = AccountId::from_str(destination_address)
            .map_err(|_| "destination must be a Classic Stellar G address".to_owned())?;
        let memo_xdr = memo.to_xdr()?;

        let account = self.gateway().get_account(&current.address)?;
        let destination_exists = self.gateway().account_exists(destination_address)?;
        if !destination_exists && !asset.is_native() {
            return Err(
                "Destination account does not exist. Only XLM can create a new Stellar account; issued assets require an existing account and trustline."
                    .to_owned(),
            );
        }
        let destination_account = if destination_exists {
            Some(self.gateway().get_account(destination_address)?)
        } else {
            None
        };
        let ledger = self.gateway().get_ledger_parameters()?;
        validate_transfer(&account, &current.address, &asset, amount, ledger)?;
        if let Some(destination_account) = destination_account.as_ref() {
            validate_destination_receive(destination_account, destination_address, &asset, amount)?;
            if matches!(&memo, PaymentMemo::None) && account_requires_memo(destination_account)? {
                return Err(format!(
                    "Destination {destination_address} requires a transaction memo (SEP-29). Add a memo and try again."
                ));
            }
        }
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

        let create_destination = !destination_exists;
        let body = payment_body(destination, &asset, amount, create_destination)?;
        let envelope = build_single_operation_envelope_with_memo(
            &current.address,
            body,
            account_sequence(&account)?,
            ledger.base_fee_in_stroops,
            memo_xdr,
        )?;
        let review = PaymentReview {
            operation: if create_destination {
                PaymentOperation::CreateAccount
            } else {
                PaymentOperation::Payment
            },
            wallet_name: current.name.clone(),
            source: current.address.clone(),
            destination: destination_address.to_owned(),
            contact_name: contact_name.map(str::to_owned),
            amount: format_stroops(amount),
            asset: asset.display(),
            fee_xlm: format_stroops(i64::from(ledger.base_fee_in_stroops)),
            network: current.network.clone(),
            memo: memo.review(),
        };
        Ok(PreparedPayment {
            review,
            wallet: current,
            envelope,
        })
    }

    pub fn submit_payment(
        &self,
        prepared: &PreparedPayment,
        passcode: &str,
    ) -> Result<TransactionSubmission, String> {
        let mut envelope = prepared.envelope.clone();
        sign_and_submit(
            self.storage(),
            &prepared.wallet,
            self.network(),
            &mut envelope,
            self.gateway(),
            passcode,
        )
    }
}

fn payment_body(
    destination: AccountId,
    asset: &AssetId,
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
        asset: asset.to_xdr(),
        amount,
    }))
}

fn validate_transfer(
    account: &Value,
    account_id: &str,
    asset: &AssetId,
    requested: i64,
    ledger: LedgerParameters,
) -> Result<(), String> {
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;

    if !asset.is_native() && asset.issuer_is(account_id) {
        ensure_payment_fee_capacity(account, ledger)?;
        return Ok(());
    }

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
            ensure_payment_trustline_authorized(raw_balance, asset)?;
            ensure_payment_fee_capacity(account, ledger)?;
            let balance = balance_stroops(raw_balance, "balance")?;
            let selling = balance_stroops(raw_balance, "selling_liabilities")?;
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

fn validate_destination_receive(
    account: &Value,
    account_id: &str,
    asset: &AssetId,
    requested: i64,
) -> Result<(), String> {
    if !asset.is_native() && asset.issuer_is(account_id) {
        return Ok(());
    }
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed destination balance data".to_owned())?;
    let raw = balances
        .iter()
        .find(|balance| asset.matches_balance(balance))
        .ok_or_else(|| format!("Destination has no trustline for {}", asset.display()))?;

    let capacity = if asset.is_native() {
        let committed = balance_stroops(raw, "balance")?
            .checked_add(balance_stroops(raw, "buying_liabilities")?)
            .ok_or_else(|| "destination native receiving capacity overflow".to_owned())?;
        i64::MAX.saturating_sub(committed)
    } else {
        ensure_payment_trustline_authorized(raw, asset)?;
        let limit = balance_stroops(raw, "limit")?;
        let committed = balance_stroops(raw, "balance")?
            .checked_add(balance_stroops(raw, "buying_liabilities")?)
            .ok_or_else(|| "destination trustline receiving capacity overflow".to_owned())?;
        limit.saturating_sub(committed).max(0)
    };
    if requested > capacity {
        return Err(format!(
            "Insufficient receiving capacity for {} at destination: need {}, available {}",
            asset.display(),
            format_stroops(requested),
            format_stroops(capacity)
        ));
    }
    Ok(())
}

fn ensure_payment_fee_capacity(account: &Value, ledger: LedgerParameters) -> Result<(), String> {
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
    let native = balances
        .iter()
        .find(|balance| text(balance, "asset_type") == Some("native"))
        .ok_or_else(|| "No XLM balance is available to pay the transaction fee".to_owned())?;
    let balance = balance_stroops(native, "balance")?;
    let selling = balance_stroops(native, "selling_liabilities")?;
    let minimum = minimum_balance_stroops(account, ledger.base_reserve_in_stroops)?;
    let free = balance
        .saturating_sub(selling)
        .saturating_sub(minimum)
        .max(0);
    let fee = i64::from(ledger.base_fee_in_stroops);
    if free < fee {
        return Err(format!(
            "Insufficient XLM for transaction fee: need {}, available {}",
            format_stroops(fee),
            format_stroops(free)
        ));
    }
    Ok(())
}

fn ensure_payment_trustline_authorized(raw: &Value, asset: &AssetId) -> Result<(), String> {
    match raw.get("is_authorized").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "Trustline for {} is not fully authorized for payment",
            asset.display()
        )),
        None => Err(format!(
            "Horizon returned malformed authorization state for {}",
            asset.display()
        )),
    }
}

fn account_requires_memo(account: &Value) -> Result<bool, String> {
    let Some(encoded) = account
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("config.memo_required"))
        .and_then(Value::as_str)
    else {
        return Ok(false);
    };
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| "Horizon returned malformed config.memo_required data".to_owned())?;
    Ok(decoded.as_slice() == b"1")
}

fn account_id_to_muxed(account: &AccountId) -> MuxedAccount {
    match &account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => MuxedAccount::Ed25519(key.clone()),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::Asset;

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
    fn shared_asset_identity_vectors_preserve_exact_code_and_issuer() {
        let vectors: Value = serde_json::from_str(include_str!(
            "../../../spec/test-vectors/asset-identity-v1.json"
        ))
        .unwrap();

        assert_eq!(
            vectors["schema"].as_str(),
            Some("fresnica-asset-identity-v1")
        );
        let mut identities = std::collections::BTreeMap::new();
        for case in vectors["issued_assets"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let code = case["code"].as_str().unwrap();
            let issuer = case["issuer"].as_str().unwrap();
            let identity = case["identity"].as_str().unwrap();
            let asset = AssetId::parse(identity).unwrap();

            assert!(!asset.is_native(), "{name}");
            assert_eq!(asset.display(), identity, "{name}");
            assert_eq!(asset.code().as_deref(), Some(code), "{name}");
            assert_eq!(asset.issuer().as_deref(), Some(issuer), "{name}");
            assert!(matches!(
                (case["asset_type"].as_str().unwrap(), asset.to_xdr()),
                ("credit_alphanum4", Asset::CreditAlphanum4(_))
                    | ("credit_alphanum12", Asset::CreditAlphanum12(_))
            ));
            identities.insert(name.to_owned(), asset);
        }

        for pair in vectors["distinct_identity_pairs"].as_array().unwrap() {
            let left = pair[0].as_str().unwrap();
            let right = pair[1].as_str().unwrap();
            assert_ne!(
                identities.get(left),
                identities.get(right),
                "{left} vs {right}"
            );
        }
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
            "GSOURCE",
            &AssetId::native(),
            parse_positive_stroops("7.99999").unwrap(),
            ledger,
        )
        .is_ok());
        assert!(validate_transfer(
            &account,
            "GSOURCE",
            &AssetId::native(),
            parse_positive_stroops("8").unwrap(),
            ledger,
        )
        .is_err());
    }

    #[test]
    fn issuer_can_send_own_asset_without_a_source_trustline() {
        let source = DESTINATION;
        let account = serde_json::json!({
            "subentry_count": 0,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [{
                "asset_type": "native",
                "balance": "10",
                "selling_liabilities": "0",
                "buying_liabilities": "0"
            }]
        });
        let ledger = LedgerParameters {
            base_fee_in_stroops: 100,
            base_reserve_in_stroops: 5_000_000,
        };
        let asset = AssetId::parse(&format!("USD:{source}")).unwrap();
        assert!(validate_transfer(&account, source, &asset, 1_000_000_000, ledger).is_ok());
    }

    #[test]
    fn destination_credit_requires_full_authorization_and_receiving_headroom() {
        let asset = AssetId::parse(&format!("USD:{DESTINATION}")).unwrap();
        let destination = serde_json::json!({
            "balances": [{
                "asset_type": "credit_alphanum4",
                "asset_code": "USD",
                "asset_issuer": DESTINATION,
                "balance": "9.5",
                "buying_liabilities": "0.25",
                "selling_liabilities": "0",
                "limit": "10",
                "is_authorized": true
            }]
        });
        // The destination is the issuer here, so redemption is a special case.
        assert!(validate_destination_receive(
            &destination,
            DESTINATION,
            &asset,
            parse_positive_stroops("100").unwrap(),
        )
        .is_ok());

        let holder = "GHOLDER";
        assert!(validate_destination_receive(
            &destination,
            holder,
            &asset,
            parse_positive_stroops("0.25").unwrap(),
        )
        .is_ok());
        assert!(validate_destination_receive(
            &destination,
            holder,
            &asset,
            parse_positive_stroops("0.2500001").unwrap(),
        )
        .is_err());

        let mut unauthorized = destination;
        unauthorized["balances"][0]["is_authorized"] = Value::Bool(false);
        assert!(validate_destination_receive(&unauthorized, holder, &asset, 1,).is_err());
    }

    #[test]
    fn native_destination_capacity_respects_int64_headroom() {
        let destination = serde_json::json!({
            "balances": [{
                "asset_type": "native",
                "balance": "922337203685.4775800",
                "buying_liabilities": "0",
                "selling_liabilities": "0"
            }]
        });
        assert!(
            validate_destination_receive(&destination, DESTINATION, &AssetId::native(), 7,).is_ok()
        );
        assert!(
            validate_destination_receive(&destination, DESTINATION, &AssetId::native(), 8,)
                .is_err()
        );
    }

    #[test]
    fn sep29_memo_required_decodes_horizon_account_data() {
        let account = serde_json::json!({
            "data": {"config.memo_required": "MQ=="}
        });
        assert!(account_requires_memo(&account).unwrap());
        let unset = serde_json::json!({"data": {}});
        assert!(!account_requires_memo(&unset).unwrap());
    }

    #[test]
    fn payment_body_switches_to_create_account_for_missing_destination() {
        let destination = AccountId::from_str(DESTINATION).unwrap();
        assert!(matches!(
            payment_body(destination.clone(), &AssetId::native(), 10_000_000, false).unwrap(),
            OperationBody::Payment(_)
        ));
        assert!(matches!(
            payment_body(destination, &AssetId::native(), 10_000_000, true).unwrap(),
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
}
