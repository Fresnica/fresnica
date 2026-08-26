use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, ChangeTrustAsset,
    ChangeTrustOp, ManageBuyOfferOp, ManageSellOfferOp, OperationBody, Price, TransactionEnvelope,
};

use crate::{
    account_sequence, balance_stroops, build_operation_envelope, format_stroops,
    minimum_balance_stroops, parse_stroops, resolve_signing_wallet, sign_and_submit,
    FresnicaClient, TransactionSubmission, WalletRecord, DEFAULT_TRUSTLINE_LIMIT, STROOPS_PER_XLM,
};

const INT32_MAX: i64 = i32::MAX as i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferSide {
    Buy,
    Sell,
}

impl OfferSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }

    pub fn operation_label(self) -> &'static str {
        match self {
            Self::Buy => "ManageBuyOffer",
            Self::Sell => "ManageSellOffer",
        }
    }

    fn assets<'a>(
        self,
        base: &'a OfferAsset,
        counter: &'a OfferAsset,
    ) -> (&'a OfferAsset, &'a OfferAsset) {
        match self {
            Self::Buy => (counter, base),
            Self::Sell => (base, counter),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferAction {
    Create,
    Update,
    Cancel,
}

impl OfferAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferOperation {
    ManageBuyOffer,
    ManageSellOffer,
}

impl OfferOperation {
    pub fn label(self) -> &'static str {
        match self {
            Self::ManageBuyOffer => "ManageBuyOffer",
            Self::ManageSellOffer => "ManageSellOffer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferRequest {
    Create {
        wallet: Option<String>,
        side: OfferSide,
        base: String,
        counter: String,
        amount: String,
        price: String,
        allow_trustline: bool,
    },
    Update {
        wallet: Option<String>,
        offer_id: i64,
        base: String,
        counter: String,
        amount: String,
        price: String,
    },
    Cancel {
        wallet: Option<String>,
        offer_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferReviewDetails {
    Trade {
        side: OfferSide,
        base: String,
        counter: String,
        amount: String,
        price: String,
        total: String,
        trustline_asset: Option<String>,
    },
    Cancel {
        selling: String,
        buying: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferReview {
    pub action: OfferAction,
    pub operation: OfferOperation,
    pub wallet_name: String,
    pub source: String,
    pub offer_id: Option<i64>,
    pub fee_xlm: String,
    pub network: String,
    pub details: OfferReviewDetails,
}

#[derive(Debug, Clone)]
pub struct PreparedOffer {
    pub review: OfferReview,
    wallet: WalletRecord,
    envelope: TransactionEnvelope,
}

impl FresnicaClient {
    pub fn prepare_offer(&self, request: &OfferRequest) -> Result<PreparedOffer, String> {
        match request {
            OfferRequest::Create {
                wallet,
                side,
                base,
                counter,
                amount,
                price,
                allow_trustline,
            } => self.prepare_offer_create(
                wallet.as_deref(),
                *side,
                base,
                counter,
                amount,
                price,
                *allow_trustline,
            ),
            OfferRequest::Update {
                wallet,
                offer_id,
                base,
                counter,
                amount,
                price,
            } => self.prepare_offer_update(
                wallet.as_deref(),
                *offer_id,
                base,
                counter,
                amount,
                price,
            ),
            OfferRequest::Cancel { wallet, offer_id } => {
                self.prepare_offer_cancel(wallet.as_deref(), *offer_id)
            }
        }
    }

    pub fn submit_offer(
        &self,
        prepared: &PreparedOffer,
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

    fn prepare_offer_create(
        &self,
        wallet_name: Option<&str>,
        side: OfferSide,
        base_text: &str,
        counter_text: &str,
        amount_text: &str,
        price_text: &str,
        allow_trustline: bool,
    ) -> Result<PreparedOffer, String> {
        let wallet =
            resolve_signing_wallet(self.storage(), self.horizon(), self.network(), wallet_name)?;
        let base = OfferAsset::parse(base_text)?;
        let counter = OfferAsset::parse(counter_text)?;
        ensure_pair(&base, &counter)?;
        let amount = parse_offer_value(amount_text, "amount")?;
        let price_stroops = parse_offer_value(price_text, "price")?;
        let price = stellar_price(price_stroops)?;

        let account = self.horizon().get_account(&wallet.address)?;
        let ledger = self.horizon().get_ledger_parameters()?;
        let (selling, buying) = side.assets(&base, &counter);
        let adds_trustline = !account_can_hold(&account, buying, &wallet.address);
        if adds_trustline && !allow_trustline {
            return Err(format!(
                "Receiving trustline is missing for {}. Add it first or explicitly allow adding the trustline.",
                buying.display()
            ));
        }

        let operation_count = if adds_trustline { 2_u32 } else { 1_u32 };
        let total_fee = i64::from(
            ledger
                .base_fee_in_stroops
                .checked_mul(operation_count)
                .ok_or_else(|| "transaction fee overflow".to_owned())?,
        );
        let extra_subentries = 1_i64 + if adds_trustline { 1 } else { 0 };
        preflight_create(
            &account,
            &wallet.address,
            side,
            selling,
            amount,
            price_stroops,
            ledger.base_reserve_in_stroops,
            total_fee,
            extra_subentries,
        )?;

        let mut operations = Vec::with_capacity(operation_count as usize);
        if adds_trustline {
            operations.push(OperationBody::ChangeTrust(ChangeTrustOp {
                line: buying.to_change_trust_xdr()?,
                limit: parse_stroops(DEFAULT_TRUSTLINE_LIMIT, true)
                    .map_err(|_| "invalid Fresnica trustline policy limit".to_owned())?,
            }));
        }
        operations.push(offer_operation(side, &base, &counter, amount, price, 0)?);
        let envelope = build_operation_envelope(
            &wallet.address,
            operations,
            account_sequence(&account)?,
            ledger.base_fee_in_stroops,
            None,
        )?;

        Ok(PreparedOffer {
            review: OfferReview {
                action: OfferAction::Create,
                operation: operation_for_side(side),
                wallet_name: wallet.name.clone(),
                source: wallet.address.clone(),
                offer_id: None,
                fee_xlm: format_stroops(total_fee),
                network: wallet.network.clone(),
                details: OfferReviewDetails::Trade {
                    side,
                    base: base.display(),
                    counter: counter.display(),
                    amount: format_stroops(amount),
                    price: format_stroops(price_stroops),
                    total: format_scaled_product(amount, price_stroops),
                    trustline_asset: adds_trustline.then(|| buying.display()),
                },
            },
            wallet,
            envelope,
        })
    }

    fn prepare_offer_update(
        &self,
        wallet_name: Option<&str>,
        offer_id: i64,
        base_text: &str,
        counter_text: &str,
        amount_text: &str,
        price_text: &str,
    ) -> Result<PreparedOffer, String> {
        validate_offer_id(offer_id)?;
        let wallet =
            resolve_signing_wallet(self.storage(), self.horizon(), self.network(), wallet_name)?;
        let base = OfferAsset::parse(base_text)?;
        let counter = OfferAsset::parse(counter_text)?;
        ensure_pair(&base, &counter)?;
        let amount = parse_offer_value(amount_text, "amount")?;
        let price_stroops = parse_offer_value(price_text, "price")?;
        let price = stellar_price(price_stroops)?;

        let raw_offer = self.horizon().get_offer(offer_id)?;
        ensure_offer_owner(&raw_offer, &wallet)?;
        let stored_selling = OfferAsset::from_horizon(
            raw_offer
                .get("selling")
                .ok_or_else(|| "Horizon returned malformed offer selling asset".to_owned())?,
        )?;
        let stored_buying = OfferAsset::from_horizon(
            raw_offer
                .get("buying")
                .ok_or_else(|| "Horizon returned malformed offer buying asset".to_owned())?,
        )?;
        let side = infer_side(&stored_selling, &stored_buying, &base, &counter)?;

        let account = self.horizon().get_account(&wallet.address)?;
        let ledger = self.horizon().get_ledger_parameters()?;
        let body = offer_operation(side, &base, &counter, amount, price, offer_id)?;
        let envelope = build_operation_envelope(
            &wallet.address,
            vec![body],
            account_sequence(&account)?,
            ledger.base_fee_in_stroops,
            None,
        )?;
        let total_fee = i64::from(ledger.base_fee_in_stroops);

        Ok(PreparedOffer {
            review: OfferReview {
                action: OfferAction::Update,
                operation: operation_for_side(side),
                wallet_name: wallet.name.clone(),
                source: wallet.address.clone(),
                offer_id: Some(offer_id),
                fee_xlm: format_stroops(total_fee),
                network: wallet.network.clone(),
                details: OfferReviewDetails::Trade {
                    side,
                    base: base.display(),
                    counter: counter.display(),
                    amount: format_stroops(amount),
                    price: format_stroops(price_stroops),
                    total: format_scaled_product(amount, price_stroops),
                    trustline_asset: None,
                },
            },
            wallet,
            envelope,
        })
    }

    fn prepare_offer_cancel(
        &self,
        wallet_name: Option<&str>,
        offer_id: i64,
    ) -> Result<PreparedOffer, String> {
        validate_offer_id(offer_id)?;
        let wallet =
            resolve_signing_wallet(self.storage(), self.horizon(), self.network(), wallet_name)?;
        let raw_offer = self.horizon().get_offer(offer_id)?;
        ensure_offer_owner(&raw_offer, &wallet)?;
        let selling = OfferAsset::from_horizon(
            raw_offer
                .get("selling")
                .ok_or_else(|| "Horizon returned malformed offer selling asset".to_owned())?,
        )?;
        let buying = OfferAsset::from_horizon(
            raw_offer
                .get("buying")
                .ok_or_else(|| "Horizon returned malformed offer buying asset".to_owned())?,
        )?;
        let price = horizon_price(&raw_offer)?;

        // Cancellation is intentionally canonicalized to the ledger's stored
        // selling/buying orientation. It does not require knowing whether the
        // original operation was ManageBuyOffer or ManageSellOffer.
        let body = OperationBody::ManageSellOffer(ManageSellOfferOp {
            selling: selling.to_xdr()?,
            buying: buying.to_xdr()?,
            amount: 0,
            price,
            offer_id,
        });
        let account = self.horizon().get_account(&wallet.address)?;
        let ledger = self.horizon().get_ledger_parameters()?;
        let envelope = build_operation_envelope(
            &wallet.address,
            vec![body],
            account_sequence(&account)?,
            ledger.base_fee_in_stroops,
            None,
        )?;

        Ok(PreparedOffer {
            review: OfferReview {
                action: OfferAction::Cancel,
                operation: OfferOperation::ManageSellOffer,
                wallet_name: wallet.name.clone(),
                source: wallet.address.clone(),
                offer_id: Some(offer_id),
                fee_xlm: format_stroops(i64::from(ledger.base_fee_in_stroops)),
                network: wallet.network.clone(),
                details: OfferReviewDetails::Cancel {
                    selling: selling.display(),
                    buying: buying.display(),
                },
            },
            wallet,
            envelope,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OfferAsset {
    Native,
    Credit { code: String, issuer: String },
}

impl OfferAsset {
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

    fn from_horizon(value: &Value) -> Result<Self, String> {
        if text(value, "asset_type") == Some("native") {
            return Ok(Self::Native);
        }
        let code = text(value, "asset_code")
            .ok_or_else(|| "Horizon returned malformed offer asset code".to_owned())?;
        let issuer = text(value, "asset_issuer")
            .ok_or_else(|| "Horizon returned malformed offer asset issuer".to_owned())?;
        Self::parse(&format!("{code}:{issuer}"))
    }

    fn display(&self) -> String {
        match self {
            Self::Native => "XLM".to_owned(),
            Self::Credit { code, issuer } => format!("{code}:{issuer}"),
        }
    }

    fn is_native(&self) -> bool {
        matches!(self, Self::Native)
    }

    fn issuer(&self) -> Option<&str> {
        match self {
            Self::Native => None,
            Self::Credit { issuer, .. } => Some(issuer),
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

    fn to_change_trust_xdr(&self) -> Result<ChangeTrustAsset, String> {
        match self {
            Self::Native => Err("XLM does not use a trustline".to_owned()),
            Self::Credit { code, issuer } => {
                let issuer = AccountId::from_str(issuer)
                    .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
                if code.len() <= 4 {
                    let mut raw = [0u8; 4];
                    raw[..code.len()].copy_from_slice(code.as_bytes());
                    Ok(ChangeTrustAsset::CreditAlphanum4(AlphaNum4 {
                        asset_code: AssetCode4(raw),
                        issuer,
                    }))
                } else {
                    let mut raw = [0u8; 12];
                    raw[..code.len()].copy_from_slice(code.as_bytes());
                    Ok(ChangeTrustAsset::CreditAlphanum12(AlphaNum12 {
                        asset_code: AssetCode12(raw),
                        issuer,
                    }))
                }
            }
        }
    }
}

fn operation_for_side(side: OfferSide) -> OfferOperation {
    match side {
        OfferSide::Buy => OfferOperation::ManageBuyOffer,
        OfferSide::Sell => OfferOperation::ManageSellOffer,
    }
}

fn offer_operation(
    side: OfferSide,
    base: &OfferAsset,
    counter: &OfferAsset,
    amount: i64,
    price: Price,
    offer_id: i64,
) -> Result<OperationBody, String> {
    match side {
        OfferSide::Buy => Ok(OperationBody::ManageBuyOffer(ManageBuyOfferOp {
            selling: counter.to_xdr()?,
            buying: base.to_xdr()?,
            buy_amount: amount,
            price,
            offer_id,
        })),
        OfferSide::Sell => Ok(OperationBody::ManageSellOffer(ManageSellOfferOp {
            selling: base.to_xdr()?,
            buying: counter.to_xdr()?,
            amount,
            price,
            offer_id,
        })),
    }
}

fn ensure_pair(base: &OfferAsset, counter: &OfferAsset) -> Result<(), String> {
    if base == counter {
        Err("Offer assets must be different".to_owned())
    } else {
        Ok(())
    }
}

fn infer_side(
    selling: &OfferAsset,
    buying: &OfferAsset,
    base: &OfferAsset,
    counter: &OfferAsset,
) -> Result<OfferSide, String> {
    if selling == base && buying == counter {
        Ok(OfferSide::Sell)
    } else if selling == counter && buying == base {
        Ok(OfferSide::Buy)
    } else {
        Err("Offer update must keep the current market pair and BUY/SELL side".to_owned())
    }
}

fn preflight_create(
    account: &Value,
    account_id: &str,
    side: OfferSide,
    selling: &OfferAsset,
    amount: i64,
    price_stroops: i64,
    base_reserve: i64,
    fee: i64,
    extra_subentries: i64,
) -> Result<(), String> {
    let required_selling = match side {
        OfferSide::Sell => amount,
        OfferSide::Buy => ceil_scaled_product(amount, price_stroops)?,
    };
    let available_selling = available_balance(account, selling)?;
    let issuer_can_sell = selling.issuer() == Some(account_id);
    let extra_reserve = base_reserve
        .checked_mul(extra_subentries)
        .ok_or_else(|| "offer reserve overflow".to_owned())?;
    let future_minimum = minimum_balance_stroops(account, base_reserve)?
        .checked_add(extra_reserve)
        .ok_or_else(|| "offer reserve overflow".to_owned())?;

    if selling.is_native() {
        let required = required_selling
            .checked_add(future_minimum)
            .and_then(|value| value.checked_add(fee))
            .ok_or_else(|| "offer requirement overflow".to_owned())?;
        if required > available_selling {
            return Err(format!(
                "Insufficient XLM for offer: need {}, available {}",
                format_stroops(required),
                format_stroops(available_selling)
            ));
        }
    } else {
        if !issuer_can_sell && required_selling > available_selling {
            return Err(format!(
                "Insufficient {} for offer: need {}, available {}",
                selling.display(),
                format_stroops(required_selling),
                format_stroops(available_selling)
            ));
        }
        let native = OfferAsset::Native;
        let available_native = available_balance(account, &native)?;
        let required_native = future_minimum
            .checked_add(fee)
            .ok_or_else(|| "offer XLM requirement overflow".to_owned())?;
        if required_native > available_native {
            return Err(format!(
                "Insufficient XLM for offer reserve and fee: need {}, available {}",
                format_stroops(required_native),
                format_stroops(available_native)
            ));
        }
    }
    Ok(())
}

fn available_balance(account: &Value, asset: &OfferAsset) -> Result<i64, String> {
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;
    let Some(raw) = balances
        .iter()
        .find(|balance| asset.matches_balance(balance))
    else {
        return Ok(0);
    };
    Ok(balance_stroops(raw, "balance")?
        .saturating_sub(balance_stroops(raw, "selling_liabilities")?)
        .max(0))
}

fn account_can_hold(account: &Value, asset: &OfferAsset, account_id: &str) -> bool {
    if asset.is_native() || asset.issuer() == Some(account_id) {
        return true;
    }
    account
        .get("balances")
        .and_then(Value::as_array)
        .is_some_and(|balances| {
            balances
                .iter()
                .any(|balance| asset.matches_balance(balance))
        })
}

fn ceil_scaled_product(left: i64, right: i64) -> Result<i64, String> {
    let product = i128::from(left)
        .checked_mul(i128::from(right))
        .ok_or_else(|| "offer total overflow".to_owned())?;
    let scale = i128::from(STROOPS_PER_XLM);
    let value = product
        .checked_add(scale - 1)
        .ok_or_else(|| "offer total overflow".to_owned())?
        / scale;
    i64::try_from(value).map_err(|_| "offer total overflow".to_owned())
}

fn parse_offer_value(value: &str, label: &str) -> Result<i64, String> {
    parse_stroops(value, true).map_err(|_| {
        format!("Offer {label} must be greater than zero with at most 7 decimal places")
    })
}

fn stellar_price(price_stroops: i64) -> Result<Price, String> {
    let mut numerator = price_stroops;
    let mut denominator = STROOPS_PER_XLM;
    let mut previous_n = 0_i64;
    let mut previous_d = 1_i64;
    let mut current_n = 1_i64;
    let mut current_d = 0_i64;
    let mut best_n = current_n;
    let mut best_d = current_d;

    loop {
        let a = numerator / denominator;
        if a > INT32_MAX {
            break;
        }
        let next_n = a
            .checked_mul(current_n)
            .and_then(|value| value.checked_add(previous_n))
            .ok_or_else(|| "Offer price has no Stellar int32 rational approximation".to_owned())?;
        let next_d = a
            .checked_mul(current_d)
            .and_then(|value| value.checked_add(previous_d))
            .ok_or_else(|| "Offer price has no Stellar int32 rational approximation".to_owned())?;
        if next_n > INT32_MAX || next_d > INT32_MAX {
            break;
        }
        best_n = next_n;
        best_d = next_d;
        previous_n = current_n;
        previous_d = current_d;
        current_n = next_n;
        current_d = next_d;

        let remainder = numerator % denominator;
        if remainder == 0 {
            break;
        }
        numerator = denominator;
        denominator = remainder;
    }

    if best_n <= 0 || best_d <= 0 {
        return Err("Offer price has no Stellar int32 rational approximation".to_owned());
    }
    Ok(Price {
        n: i32::try_from(best_n).map_err(|_| "Offer price numerator exceeds int32".to_owned())?,
        d: i32::try_from(best_d).map_err(|_| "Offer price denominator exceeds int32".to_owned())?,
    })
}

fn horizon_price(offer: &Value) -> Result<Price, String> {
    let ratio = offer
        .get("price_r")
        .and_then(Value::as_object)
        .ok_or_else(|| "Horizon returned malformed offer price ratio".to_owned())?;
    let n = integer(ratio.get("n"))
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "Horizon returned invalid offer price numerator".to_owned())?;
    let d = integer(ratio.get("d"))
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| "Horizon returned invalid offer price denominator".to_owned())?;
    Ok(Price { n, d })
}

fn ensure_offer_owner(offer: &Value, record: &WalletRecord) -> Result<(), String> {
    let seller = text(offer, "seller")
        .ok_or_else(|| "Horizon returned malformed offer seller".to_owned())?;
    if seller != record.address {
        return Err(format!(
            "Offer is owned by {seller}, not wallet {}",
            record.address
        ));
    }
    Ok(())
}

fn validate_offer_id(offer_id: i64) -> Result<(), String> {
    if offer_id > 0 {
        Ok(())
    } else {
        Err("offer id must be a positive integer".to_owned())
    }
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

fn format_scaled_product(left: i64, right: i64) -> String {
    let product = i128::from(left) * i128::from(right);
    let scale = i128::from(STROOPS_PER_XLM) * i128::from(STROOPS_PER_XLM);
    let whole = product / scale;
    let fraction = product % scale;
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:014}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
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

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn stellar_price_matches_sdk_best_rational_for_decimal_input() {
        assert_eq!(
            stellar_price(parse_offer_value("0.325", "price").unwrap()).unwrap(),
            Price { n: 13, d: 40 }
        );
        assert_eq!(
            stellar_price(parse_offer_value("0.0000001", "price").unwrap()).unwrap(),
            Price {
                n: 1,
                d: 10_000_000
            }
        );
        assert!(stellar_price(parse_offer_value("2147483648", "price").unwrap()).is_err());
    }

    #[test]
    fn buy_operation_keeps_pair_price_direction() {
        let base = OfferAsset::parse(&format!("XRP:{ISSUER}")).unwrap();
        let counter = OfferAsset::Native;
        let price = Price { n: 13, d: 40 };
        let operation = offer_operation(
            OfferSide::Buy,
            &base,
            &counter,
            parse_offer_value("100", "amount").unwrap(),
            price.clone(),
            0,
        )
        .unwrap();
        let OperationBody::ManageBuyOffer(operation) = operation else {
            panic!("expected manage buy offer");
        };
        assert_eq!(operation.selling, counter.to_xdr().unwrap());
        assert_eq!(operation.buying, base.to_xdr().unwrap());
        assert_eq!(operation.buy_amount, 1_000_000_000);
        assert_eq!(operation.price, price);
    }

    #[test]
    fn update_side_is_inferred_from_current_offer_projection() {
        let base = OfferAsset::parse(&format!("XRP:{ISSUER}")).unwrap();
        let counter = OfferAsset::Native;
        assert_eq!(
            infer_side(&counter, &base, &base, &counter).unwrap(),
            OfferSide::Buy
        );
        assert_eq!(
            infer_side(&base, &counter, &base, &counter).unwrap(),
            OfferSide::Sell
        );
    }

    #[test]
    fn review_total_keeps_full_decimal_precision() {
        assert_eq!(
            format_scaled_product(
                parse_offer_value("3.5", "amount").unwrap(),
                parse_offer_value("0.325", "price").unwrap()
            ),
            "1.1375"
        );
    }
}
