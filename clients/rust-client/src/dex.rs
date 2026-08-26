use std::str::FromStr;

use serde::Serialize;
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenOffer {
    pub offer_id: i64,
    pub seller: String,
    pub selling: String,
    pub buying: String,
    pub amount: String,
    pub price: String,
    pub price_n: i32,
    pub price_d: i32,
    pub last_modified_ledger: Option<i64>,
    pub last_modified_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderBookLevel {
    pub amount: String,
    pub price: String,
    pub price_n: i32,
    pub price_d: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrderBookSnapshot {
    pub base: String,
    pub counter: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DexTradeSide {
    Buy,
    Sell,
}

impl DexTradeSide {
    pub fn label(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairTrade {
    pub trade_id: String,
    pub ledger_close_time: Option<String>,
    pub base_amount: String,
    pub counter_amount: String,
    pub price: String,
    pub price_n: Option<i32>,
    pub price_d: Option<i32>,
    pub base_side: DexTradeSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairTradesSnapshot {
    pub base: String,
    pub counter: String,
    pub trades: Vec<PairTrade>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FillSegment {
    pub base_asset: String,
    pub counter_asset: String,
    pub side: DexTradeSide,
    pub base_amount: String,
    pub counter_amount: String,
    pub price: String,
    pub price_n: i32,
    pub price_d: i32,
    pub offer_id: Option<String>,
    pub trade_count: usize,
    pub first_time: Option<String>,
    pub last_time: Option<String>,
    pub first_trade_id: String,
    pub last_trade_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountFillsSnapshot {
    pub wallet: WalletRecord,
    pub fills: Vec<FillSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TradeCandle {
    pub timestamp: u64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub base_volume: String,
    pub counter_volume: Option<String>,
    pub trade_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandleSnapshot {
    pub base: String,
    pub counter: String,
    pub resolution_ms: u64,
    pub candles: Vec<TradeCandle>,
}

impl OpenOffer {
    fn from_horizon(raw: Value) -> Result<Self, String> {
        let offer_id = integer(raw.get("id"))
            .filter(|value| *value > 0)
            .ok_or_else(|| "Horizon returned invalid offer id".to_owned())?;
        let seller = text(&raw, "seller")
            .ok_or_else(|| "Horizon returned malformed offer seller".to_owned())?
            .to_owned();
        let selling = OfferAsset::from_horizon(
            raw.get("selling")
                .ok_or_else(|| "Horizon returned malformed offer selling asset".to_owned())?,
        )?
        .display();
        let buying = OfferAsset::from_horizon(
            raw.get("buying")
                .ok_or_else(|| "Horizon returned malformed offer buying asset".to_owned())?,
        )?
        .display();
        let amount = text(&raw, "amount")
            .ok_or_else(|| "Horizon returned malformed offer amount".to_owned())?
            .to_owned();
        let price = text(&raw, "price")
            .ok_or_else(|| "Horizon returned malformed offer price".to_owned())?
            .to_owned();
        let ratio = horizon_price(&raw)?;
        let last_modified_ledger = integer(raw.get("last_modified_ledger"));
        let last_modified_time = text(&raw, "last_modified_time").map(str::to_owned);
        Ok(Self {
            offer_id,
            seller,
            selling,
            buying,
            amount,
            price,
            price_n: ratio.n,
            price_d: ratio.d,
            last_modified_ledger,
            last_modified_time,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenOffersSnapshot {
    pub wallet: WalletRecord,
    pub offers: Vec<OpenOffer>,
}

impl FresnicaClient {
    pub fn order_book(
        &self,
        base_text: &str,
        counter_text: &str,
    ) -> Result<OrderBookSnapshot, String> {
        let base = OfferAsset::parse(base_text)?;
        let counter = OfferAsset::parse(counter_text)?;
        ensure_pair(&base, &counter)?;
        let raw = self
            .horizon()
            .get_order_book(&base.query("selling"), &counter.query("buying"))?;
        let bids = order_book_rows(&raw, "bids")?
            .iter()
            .map(order_book_bid)
            .collect::<Result<Vec<_>, _>>()?;
        let asks = order_book_rows(&raw, "asks")?
            .iter()
            .map(order_book_ask)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OrderBookSnapshot {
            base: base.display(),
            counter: counter.display(),
            bids,
            asks,
        })
    }

    pub fn open_offers(
        &self,
        wallet_name: Option<&str>,
        limit: usize,
    ) -> Result<OpenOffersSnapshot, String> {
        if !(1..=200).contains(&limit) {
            return Err("offers limit must be from 1 to 200".to_owned());
        }
        let wallet = self.resolve_wallet(wallet_name)?;
        let offers = self
            .horizon()
            .get_offers(&wallet.address, limit)?
            .into_iter()
            .map(OpenOffer::from_horizon)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(OpenOffersSnapshot { wallet, offers })
    }

    pub fn pair_trades(
        &self,
        base_text: &str,
        counter_text: &str,
        limit: usize,
    ) -> Result<PairTradesSnapshot, String> {
        validate_page_limit(limit, "trades")?;
        let base = OfferAsset::parse(base_text)?;
        let counter = OfferAsset::parse(counter_text)?;
        ensure_pair(&base, &counter)?;
        let trades = self
            .horizon()
            .get_trades(&base.query("base"), &counter.query("counter"), limit)?
            .into_iter()
            .map(pair_trade_from_horizon)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PairTradesSnapshot {
            base: base.display(),
            counter: counter.display(),
            trades,
        })
    }

    pub fn account_fills(
        &self,
        wallet_name: Option<&str>,
        limit: usize,
    ) -> Result<AccountFillsSnapshot, String> {
        validate_page_limit(limit, "fills")?;
        let wallet = self.resolve_wallet(wallet_name)?;
        let records = self.horizon().get_account_trades(&wallet.address, limit)?;
        let fills = compress_account_trades(&records, &wallet.address)?;
        Ok(AccountFillsSnapshot { wallet, fills })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn candles(
        &self,
        base_text: &str,
        counter_text: &str,
        resolution_ms: u64,
        start_time: Option<u64>,
        end_time: Option<u64>,
        offset: Option<u64>,
        limit: usize,
    ) -> Result<CandleSnapshot, String> {
        validate_page_limit(limit, "candles")?;
        validate_candle_resolution(resolution_ms)?;
        if let (Some(start), Some(end)) = (start_time, end_time) {
            if start > end {
                return Err("candle start time must not be after end time".to_owned());
            }
        }
        if let Some(value) = offset {
            validate_candle_offset(value, resolution_ms)?;
        }
        let base = OfferAsset::parse(base_text)?;
        let counter = OfferAsset::parse(counter_text)?;
        ensure_pair(&base, &counter)?;
        let candles = self
            .horizon()
            .get_trade_aggregations(
                &base.query("base"),
                &counter.query("counter"),
                resolution_ms,
                start_time,
                end_time,
                offset,
                limit,
            )?
            .into_iter()
            .map(candle_from_horizon)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CandleSnapshot {
            base: base.display(),
            counter: counter.display(),
            resolution_ms,
            candles,
        })
    }

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

    fn query(&self, prefix: &str) -> String {
        match self {
            Self::Native => format!("{prefix}_asset_type=native"),
            Self::Credit { code, issuer } => {
                let asset_type = if code.len() <= 4 {
                    "credit_alphanum4"
                } else {
                    "credit_alphanum12"
                };
                format!(
                    "{prefix}_asset_type={asset_type}&{prefix}_asset_code={code}&{prefix}_asset_issuer={issuer}"
                )
            }
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

fn order_book_rows<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("Horizon returned malformed order book {key}"))
}

fn order_book_bid(value: &Value) -> Result<OrderBookLevel, String> {
    let amount = parse_stroops(text(value, "amount").unwrap_or(""), true)
        .map_err(|_| "Horizon returned invalid order book amount".to_owned())?;
    let price = horizon_price(value)?;
    let numerator = i128::from(amount)
        .checked_mul(i128::from(price.d))
        .ok_or_else(|| "order book bid amount overflow".to_owned())?;
    let base_stroops = round_ratio(numerator, i128::from(price.n))?;
    Ok(OrderBookLevel {
        amount: format_scaled_7(base_stroops),
        price: format_price_ratio(price.n, price.d)?,
        price_n: price.n,
        price_d: price.d,
    })
}

fn order_book_ask(value: &Value) -> Result<OrderBookLevel, String> {
    let amount = parse_stroops(text(value, "amount").unwrap_or(""), true)
        .map_err(|_| "Horizon returned invalid order book amount".to_owned())?;
    let price = horizon_price(value)?;
    Ok(OrderBookLevel {
        amount: format_scaled_7(i128::from(amount)),
        price: format_price_ratio(price.n, price.d)?,
        price_n: price.n,
        price_d: price.d,
    })
}

fn format_price_ratio(n: i32, d: i32) -> Result<String, String> {
    let numerator = i128::from(n)
        .checked_mul(i128::from(STROOPS_PER_XLM))
        .ok_or_else(|| "price display overflow".to_owned())?;
    if numerator
        .checked_mul(2)
        .ok_or_else(|| "price display overflow".to_owned())?
        < i128::from(d)
    {
        return Ok("<0.0000001".to_owned());
    }
    Ok(format_scaled_7(round_ratio(numerator, i128::from(d))?))
}

fn round_ratio(numerator: i128, denominator: i128) -> Result<i128, String> {
    if numerator < 0 || denominator <= 0 {
        return Err("invalid non-negative decimal ratio".to_owned());
    }
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    Ok(if remainder.saturating_mul(2) >= denominator {
        quotient + 1
    } else {
        quotient
    })
}

fn format_scaled_7(value: i128) -> String {
    let scale = i128::from(STROOPS_PER_XLM);
    let whole = value / scale;
    let fraction = value % scale;
    format!("{whole}.{fraction:07}")
}

fn validate_page_limit(limit: usize, label: &str) -> Result<(), String> {
    if !(1..=200).contains(&limit) {
        Err(format!("{label} limit must be from 1 to 200"))
    } else {
        Ok(())
    }
}

fn pair_trade_from_horizon(raw: Value) -> Result<PairTrade, String> {
    let trade_id = value_text(raw.get("id"))
        .or_else(|| value_text(raw.get("paging_token")))
        .ok_or_else(|| "Horizon returned malformed trade id".to_owned())?;
    let base_amount = text(&raw, "base_amount")
        .ok_or_else(|| "Horizon returned malformed trade base amount".to_owned())?
        .to_owned();
    let counter_amount = text(&raw, "counter_amount")
        .ok_or_else(|| "Horizon returned malformed trade counter amount".to_owned())?
        .to_owned();
    let (price, price_n, price_d) = trade_price_parts(&raw)?;
    Ok(PairTrade {
        trade_id,
        ledger_close_time: text(&raw, "ledger_close_time").map(str::to_owned),
        base_amount,
        counter_amount,
        price,
        price_n,
        price_d,
        base_side: if raw
            .get("base_is_seller")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            DexTradeSide::Sell
        } else {
            DexTradeSide::Buy
        },
    })
}

fn trade_price_parts(raw: &Value) -> Result<(String, Option<i32>, Option<i32>), String> {
    if let Some(price) = raw.get("price").and_then(Value::as_object) {
        let n = integer(price.get("n")).and_then(|value| i32::try_from(value).ok());
        let d = integer(price.get("d")).and_then(|value| i32::try_from(value).ok());
        if let (Some(n), Some(d)) = (n, d) {
            if n > 0 && d > 0 {
                return Ok((format_price_ratio(n, d)?, Some(n), Some(d)));
            }
        }
    }
    let base = parse_trade_amount(raw, "base_amount")?;
    let counter = parse_trade_amount(raw, "counter_amount")?;
    if base <= 0 {
        return Err("Horizon returned invalid zero trade base amount".to_owned());
    }
    let numerator = i128::from(counter)
        .checked_mul(i128::from(STROOPS_PER_XLM))
        .ok_or_else(|| "trade price overflow".to_owned())?;
    let denominator = i128::from(base);
    let price = if numerator
        .checked_mul(2)
        .ok_or_else(|| "trade price overflow".to_owned())?
        < denominator
    {
        "<0.0000001".to_owned()
    } else {
        format_scaled_7(round_ratio(numerator, denominator)?)
    };
    Ok((price, None, None))
}

fn compress_account_trades(records: &[Value], address: &str) -> Result<Vec<FillSegment>, String> {
    #[derive(Debug)]
    struct WorkingFill {
        base_asset: String,
        counter_asset: String,
        side: DexTradeSide,
        base_amount: i64,
        counter_amount: i64,
        price_n: i32,
        price_d: i32,
        offer_id: Option<String>,
        trade_count: usize,
        first_time: Option<String>,
        last_time: Option<String>,
        first_trade_id: String,
        last_trade_id: String,
    }

    impl WorkingFill {
        fn key(&self) -> Option<(&str, &str, DexTradeSide, i32, i32, &str)> {
            self.offer_id.as_deref().map(|offer_id| {
                (
                    self.base_asset.as_str(),
                    self.counter_asset.as_str(),
                    self.side,
                    self.price_n,
                    self.price_d,
                    offer_id,
                )
            })
        }

        fn finish(self) -> Result<FillSegment, String> {
            Ok(FillSegment {
                base_asset: self.base_asset,
                counter_asset: self.counter_asset,
                side: self.side,
                base_amount: format_scaled_7(i128::from(self.base_amount)),
                counter_amount: format_scaled_7(i128::from(self.counter_amount)),
                price: format_price_ratio(self.price_n, self.price_d)?,
                price_n: self.price_n,
                price_d: self.price_d,
                offer_id: self.offer_id,
                trade_count: self.trade_count,
                first_time: self.first_time,
                last_time: self.last_time,
                first_trade_id: self.first_trade_id,
                last_trade_id: self.last_trade_id,
            })
        }
    }

    fn segment(raw: &Value, address: &str) -> Result<WorkingFill, String> {
        let price = raw
            .get("price")
            .and_then(Value::as_object)
            .ok_or_else(|| "Invalid Horizon trade record: missing price".to_owned())?;
        let price_n = integer(price.get("n"))
            .and_then(|value| i32::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| "Invalid Horizon trade record: bad price numerator".to_owned())?;
        let price_d = integer(price.get("d"))
            .and_then(|value| i32::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or_else(|| "Invalid Horizon trade record: bad price denominator".to_owned())?;
        let base_account = text(raw, "base_account");
        let counter_account = text(raw, "counter_account");
        let side = if base_account == Some(address) {
            DexTradeSide::Sell
        } else if counter_account == Some(address) {
            DexTradeSide::Buy
        } else if raw
            .get("base_is_seller")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            DexTradeSide::Sell
        } else {
            DexTradeSide::Buy
        };
        let offer_id = if base_account == Some(address) {
            value_text(raw.get("base_offer_id"))
        } else if counter_account == Some(address) {
            value_text(raw.get("counter_offer_id"))
        } else {
            None
        };
        let trade_id = value_text(raw.get("id"))
            .or_else(|| value_text(raw.get("paging_token")))
            .unwrap_or_default();
        let time = text(raw, "ledger_close_time").map(str::to_owned);
        Ok(WorkingFill {
            base_asset: trade_asset(raw, "base")?,
            counter_asset: trade_asset(raw, "counter")?,
            side,
            base_amount: parse_trade_amount(raw, "base_amount")?,
            counter_amount: parse_trade_amount(raw, "counter_amount")?,
            price_n,
            price_d,
            offer_id,
            trade_count: 1,
            first_time: time.clone(),
            last_time: time,
            first_trade_id: trade_id.clone(),
            last_trade_id: trade_id,
        })
    }

    let mut result: Vec<WorkingFill> = Vec::new();
    for raw in records {
        let current = segment(raw, address)?;
        let can_merge = result
            .last()
            .and_then(WorkingFill::key)
            .zip(current.key())
            .is_some_and(|(left, right)| left == right);
        if can_merge {
            let previous = result.last_mut().expect("segment exists");
            previous.base_amount = previous
                .base_amount
                .checked_add(current.base_amount)
                .ok_or_else(|| "fill base amount overflow".to_owned())?;
            previous.counter_amount = previous
                .counter_amount
                .checked_add(current.counter_amount)
                .ok_or_else(|| "fill counter amount overflow".to_owned())?;
            previous.trade_count += 1;
            previous.last_time = current.last_time;
            previous.last_trade_id = current.last_trade_id;
        } else {
            result.push(current);
        }
    }
    result.into_iter().map(WorkingFill::finish).collect()
}

fn trade_asset(raw: &Value, prefix: &str) -> Result<String, String> {
    if text(raw, &format!("{prefix}_asset_type")) == Some("native") {
        return Ok("XLM".to_owned());
    }
    let code = text(raw, &format!("{prefix}_asset_code"))
        .ok_or_else(|| "Invalid Horizon trade asset code".to_owned())?;
    let issuer = text(raw, &format!("{prefix}_asset_issuer"))
        .ok_or_else(|| "Invalid Horizon trade asset issuer".to_owned())?;
    OfferAsset::parse(&format!("{code}:{issuer}")).map(|asset| asset.display())
}

fn parse_trade_amount(raw: &Value, key: &str) -> Result<i64, String> {
    let value = text(raw, key).ok_or_else(|| format!("Invalid Horizon trade record: {key}"))?;
    parse_stroops(value, false).map_err(|_| format!("Invalid Horizon trade record amount: {value}"))
}

fn validate_candle_resolution(value: u64) -> Result<(), String> {
    match value {
        60_000 | 300_000 | 900_000 | 3_600_000 | 86_400_000 | 604_800_000 => Ok(()),
        _ => Err(format!("Unsupported trade aggregation resolution: {value}")),
    }
}

fn validate_candle_offset(offset: u64, resolution: u64) -> Result<(), String> {
    const HOUR_MS: u64 = 3_600_000;
    if offset > resolution || offset >= 24 * HOUR_MS || offset % HOUR_MS != 0 {
        Err(format!(
            "Invalid candle offset {offset} for resolution {resolution}"
        ))
    } else {
        Ok(())
    }
}

fn candle_from_horizon(raw: Value) -> Result<TradeCandle, String> {
    let timestamp = unsigned_integer(raw.get("timestamp"))
        .ok_or_else(|| "Horizon returned invalid candle timestamp".to_owned())?;
    let trade_count = unsigned_integer(raw.get("trade_count"))
        .ok_or_else(|| "Horizon returned invalid candle trade count".to_owned())?;
    Ok(TradeCandle {
        timestamp,
        open: required_text(&raw, "open", "candle open")?,
        high: required_text(&raw, "high", "candle high")?,
        low: required_text(&raw, "low", "candle low")?,
        close: required_text(&raw, "close", "candle close")?,
        base_volume: required_text(&raw, "base_volume", "candle base volume")?,
        counter_volume: text(&raw, "counter_volume").map(str::to_owned),
        trade_count,
    })
}

fn required_text(raw: &Value, key: &str, label: &str) -> Result<String, String> {
    text(raw, key)
        .map(str::to_owned)
        .ok_or_else(|| format!("Horizon returned malformed {label}"))
}

fn unsigned_integer(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(value) => value.as_u64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
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

    #[test]
    fn open_offer_preserves_full_asset_identity_and_exact_price_ratio() {
        let raw = serde_json::json!({
            "id": "42",
            "seller": ISSUER,
            "selling": {"asset_type": "native"},
            "buying": {
                "asset_type": "credit_alphanum4",
                "asset_code": "USD",
                "asset_issuer": ISSUER
            },
            "amount": "3.5000000",
            "price": "0.3250000",
            "price_r": {"n": 13, "d": 40},
            "last_modified_ledger": 123,
            "last_modified_time": "2026-08-26T00:00:00Z"
        });
        let offer = OpenOffer::from_horizon(raw).unwrap();
        assert_eq!(offer.offer_id, 42);
        assert_eq!(offer.selling, "XLM");
        assert_eq!(offer.buying, format!("USD:{ISSUER}"));
        assert_eq!((offer.price_n, offer.price_d), (13, 40));
        assert_eq!(offer.last_modified_ledger, Some(123));
        let encoded = serde_json::to_value(&offer).unwrap();
        assert_eq!(encoded["offer_id"], 42);
        assert_eq!(encoded["selling"], "XLM");
        assert_eq!(encoded["buying"], format!("USD:{ISSUER}"));
        assert!(encoded.get("_links").is_none());
    }

    #[test]
    fn order_book_bid_normalizes_horizon_counter_amount_to_base() {
        let row = serde_json::json!({
            "amount": "14.2000000",
            "price": "2.0000000",
            "price_r": {"n": 2, "d": 1}
        });
        let level = order_book_bid(&row).unwrap();
        assert_eq!(level.amount, "7.1000000");
        assert_eq!(level.price, "2.0000000");
        assert_eq!((level.price_n, level.price_d), (2, 1));
    }

    #[test]
    fn order_book_ask_keeps_base_amount_and_exact_price_ratio() {
        let row = serde_json::json!({
            "amount": "7.2000000",
            "price": "2.1000000",
            "price_r": {"n": 21, "d": 10}
        });
        let level = order_book_ask(&row).unwrap();
        assert_eq!(level.amount, "7.2000000");
        assert_eq!(level.price, "2.1000000");
        assert_eq!((level.price_n, level.price_d), (21, 10));
    }

    #[test]
    fn order_book_tiny_nonzero_price_does_not_render_as_zero() {
        assert_eq!(format_price_ratio(1, 30_000_000).unwrap(), "<0.0000001");
    }

    #[test]
    fn order_book_query_preserves_full_asset_identity() {
        let asset = OfferAsset::parse(&format!("USD:{ISSUER}")).unwrap();
        assert_eq!(
            asset.query("buying"),
            format!(
                "buying_asset_type=credit_alphanum4&buying_asset_code=USD&buying_asset_issuer={ISSUER}"
            )
        );
    }

    #[test]
    fn pair_trade_preserves_exact_price_and_base_side() {
        let raw = serde_json::json!({
            "id": "17",
            "ledger_close_time": "2026-08-26T00:00:00Z",
            "base_amount": "7.1000000",
            "counter_amount": "14.2000000",
            "price": {"n": 2, "d": 1},
            "base_is_seller": true
        });
        let trade = pair_trade_from_horizon(raw).unwrap();
        assert_eq!(trade.trade_id, "17");
        assert_eq!(trade.price, "2.0000000");
        assert_eq!((trade.price_n, trade.price_d), (Some(2), Some(1)));
        assert_eq!(trade.base_side, DexTradeSide::Sell);
    }

    #[test]
    fn pair_trade_falls_back_to_amount_ratio_without_false_zero() {
        let raw = serde_json::json!({
            "id": "18",
            "base_amount": "3.0000000",
            "counter_amount": "0.0000001",
            "base_is_seller": false
        });
        let trade = pair_trade_from_horizon(raw).unwrap();
        assert_eq!(trade.price, "<0.0000001");
        assert_eq!((trade.price_n, trade.price_d), (None, None));
        assert_eq!(trade.base_side, DexTradeSide::Buy);
    }

    #[test]
    fn account_fills_merge_only_consecutive_same_user_offer() {
        let first = serde_json::json!({
            "id":"1",
            "paging_token":"1",
            "ledger_close_time":"2026-01-01T00:00:00Z",
            "base_asset_type":"native",
            "counter_asset_type":"credit_alphanum4",
            "counter_asset_code":"USD",
            "counter_asset_issuer":ISSUER,
            "base_amount":"1.0000000",
            "counter_amount":"2.0000000",
            "price":{"n":2,"d":1},
            "base_account":ISSUER,
            "counter_account":"GOTHER",
            "base_offer_id":"7",
            "counter_offer_id":"8",
            "base_is_seller":true
        });
        let second = serde_json::json!({
            "id":"2",
            "paging_token":"2",
            "ledger_close_time":"2026-01-01T00:00:01Z",
            "base_asset_type":"native",
            "counter_asset_type":"credit_alphanum4",
            "counter_asset_code":"USD",
            "counter_asset_issuer":ISSUER,
            "base_amount":"3.0000000",
            "counter_amount":"6.0000000",
            "price":{"n":2,"d":1},
            "base_account":ISSUER,
            "counter_account":"GOTHER",
            "base_offer_id":"7",
            "counter_offer_id":"9",
            "base_is_seller":true
        });
        let fills = compress_account_trades(&[first, second], ISSUER).unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].base_amount, "4.0000000");
        assert_eq!(fills[0].counter_amount, "8.0000000");
        assert_eq!(fills[0].price, "2.0000000");
        assert_eq!(fills[0].trade_count, 2);
        assert_eq!(fills[0].offer_id.as_deref(), Some("7"));
        assert_eq!(fills[0].side, DexTradeSide::Sell);
        assert_eq!(fills[0].counter_asset, format!("USD:{ISSUER}"));
    }

    #[test]
    fn missing_offer_ids_do_not_merge_fills() {
        let trade = serde_json::json!({
            "id":"1",
            "ledger_close_time":"2026-01-01T00:00:00Z",
            "base_asset_type":"native",
            "counter_asset_type":"native",
            "base_amount":"1.0000000",
            "counter_amount":"1.0000000",
            "price":{"n":1,"d":1},
            "base_is_seller":true
        });
        let fills = compress_account_trades(&[trade.clone(), trade], ISSUER).unwrap();
        assert_eq!(fills.len(), 2);
    }

    #[test]
    fn candle_validation_matches_horizon_rules() {
        assert!(validate_candle_resolution(3_600_000).is_ok());
        assert!(validate_candle_resolution(7_200_000).is_err());
        assert!(validate_candle_offset(0, 3_600_000).is_ok());
        assert!(validate_candle_offset(3_600_000, 3_600_000).is_ok());
        assert!(validate_candle_offset(1_000, 86_400_000).is_err());
        assert!(validate_candle_offset(86_400_000, 604_800_000).is_err());
    }

    #[test]
    fn candle_parses_numeric_timestamp_and_transport_neutral_fields() {
        let candle = candle_from_horizon(serde_json::json!({
            "timestamp": 1582156800000_u64,
            "trade_count": "3",
            "open":"1.0000000",
            "high":"2.0000000",
            "low":"0.5000000",
            "close":"1.5000000",
            "base_volume":"10.0000000",
            "counter_volume":"12.0000000"
        }))
        .unwrap();
        assert_eq!(candle.timestamp, 1_582_156_800_000);
        assert_eq!(candle.trade_count, 3);
        assert_eq!(candle.counter_volume.as_deref(), Some("12.0000000"));
    }
}
