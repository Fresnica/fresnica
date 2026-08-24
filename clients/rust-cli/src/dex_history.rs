use serde_json::{json, Value};

use crate::storage::{WalletRecord, WalletStorage};
use crate::transaction_flow::{format_stroops, parse_stroops};

use super::{
    format_price_ratio, get_json, horizon_url, resolve_network_wallet, ClassicAsset,
    MAX_PAGE_LIMIT,
};

const HOUR_MS: u64 = 3_600_000;

pub fn command_dex_history(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "trades" => command_trades(network, &arguments[1..]),
        "fills" => command_fills(storage, network, &arguments[1..]),
        "candles" => command_candles(network, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn command_trades(network: &str, arguments: &[String]) -> Result<(), String> {
    let request = PairHistoryRequest::parse(arguments, 20, "trades")?;
    let base = ClassicAsset::parse(&request.base)?;
    let counter = ClassicAsset::parse(&request.counter)?;
    ensure_pair(&base, &counter)?;
    let trades = fetch_pair_trades(network, &base, &counter, request.limit)?;

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&trades)
                .map_err(|error| format!("unable to encode trades: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Trades · {}/{} [{}]",
        base.display(),
        counter.display(),
        network
    );
    println!(
        "{:<24} {:>16} {:>16} {:>14} {:<9}",
        "Time", "Base", "Counter", "Price", "Base side"
    );
    for trade in trades {
        println!(
            "{:<24} {:>16} {:>16} {:>14} {:<9}",
            text(&trade, "ledger_close_time").unwrap_or(""),
            text(&trade, "base_amount").unwrap_or("?"),
            text(&trade, "counter_amount").unwrap_or("?"),
            trade_price(&trade),
            if trade
                .get("base_is_seller")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "sell"
            } else {
                "buy"
            },
        );
    }
    Ok(())
}

fn command_fills(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let request = FillRequest::parse(arguments)?;
    let record = resolve_network_wallet(storage, network, request.wallet.as_deref())?;
    let trades = fetch_account_trades(network, &record.address, request.limit)?;
    let segments = compress_account_trades(&trades, &record.address)?;

    if request.json {
        let values: Vec<Value> = segments.iter().map(FillSegment::to_json).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&values)
                .map_err(|error| format!("unable to encode fills: {error}"))?
        );
        return Ok(());
    }

    println!("Offer fills · {} [{}]", record.name, record.network);
    println!(
        "{:<24} {:<5} {:<25} {:>14} {:>14} {:>14} {:>5} {:<12}",
        "Time", "Side", "Pair", "Base", "Counter", "Price", "Fills", "Offer"
    );
    for segment in segments {
        println!(
            "{:<24} {:<5} {:<25} {:>14} {:>14} {:>14} {:>5} {:<12}",
            segment.last_time.as_deref().or(segment.first_time.as_deref()).unwrap_or(""),
            segment.side.to_ascii_uppercase(),
            format!("{}/{}", segment.base_asset, segment.counter_asset),
            format_stroops(segment.base_amount),
            format_stroops(segment.counter_amount),
            format_price_ratio(segment.price_n, segment.price_d)?,
            segment.trade_count,
            segment.offer_id.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

fn command_candles(network: &str, arguments: &[String]) -> Result<(), String> {
    let request = CandleRequest::parse(arguments)?;
    let base = ClassicAsset::parse(&request.base)?;
    let counter = ClassicAsset::parse(&request.counter)?;
    ensure_pair(&base, &counter)?;
    let candles = fetch_candles(network, &base, &counter, &request)?;

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&candles)
                .map_err(|error| format!("unable to encode candles: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Candles · {}/{} · {} [{}]",
        base.display(),
        counter.display(),
        request.resolution,
        network
    );
    println!(
        "{:<16} {:>14} {:>14} {:>14} {:>14} {:>16} {:>8}",
        "Time(ms)", "Open", "High", "Low", "Close", "Base volume", "Trades"
    );
    for item in candles.iter().rev() {
        println!(
            "{:<16} {:>14} {:>14} {:>14} {:>14} {:>16} {:>8}",
            text(item, "timestamp").unwrap_or("?"),
            text(item, "open").unwrap_or("?"),
            text(item, "high").unwrap_or("?"),
            text(item, "low").unwrap_or("?"),
            text(item, "close").unwrap_or("?"),
            text(item, "base_volume").unwrap_or("?"),
            display_integer(item.get("trade_count")),
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairHistoryRequest {
    base: String,
    counter: String,
    limit: usize,
    json: bool,
}

impl PairHistoryRequest {
    fn parse(arguments: &[String], default_limit: usize, command: &str) -> Result<Self, String> {
        let usage = format!(
            "usage: fresnica dex {command} BASE COUNTER [--limit N] [--json]"
        );
        if arguments.len() < 2 {
            return Err(usage);
        }
        let mut limit = default_limit;
        let mut json = false;
        let mut index = 2;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--limit" => {
                    index += 1;
                    limit = parse_limit(arguments.get(index), &usage)?;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(usage),
            }
        }
        Ok(Self {
            base: arguments[0].clone(),
            counter: arguments[1].clone(),
            limit,
            json,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FillRequest {
    wallet: Option<String>,
    limit: usize,
    json: bool,
}

impl FillRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let usage = "usage: fresnica dex fills [--wallet NAME] [--limit N] [--json]";
        let mut wallet = None;
        let mut limit = MAX_PAGE_LIMIT;
        let mut json = false;
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| usage.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "--limit" => {
                    index += 1;
                    limit = parse_limit(arguments.get(index), usage)?;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(usage.to_owned()),
            }
        }
        Ok(Self {
            wallet,
            limit,
            json,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandleRequest {
    base: String,
    counter: String,
    resolution: String,
    resolution_ms: u64,
    start_time: Option<u64>,
    end_time: Option<u64>,
    offset: Option<u64>,
    limit: usize,
    json: bool,
}

impl CandleRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let usage = "usage: fresnica dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]";
        if arguments.len() < 2 {
            return Err(usage.to_owned());
        }
        let mut resolution = "1h".to_owned();
        let mut resolution_ms = resolution_value(&resolution)?;
        let mut start_time = None;
        let mut end_time = None;
        let mut offset = None;
        let mut limit = 100usize;
        let mut json = false;
        let mut index = 2;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--resolution" => {
                    index += 1;
                    resolution = arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .clone();
                    resolution_ms = resolution_value(&resolution)?;
                    index += 1;
                }
                "--start" => {
                    index += 1;
                    start_time = Some(parse_u64(arguments.get(index), "--start")?);
                    index += 1;
                }
                "--end" => {
                    index += 1;
                    end_time = Some(parse_u64(arguments.get(index), "--end")?);
                    index += 1;
                }
                "--offset" => {
                    index += 1;
                    offset = Some(parse_u64(arguments.get(index), "--offset")?);
                    index += 1;
                }
                "--limit" => {
                    index += 1;
                    limit = parse_limit(arguments.get(index), usage)?;
                    index += 1;
                }
                "--json" => {
                    json = true;
                    index += 1;
                }
                _ => return Err(usage.to_owned()),
            }
        }
        if let (Some(start), Some(end)) = (start_time, end_time) {
            if start > end {
                return Err("--start must not be after --end".to_owned());
            }
        }
        if let Some(value) = offset {
            validate_offset(value, resolution_ms)?;
        }
        Ok(Self {
            base: arguments[0].clone(),
            counter: arguments[1].clone(),
            resolution,
            resolution_ms,
            start_time,
            end_time,
            offset,
            limit,
            json,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FillSegment {
    base_asset: String,
    counter_asset: String,
    side: String,
    base_amount: i64,
    counter_amount: i64,
    price_n: i64,
    price_d: i64,
    offer_id: Option<String>,
    trade_count: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    first_trade_id: String,
    last_trade_id: String,
}

impl FillSegment {
    fn to_json(&self) -> Value {
        json!({
            "base_asset": self.base_asset,
            "counter_asset": self.counter_asset,
            "side": self.side,
            "base_amount": format_stroops(self.base_amount),
            "counter_amount": format_stroops(self.counter_amount),
            "price_r": {"n": self.price_n, "d": self.price_d},
            "offer_id": self.offer_id,
            "trade_count": self.trade_count,
            "first_time": self.first_time,
            "last_time": self.last_time,
            "first_trade_id": self.first_trade_id,
            "last_trade_id": self.last_trade_id,
        })
    }

    fn key(&self) -> Option<String> {
        self.offer_id.as_ref().map(|offer_id| {
            format!(
                "{}|{}|{}|{}:{}|{}",
                self.base_asset,
                self.counter_asset,
                self.side,
                self.price_n,
                self.price_d,
                offer_id
            )
        })
    }
}

fn fetch_pair_trades(
    network: &str,
    base: &ClassicAsset,
    counter: &ClassicAsset,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let url = format!(
        "{}/trades?{}&{}&order=desc&limit={limit}",
        horizon_url(network)?,
        base.query("base"),
        counter.query("counter")
    );
    records(get_json(&url, "Unable to load Stellar trades")?, "trade")
}

fn fetch_account_trades(network: &str, address: &str, limit: usize) -> Result<Vec<Value>, String> {
    let url = format!(
        "{}/accounts/{address}/trades?order=desc&limit={limit}",
        horizon_url(network)?
    );
    records(
        get_json(&url, &format!("Stellar account not found: {address}"))?,
        "account trade",
    )
}

fn fetch_candles(
    network: &str,
    base: &ClassicAsset,
    counter: &ClassicAsset,
    request: &CandleRequest,
) -> Result<Vec<Value>, String> {
    let mut url = format!(
        "{}/trade_aggregations?{}&{}&resolution={}",
        horizon_url(network)?,
        base.query("base"),
        counter.query("counter"),
        request.resolution_ms
    );
    if let Some(value) = request.start_time {
        url.push_str(&format!("&start_time={value}"));
    }
    if let Some(value) = request.end_time {
        url.push_str(&format!("&end_time={value}"));
    }
    if let Some(value) = request.offset {
        url.push_str(&format!("&offset={value}"));
    }
    url.push_str(&format!("&order=desc&limit={}", request.limit));
    records(
        get_json(&url, "Unable to load Stellar trade aggregations")?,
        "trade aggregation",
    )
}

fn records(value: Value, label: &str) -> Result<Vec<Value>, String> {
    value
        .get("_embedded")
        .and_then(|value| value.get("records"))
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| format!("Horizon returned malformed {label} data"))
}

fn compress_account_trades(records: &[Value], address: &str) -> Result<Vec<FillSegment>, String> {
    let mut result: Vec<FillSegment> = Vec::new();
    for raw in records {
        let segment = segment_from_trade(raw, address)?;
        let can_merge = result
            .last()
            .and_then(FillSegment::key)
            .zip(segment.key())
            .is_some_and(|(left, right)| left == right);
        if can_merge {
            let previous = result.last_mut().expect("segment exists");
            previous.base_amount = previous
                .base_amount
                .checked_add(segment.base_amount)
                .ok_or_else(|| "fill base amount overflow".to_owned())?;
            previous.counter_amount = previous
                .counter_amount
                .checked_add(segment.counter_amount)
                .ok_or_else(|| "fill counter amount overflow".to_owned())?;
            previous.trade_count += 1;
            previous.last_time = segment.last_time;
            previous.last_trade_id = segment.last_trade_id;
        } else {
            result.push(segment);
        }
    }
    Ok(result)
}

fn segment_from_trade(raw: &Value, address: &str) -> Result<FillSegment, String> {
    let price = raw
        .get("price")
        .and_then(Value::as_object)
        .ok_or_else(|| "Invalid Horizon trade record: missing price".to_owned())?;
    let price_n = integer(price.get("n"))
        .filter(|value| *value > 0)
        .ok_or_else(|| "Invalid Horizon trade record: bad price numerator".to_owned())?;
    let price_d = integer(price.get("d"))
        .filter(|value| *value > 0)
        .ok_or_else(|| "Invalid Horizon trade record: bad price denominator".to_owned())?;
    let base_amount = parse_trade_amount(raw, "base_amount")?;
    let counter_amount = parse_trade_amount(raw, "counter_amount")?;
    let base_asset = trade_asset(raw, "base")?;
    let counter_asset = trade_asset(raw, "counter")?;
    let base_account = text(raw, "base_account");
    let counter_account = text(raw, "counter_account");
    let side = if base_account == Some(address) {
        "sell"
    } else if counter_account == Some(address) {
        "buy"
    } else if raw
        .get("base_is_seller")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        "sell"
    } else {
        "buy"
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
    Ok(FillSegment {
        base_asset,
        counter_asset,
        side: side.to_owned(),
        base_amount,
        counter_amount,
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

fn trade_asset(raw: &Value, prefix: &str) -> Result<String, String> {
    if text(raw, &format!("{prefix}_asset_type")) == Some("native") {
        return Ok("XLM".to_owned());
    }
    let code = text(raw, &format!("{prefix}_asset_code"))
        .ok_or_else(|| "Invalid Horizon trade asset code".to_owned())?;
    let issuer = text(raw, &format!("{prefix}_asset_issuer"))
        .ok_or_else(|| "Invalid Horizon trade asset issuer".to_owned())?;
    Ok(format!("{code}:{issuer}"))
}

fn trade_price(raw: &Value) -> String {
    if let Some(price) = raw.get("price").and_then(Value::as_object) {
        if let (Some(n), Some(d)) = (integer(price.get("n")), integer(price.get("d"))) {
            if n > 0 && d > 0 {
                if let Ok(value) = format_price_ratio(n, d) {
                    return value;
                }
            }
        }
    }
    let base = text(raw, "base_amount")
        .and_then(|value| parse_stroops(value, true).ok());
    let counter = text(raw, "counter_amount")
        .and_then(|value| parse_stroops(value, false).ok());
    match (base, counter) {
        (Some(base), Some(counter)) if base > 0 => {
            let numerator = i128::from(counter) * 10_000_000_i128;
            let rounded = (numerator + i128::from(base) / 2) / i128::from(base);
            i64::try_from(rounded)
                .ok()
                .map(format_fixed_7)
                .unwrap_or_else(|| "?".to_owned())
        }
        _ => "?".to_owned(),
    }
}

fn parse_trade_amount(raw: &Value, key: &str) -> Result<i64, String> {
    let value = text(raw, key).ok_or_else(|| format!("Invalid Horizon trade record: {key}"))?;
    parse_stroops(value, false)
        .map_err(|_| format!("Invalid Horizon trade record amount: {value}"))
}

fn ensure_pair(base: &ClassicAsset, counter: &ClassicAsset) -> Result<(), String> {
    if base == counter {
        Err("BASE and COUNTER assets must be different".to_owned())
    } else {
        Ok(())
    }
}

fn parse_limit(value: Option<&String>, usage: &str) -> Result<usize, String> {
    let value = value.ok_or_else(|| usage.to_owned())?;
    let limit = value
        .parse::<usize>()
        .map_err(|_| "--limit requires an integer from 1 to 200".to_owned())?;
    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
        return Err("--limit must be from 1 to 200".to_owned());
    }
    Ok(limit)
}

fn parse_u64(value: Option<&String>, name: &str) -> Result<u64, String> {
    value
        .ok_or_else(|| format!("{name} requires a millisecond timestamp"))?
        .parse()
        .map_err(|_| format!("{name} requires a non-negative millisecond value"))
}

fn resolution_value(value: &str) -> Result<u64, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1m" | "60000" => Ok(60_000),
        "5m" | "300000" => Ok(300_000),
        "15m" | "900000" => Ok(900_000),
        "1h" | "3600000" => Ok(3_600_000),
        "1d" | "86400000" => Ok(86_400_000),
        "1w" | "604800000" => Ok(604_800_000),
        _ => Err(format!("Unsupported trade aggregation resolution: {value}")),
    }
}

fn validate_offset(offset: u64, resolution: u64) -> Result<(), String> {
    if offset > resolution || offset >= 24 * HOUR_MS || offset % HOUR_MS != 0 {
        return Err(format!(
            "Invalid candle offset {offset} for resolution {resolution}"
        ));
    }
    Ok(())
}

fn display_integer(value: Option<&Value>) -> String {
    match value {
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::String(value)) => value.clone(),
        _ => "?".to_owned(),
    }
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
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

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn format_fixed_7(value: i64) -> String {
    let whole = value / 10_000_000;
    let fraction = value % 10_000_000;
    format!("{whole}.{fraction:07}")
}

fn usage() -> &'static str {
    "usage:\n  fresnica dex trades BASE COUNTER [--limit N] [--json]\n  fresnica dex fills [--wallet NAME] [--limit N] [--json]\n  fresnica dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn resolution_parser_matches_python_reference() {
        assert_eq!(resolution_value("1m").unwrap(), 60_000);
        assert_eq!(resolution_value("3600000").unwrap(), 3_600_000);
        assert!(resolution_value("2h").is_err());
    }

    #[test]
    fn account_fills_merge_only_consecutive_same_user_offer() {
        let first = json!({
            "id":"1",
            "paging_token":"1",
            "ledger_close_time":"2026-01-01T00:00:00Z",
            "base_asset_type":"native",
            "counter_asset_type":"credit_alphanum4",
            "counter_asset_code":"USD",
            "counter_asset_issuer":ACCOUNT,
            "base_amount":"1.0000000",
            "counter_amount":"2.0000000",
            "price":{"n":2,"d":1},
            "base_account":ACCOUNT,
            "counter_account":"GOTHER",
            "base_offer_id":"7",
            "counter_offer_id":"8",
            "base_is_seller":true
        });
        let second = json!({
            "id":"2",
            "paging_token":"2",
            "ledger_close_time":"2026-01-01T00:00:01Z",
            "base_asset_type":"native",
            "counter_asset_type":"credit_alphanum4",
            "counter_asset_code":"USD",
            "counter_asset_issuer":ACCOUNT,
            "base_amount":"3.0000000",
            "counter_amount":"6.0000000",
            "price":{"n":2,"d":1},
            "base_account":ACCOUNT,
            "counter_account":"GOTHER",
            "base_offer_id":"7",
            "counter_offer_id":"9",
            "base_is_seller":true
        });
        let segments = compress_account_trades(&[first, second], ACCOUNT).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].base_amount, 40_000_000);
        assert_eq!(segments[0].counter_amount, 80_000_000);
        assert_eq!(segments[0].trade_count, 2);
        assert_eq!(segments[0].offer_id.as_deref(), Some("7"));
    }

    #[test]
    fn missing_offer_ids_do_not_merge() {
        let trade = json!({
            "id":"1",
            "ledger_close_time":"2026-01-01T00:00:00Z",
            "base_asset_type":"native",
            "counter_asset_type":"native",
            "base_amount":"1.0000000",
            "counter_amount":"1.0000000",
            "price":{"n":1,"d":1},
            "base_is_seller":true
        });
        let segments = compress_account_trades(&[trade.clone(), trade], ACCOUNT).unwrap();
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn candle_offset_validation_matches_sdk_rule() {
        assert!(validate_offset(0, 3_600_000).is_ok());
        assert!(validate_offset(3_600_000, 3_600_000).is_ok());
        assert!(validate_offset(1_000, 86_400_000).is_err());
        assert!(validate_offset(86_400_000, 604_800_000).is_err());
    }

    #[test]
    fn trades_parser_bounds_limit() {
        let args = ["XLM", "XLM", "--limit", "201"].map(str::to_owned);
        assert_eq!(
            PairHistoryRequest::parse(&args, 20, "trades").unwrap_err(),
            "--limit must be from 1 to 200"
        );
    }
}
