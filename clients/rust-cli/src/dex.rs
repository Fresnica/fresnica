#[path = "dex_history.rs"]
mod history;
#[path = "dex_write.rs"]
mod write;

use fresnica_sdk::{FresnicaSdk, SdkAccountKind};
use serde_json::Value;

use crate::transaction_flow::parse_stroops;
use fresnica_client::{
    FresnicaClient, WalletRecord, WalletStorage, MAINNET_HORIZON_URL, TESTNET_HORIZON_URL,
};

const MAX_PAGE_LIMIT: usize = 200;
const STROOPS_PER_UNIT: i128 = 10_000_000;

pub fn command_dex(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let storage = client.storage();
    let network = client.network();
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "orderbook" => command_orderbook(network, &arguments[1..]),
        "offers" => command_offers(client, &arguments[1..]),
        "buy" | "sell" | "update" | "cancel" => write::command_dex_write(client, arguments),
        "trades" | "fills" | "candles" => history::command_dex_history(storage, network, arguments),
        _ => Err(usage().to_owned()),
    }
}

fn command_orderbook(network: &str, arguments: &[String]) -> Result<(), String> {
    let request = OrderbookRequest::parse(arguments)?;
    let selling = ClassicAsset::parse(&request.selling)?;
    let buying = ClassicAsset::parse(&request.buying)?;
    if selling == buying {
        return Err("BASE and COUNTER assets must be different".to_owned());
    }

    let orderbook = fetch_orderbook(network, &selling, &buying)?;
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&orderbook)
                .map_err(|error| format!("unable to encode order book: {error}"))?
        );
        return Ok(());
    }

    let bids = rows(&orderbook, "bids")?;
    let asks = rows(&orderbook, "asks")?;
    println!(
        "Stellar DEX · {}/{} [{}]",
        selling.display(),
        buying.display(),
        network
    );
    println!("BID · BUY                              ASK · SELL");
    println!(
        "{:>16} {:>14}    {:<14} {:<16}",
        "Amount", "Price", "Price", "Amount"
    );
    let count = bids.len().max(asks.len());
    for index in 0..count {
        let bid = bids.get(index).map(book_bid_cells).transpose()?;
        let ask = asks.get(index).map(book_ask_cells).transpose()?;
        let (bid_amount, bid_price) = bid.unwrap_or_else(|| (String::new(), String::new()));
        let (ask_price, ask_amount) = ask.unwrap_or_else(|| (String::new(), String::new()));
        println!(
            "{:>16} {:>14}    {:<14} {:<16}",
            bid_amount, bid_price, ask_price, ask_amount
        );
    }
    Ok(())
}

fn command_offers(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = OffersRequest::parse(arguments)?;
    let snapshot = client.open_offers(request.wallet.as_deref(), request.limit)?;
    if request.json {
        let raw = snapshot
            .offers
            .iter()
            .map(|offer| offer.raw())
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&raw)
                .map_err(|error| format!("unable to encode offers: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Offers · {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    println!(
        "{:<12} {:<24} {:<24} {:>16} {:>14}",
        "ID", "Selling", "Buying", "Amount", "Price"
    );
    for offer in snapshot.offers {
        println!(
            "{:<12} {:<24} {:<24} {:>16} {:>14}",
            offer.offer_id, offer.selling, offer.buying, offer.amount, offer.price
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrderbookRequest {
    selling: String,
    buying: String,
    json: bool,
}

impl OrderbookRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        if arguments.len() < 2 {
            return Err("usage: fresnica dex orderbook SELLING BUYING [--json]".to_owned());
        }
        let selling = arguments[0].clone();
        let buying = arguments[1].clone();
        let mut json = false;
        for argument in &arguments[2..] {
            match argument.as_str() {
                "--json" => json = true,
                _ => return Err("usage: fresnica dex orderbook SELLING BUYING [--json]".to_owned()),
            }
        }
        Ok(Self {
            selling,
            buying,
            json,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OffersRequest {
    wallet: Option<String>,
    limit: usize,
    json: bool,
}

impl OffersRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let usage = "usage: fresnica dex offers [--wallet NAME] [--limit N] [--json]";
        let mut wallet = None;
        let mut limit = 20usize;
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
                    limit = arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .parse()
                        .map_err(|_| "--limit requires an integer from 1 to 200".to_owned())?;
                    if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
                        return Err("--limit must be from 1 to 200".to_owned());
                    }
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
enum ClassicAsset {
    Native,
    Issued { code: String, issuer: String },
}

impl ClassicAsset {
    fn parse(value: &str) -> Result<Self, String> {
        let text = value.trim();
        if text.eq_ignore_ascii_case("XLM") {
            return Ok(Self::Native);
        }
        let (code, issuer) = text
            .split_once(':')
            .ok_or_else(|| "Issued assets must use CODE:GISSUER".to_owned())?;
        let code = code.trim();
        let issuer = issuer.trim();
        if code.is_empty()
            || code.len() > 12
            || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err("Asset code must be 1 to 12 ASCII alphanumeric characters".to_owned());
        }
        let identity = FresnicaSdk::new()
            .parse_account(issuer.to_owned())
            .map_err(|_| "invalid Stellar asset issuer".to_owned())?;
        if identity.kind != SdkAccountKind::Classic {
            return Err("asset issuer must be a Classic G address".to_owned());
        }
        Ok(Self::Issued {
            code: code.to_owned(),
            issuer: identity.address,
        })
    }

    fn display(&self) -> String {
        match self {
            Self::Native => "XLM".to_owned(),
            Self::Issued { code, .. } => code.clone(),
        }
    }

    fn query(&self, prefix: &str) -> String {
        match self {
            Self::Native => format!("{prefix}_asset_type=native"),
            Self::Issued { code, issuer } => {
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
}

fn fetch_orderbook(
    network: &str,
    selling: &ClassicAsset,
    buying: &ClassicAsset,
) -> Result<Value, String> {
    let base = horizon_url(network)?;
    let url = format!(
        "{base}/order_book?{}&{}",
        selling.query("selling"),
        buying.query("buying")
    );
    get_json(&url, "Unable to load Stellar order book")
}

fn get_json(url: &str, not_found: &str) -> Result<Value, String> {
    let mut response = match ureq::get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(404)) => return Err(not_found.to_owned()),
        Err(ureq::Error::StatusCode(code)) => {
            return Err(format!("Horizon returned HTTP {code} for {url}"))
        }
        Err(error) => return Err(format!("Unable to contact Horizon at {url}: {error}")),
    };
    response
        .body_mut()
        .read_json::<Value>()
        .map_err(|error| format!("Horizon returned invalid JSON for {url}: {error}"))
}

fn horizon_url(network: &str) -> Result<&'static str, String> {
    match network {
        "mainnet" => Ok(MAINNET_HORIZON_URL),
        "testnet" => Ok(TESTNET_HORIZON_URL),
        other => Err(format!("unknown network: {other}")),
    }
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

fn rows<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("Horizon returned malformed order book {key}"))
}

fn book_bid_cells(row: &Value) -> Result<(String, String), String> {
    let amount = parse_stroops(text(row, "amount").unwrap_or(""), true)
        .map_err(|_| "Horizon returned invalid order book amount".to_owned())?;
    let (n, d) = price_ratio(row)?;
    let numerator = i128::from(amount)
        .checked_mul(i128::from(d))
        .ok_or_else(|| "order book bid amount overflow".to_owned())?;
    let base_stroops = round_ratio(numerator, i128::from(n))?;
    Ok((format_scaled_7(base_stroops), format_price_ratio(n, d)?))
}

fn book_ask_cells(row: &Value) -> Result<(String, String), String> {
    let amount = parse_stroops(text(row, "amount").unwrap_or(""), true)
        .map_err(|_| "Horizon returned invalid order book amount".to_owned())?;
    let (n, d) = price_ratio(row)?;
    Ok((
        format_price_ratio(n, d)?,
        format_scaled_7(i128::from(amount)),
    ))
}

fn price_ratio(value: &Value) -> Result<(i64, i64), String> {
    let ratio = value
        .get("price_r")
        .and_then(Value::as_object)
        .ok_or_else(|| "Horizon returned malformed price ratio".to_owned())?;
    let n = integer(ratio.get("n"))
        .filter(|value| *value > 0)
        .ok_or_else(|| "Horizon returned invalid price numerator".to_owned())?;
    let d = integer(ratio.get("d"))
        .filter(|value| *value > 0)
        .ok_or_else(|| "Horizon returned invalid price denominator".to_owned())?;
    Ok((n, d))
}

fn format_price_ratio(n: i64, d: i64) -> Result<String, String> {
    let numerator = i128::from(n)
        .checked_mul(STROOPS_PER_UNIT)
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
    let whole = value / STROOPS_PER_UNIT;
    let fraction = value % STROOPS_PER_UNIT;
    format!("{whole}.{fraction:07}")
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn usage() -> &'static str {
    "usage:\n  fresnica dex orderbook SELLING BUYING [--json]\n  fresnica dex offers [--wallet NAME] [--limit N] [--json]\n  fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]\n  fresnica dex cancel OFFER_ID [--wallet NAME] [-y]\n  fresnica dex trades BASE COUNTER [--limit N] [--json]\n  fresnica dex fills [--wallet NAME] [--limit N] [--json]\n  fresnica dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn mock_server(expected_target: &'static str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(
                request.starts_with(&format!("GET {expected_target} HTTP/1.1")),
                "unexpected request: {request}"
            );
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        format!("http://{address}")
    }

    #[test]
    fn parses_classic_market_assets() {
        assert_eq!(ClassicAsset::parse("XLM").unwrap(), ClassicAsset::Native);
        let issuer = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        assert_eq!(
            ClassicAsset::parse(&format!("USDC:{issuer}")).unwrap(),
            ClassicAsset::Issued {
                code: "USDC".to_owned(),
                issuer: issuer.to_owned(),
            }
        );
    }

    #[test]
    fn bid_amount_is_normalized_to_base_with_exact_price_ratio() {
        let row: Value = serde_json::from_str(
            r#"{"amount":"14.2000000","price":"2.0000000","price_r":{"n":2,"d":1}}"#,
        )
        .unwrap();
        assert_eq!(
            book_bid_cells(&row).unwrap(),
            ("7.1000000".to_owned(), "2.0000000".to_owned())
        );
    }

    #[test]
    fn tiny_nonzero_price_does_not_render_as_zero() {
        assert_eq!(format_price_ratio(1, 30_000_000).unwrap(), "<0.0000001");
    }

    #[test]
    fn orderbook_query_uses_full_asset_identity() {
        let issuer = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        let body = r#"{"bids":[],"asks":[]}"#;
        let base = mock_server(
            "/order_book?selling_asset_type=native&buying_asset_type=credit_alphanum4&buying_asset_code=USDC&buying_asset_issuer=GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            body,
        );
        let selling = ClassicAsset::Native;
        let buying = ClassicAsset::parse(&format!("USDC:{issuer}")).unwrap();
        let url = format!(
            "{base}/order_book?{}&{}",
            selling.query("selling"),
            buying.query("buying")
        );
        let value = get_json(&url, "not found").unwrap();
        assert_eq!(value["bids"], serde_json::json!([]));
    }

    #[test]
    fn offers_limit_is_bounded() {
        let args = vec!["--limit".to_owned(), "201".to_owned()];
        assert_eq!(
            OffersRequest::parse(&args).unwrap_err(),
            "--limit must be from 1 to 200"
        );
    }
}
