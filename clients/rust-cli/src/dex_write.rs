use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, ChangeTrustAsset,
    ChangeTrustOp, ManageBuyOfferOp, ManageSellOfferOp, OperationBody, Price,
};

use crate::transaction_flow::{
    account_sequence, balance_stroops, build_operation_envelope, confirm_submission,
    format_stroops, minimum_balance_stroops, network_client, parse_stroops, resolve_signing_wallet,
    sign_and_submit, STROOPS_PER_XLM,
};
use fresnica_client::{
    HorizonClient, WalletRecord, WalletStorage, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL,
};

const FRESNICA_TRUSTLINE_LIMIT: &str = "708269837873.6765";
const INT32_MAX: i64 = i32::MAX as i64;

pub fn command_dex_write(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = WriteRequest::parse(arguments)?;
    match request {
        WriteRequest::Create {
            side,
            base,
            counter,
            amount,
            price,
            wallet,
            allow_trustline,
            yes,
        } => command_create(
            storage,
            network,
            side,
            &base,
            &counter,
            &amount,
            &price,
            wallet.as_deref(),
            allow_trustline,
            yes,
        ),
        WriteRequest::Update {
            offer_id,
            base,
            counter,
            amount,
            price,
            wallet,
            yes,
        } => command_update(
            storage,
            network,
            offer_id,
            &base,
            &counter,
            &amount,
            &price,
            wallet.as_deref(),
            yes,
        ),
        WriteRequest::Cancel {
            offer_id,
            wallet,
            yes,
        } => command_cancel(storage, network, offer_id, wallet.as_deref(), yes),
    }
}

fn command_create(
    storage: &WalletStorage,
    network: &str,
    side: Side,
    base_text: &str,
    counter_text: &str,
    amount_text: &str,
    price_text: &str,
    wallet: Option<&str>,
    allow_trustline: bool,
    yes: bool,
) -> Result<(), String> {
    let record = resolve_signing_wallet(storage, network, wallet)?;
    let base = OfferAsset::parse(base_text)?;
    let counter = OfferAsset::parse(counter_text)?;
    ensure_pair(&base, &counter)?;
    let amount = parse_offer_value(amount_text, "amount")?;
    let price_stroops = parse_offer_value(price_text, "price")?;
    let price = stellar_price(price_stroops)?;

    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let ledger = horizon.get_ledger_parameters()?;
    let (selling, buying) = side.assets(&base, &counter);
    let adds_trustline = !account_can_hold(&account, buying, &record.address);
    if adds_trustline && !allow_trustline {
        return Err(format!(
            "Receiving trustline is missing for {}. Add it first or rerun with --allow-trustline.",
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
        &record.address,
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
            limit: parse_stroops(FRESNICA_TRUSTLINE_LIMIT, true)
                .map_err(|_| "invalid Fresnica trustline policy limit".to_owned())?,
        }));
    }
    operations.push(offer_operation(side, &base, &counter, amount, price, 0)?);
    let mut envelope = build_operation_envelope(
        &record.address,
        operations,
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        None,
    )?;

    render_offer_review(
        &record,
        "create",
        side,
        &base,
        &counter,
        amount,
        price_stroops,
        total_fee,
        None,
        if adds_trustline { Some(buying) } else { None },
    );
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }
    sign_and_submit(&record, network, &mut envelope, &horizon)
}

fn command_update(
    storage: &WalletStorage,
    network: &str,
    offer_id: i64,
    base_text: &str,
    counter_text: &str,
    amount_text: &str,
    price_text: &str,
    wallet: Option<&str>,
    yes: bool,
) -> Result<(), String> {
    let record = resolve_signing_wallet(storage, network, wallet)?;
    let base = OfferAsset::parse(base_text)?;
    let counter = OfferAsset::parse(counter_text)?;
    ensure_pair(&base, &counter)?;
    let amount = parse_offer_value(amount_text, "amount")?;
    let price_stroops = parse_offer_value(price_text, "price")?;
    let price = stellar_price(price_stroops)?;

    let raw_offer = fetch_offer(network, offer_id)?;
    ensure_offer_owner(&raw_offer, &record)?;
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

    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let ledger = horizon.get_ledger_parameters()?;
    let body = offer_operation(side, &base, &counter, amount, price, offer_id)?;
    let mut envelope = build_operation_envelope(
        &record.address,
        vec![body],
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        None,
    )?;
    let total_fee = i64::from(ledger.base_fee_in_stroops);

    render_offer_review(
        &record,
        "update",
        side,
        &base,
        &counter,
        amount,
        price_stroops,
        total_fee,
        Some(offer_id),
        None,
    );
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }
    sign_and_submit(&record, network, &mut envelope, &horizon)
}

fn command_cancel(
    storage: &WalletStorage,
    network: &str,
    offer_id: i64,
    wallet: Option<&str>,
    yes: bool,
) -> Result<(), String> {
    let record = resolve_signing_wallet(storage, network, wallet)?;
    let raw_offer = fetch_offer(network, offer_id)?;
    ensure_offer_owner(&raw_offer, &record)?;
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
    let horizon = network_client(network)?;
    let account = horizon.get_account(&record.address)?;
    let ledger = horizon.get_ledger_parameters()?;
    let mut envelope = build_operation_envelope(
        &record.address,
        vec![body],
        account_sequence(&account)?,
        ledger.base_fee_in_stroops,
        None,
    )?;

    println!("Review transaction");
    println!("Operation: ManageSellOffer (cancel)");
    println!("Wallet:    {} ({})", record.name, record.address);
    println!("Offer:     #{offer_id}");
    println!("Selling:   {}", selling.display());
    println!("Buying:    {}", buying.display());
    println!(
        "Fee:       {} XLM",
        format_stroops(i64::from(ledger.base_fee_in_stroops))
    );
    println!("Network:   {}", record.network);
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }
    sign_and_submit(&record, network, &mut envelope, &horizon)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Buy,
    Sell,
}

impl Side {
    fn label(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }

    fn operation_label(self) -> &'static str {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum WriteRequest {
    Create {
        side: Side,
        base: String,
        counter: String,
        amount: String,
        price: String,
        wallet: Option<String>,
        allow_trustline: bool,
        yes: bool,
    },
    Update {
        offer_id: i64,
        base: String,
        counter: String,
        amount: String,
        price: String,
        wallet: Option<String>,
        yes: bool,
    },
    Cancel {
        offer_id: i64,
        wallet: Option<String>,
        yes: bool,
    },
}

impl WriteRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "buy" | "sell" => {
                if arguments.len() < 5 {
                    return Err(usage().to_owned());
                }
                let side = if command == "buy" {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let (wallet, allow_trustline, yes) = parse_options(&arguments[5..], true)?;
                Ok(Self::Create {
                    side,
                    base: arguments[1].clone(),
                    counter: arguments[2].clone(),
                    amount: arguments[3].clone(),
                    price: arguments[4].clone(),
                    wallet,
                    allow_trustline,
                    yes,
                })
            }
            "update" => {
                if arguments.len() < 6 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes) = parse_options(&arguments[6..], false)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self::Update {
                    offer_id,
                    base: arguments[2].clone(),
                    counter: arguments[3].clone(),
                    amount: arguments[4].clone(),
                    price: arguments[5].clone(),
                    wallet,
                    yes,
                })
            }
            "cancel" => {
                if arguments.len() < 2 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes) = parse_options(&arguments[2..], false)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self::Cancel {
                    offer_id,
                    wallet,
                    yes,
                })
            }
            _ => Err(usage().to_owned()),
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_trustline_option: bool,
) -> Result<(Option<String>, bool, bool), String> {
    let mut wallet = None;
    let mut allow_trustline = false;
    let mut yes = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .clone(),
                );
                index += 1;
            }
            "--allow-trustline" if allow_trustline_option => {
                allow_trustline = true;
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, allow_trustline, yes))
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

fn offer_operation(
    side: Side,
    base: &OfferAsset,
    counter: &OfferAsset,
    amount: i64,
    price: Price,
    offer_id: i64,
) -> Result<OperationBody, String> {
    match side {
        Side::Buy => Ok(OperationBody::ManageBuyOffer(ManageBuyOfferOp {
            selling: counter.to_xdr()?,
            buying: base.to_xdr()?,
            buy_amount: amount,
            price,
            offer_id,
        })),
        Side::Sell => Ok(OperationBody::ManageSellOffer(ManageSellOfferOp {
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
) -> Result<Side, String> {
    if selling == base && buying == counter {
        Ok(Side::Sell)
    } else if selling == counter && buying == base {
        Ok(Side::Buy)
    } else {
        Err("Offer update must keep the current market pair and BUY/SELL side".to_owned())
    }
}

fn preflight_create(
    account: &Value,
    account_id: &str,
    side: Side,
    selling: &OfferAsset,
    amount: i64,
    price_stroops: i64,
    base_reserve: i64,
    fee: i64,
    extra_subentries: i64,
) -> Result<(), String> {
    let required_selling = match side {
        Side::Sell => amount,
        Side::Buy => ceil_scaled_product(amount, price_stroops)?,
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

fn fetch_offer(network: &str, offer_id: i64) -> Result<Value, String> {
    let base = match network {
        "mainnet" => MAINNET_HORIZON_URL,
        "testnet" => TESTNET_HORIZON_URL,
        other => return Err(format!("unknown network: {other}")),
    };
    let url = format!("{base}/offers/{offer_id}");
    let mut response = match ureq::get(&url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Err(format!("Offer not found: {offer_id}")),
        Err(ureq::Error::StatusCode(code)) => {
            return Err(format!("Horizon returned HTTP {code} for {url}"))
        }
        Err(error) => return Err(format!("Unable to contact Horizon at {url}: {error}")),
    };
    response
        .body_mut()
        .read_json::<Value>()
        .map_err(|error| format!("Horizon returned invalid offer JSON: {error}"))
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

fn parse_offer_id(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "offer id must be a positive integer".to_owned())
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

fn render_offer_review(
    record: &WalletRecord,
    action: &str,
    side: Side,
    base: &OfferAsset,
    counter: &OfferAsset,
    amount: i64,
    price_stroops: i64,
    fee: i64,
    offer_id: Option<i64>,
    trustline_asset: Option<&OfferAsset>,
) {
    println!("Review transaction");
    println!("Operation: {} ({action})", side.operation_label());
    println!("Wallet:    {} ({})", record.name, record.address);
    if let Some(offer_id) = offer_id {
        println!("Offer:     #{offer_id}");
    }
    println!("Side:      {}", side.label());
    println!("Pair:      {} / {}", base.display(), counter.display());
    println!("Amount:    {} {}", format_stroops(amount), base.display());
    println!(
        "Price:     {} {}/{}",
        format_stroops(price_stroops),
        counter.display(),
        base.display()
    );
    println!(
        "Total:     {} {}",
        format_scaled_product(amount, price_stroops),
        counter.display()
    );
    if let Some(asset) = trustline_asset {
        println!("Trustline: + {} (explicitly approved)", asset.display());
    }
    println!("Fee:       {} XLM", format_stroops(fee));
    println!("Network:   {}", record.network);
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

fn usage() -> &'static str {
    "usage:\n  fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]\n  fresnica dex cancel OFFER_ID [--wallet NAME] [-y]"
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
            Side::Buy,
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
            Side::Buy
        );
        assert_eq!(
            infer_side(&base, &counter, &base, &counter).unwrap(),
            Side::Sell
        );
    }

    #[test]
    fn parser_matches_python_cli_write_shape() {
        let args = [
            "buy",
            "XLM",
            &format!("USD:{ISSUER}"),
            "10",
            "2.5",
            "--allow-trustline",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = WriteRequest::parse(&args).unwrap();
        assert!(matches!(
            request,
            WriteRequest::Create {
                side: Side::Buy,
                allow_trustline: true,
                yes: true,
                wallet: Some(ref wallet),
                ..
            } if wallet == "alpha"
        ));
    }
}
