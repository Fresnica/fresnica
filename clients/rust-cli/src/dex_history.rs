use fresnica_client::{DexTradeSide, FresnicaClient};

use super::MAX_PAGE_LIMIT;

pub fn command_dex_history(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "trades" => command_trades(client, &arguments[1..]),
        "fills" => command_fills(client, &arguments[1..]),
        "candles" => command_candles(client, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn command_trades(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = PairHistoryRequest::parse(arguments, 20, "trades")?;
    let snapshot = client.pair_trades(&request.base, &request.counter, request.limit)?;

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.trades)
                .map_err(|error| format!("unable to encode trades: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Trades · {}/{} [{}]",
        snapshot.base,
        snapshot.counter,
        client.network()
    );
    println!(
        "{:<24} {:>16} {:>16} {:>14} {:<9}",
        "Time", "Base", "Counter", "Price", "Base side"
    );
    for trade in snapshot.trades {
        println!(
            "{:<24} {:>16} {:>16} {:>14} {:<9}",
            trade.ledger_close_time.as_deref().unwrap_or(""),
            trade.base_amount,
            trade.counter_amount,
            trade.price,
            match trade.base_side {
                DexTradeSide::Sell => "sell",
                DexTradeSide::Buy => "buy",
            },
        );
    }
    Ok(())
}

fn command_fills(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = FillRequest::parse(arguments)?;
    let snapshot = client.account_fills(request.wallet.as_deref(), request.limit)?;

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.fills)
                .map_err(|error| format!("unable to encode fills: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Offer fills · {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    println!(
        "{:<24} {:<5} {:<25} {:>14} {:>14} {:>14} {:>5} {:<12}",
        "Time", "Side", "Pair", "Base", "Counter", "Price", "Fills", "Offer"
    );
    for fill in snapshot.fills {
        println!(
            "{:<24} {:<5} {:<25} {:>14} {:>14} {:>14} {:>5} {:<12}",
            fill.last_time
                .as_deref()
                .or(fill.first_time.as_deref())
                .unwrap_or(""),
            fill.side.label(),
            format!("{}/{}", fill.base_asset, fill.counter_asset),
            fill.base_amount,
            fill.counter_amount,
            fill.price,
            fill.trade_count,
            fill.offer_id.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

fn command_candles(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = CandleRequest::parse(arguments)?;
    let snapshot = client.candles(
        &request.base,
        &request.counter,
        request.resolution_ms,
        request.start_time,
        request.end_time,
        request.offset,
        request.limit,
    )?;

    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.candles)
                .map_err(|error| format!("unable to encode candles: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Candles · {}/{} · {} [{}]",
        snapshot.base,
        snapshot.counter,
        request.resolution,
        client.network()
    );
    println!(
        "{:<16} {:>14} {:>14} {:>14} {:>14} {:>16} {:>8}",
        "Time(ms)", "Open", "High", "Low", "Close", "Base volume", "Trades"
    );
    for candle in snapshot.candles.iter().rev() {
        println!(
            "{:<16} {:>14} {:>14} {:>14} {:>14} {:>16} {:>8}",
            candle.timestamp,
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.base_volume,
            candle.trade_count,
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
        let usage = format!("usage: fresnica dex {command} BASE COUNTER [--limit N] [--json]");
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

fn usage() -> &'static str {
    "usage:\n  fresnica dex trades BASE COUNTER [--limit N] [--json]\n  fresnica dex fills [--wallet NAME] [--limit N] [--json]\n  fresnica dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_parser_matches_reference_aliases() {
        assert_eq!(resolution_value("1m").unwrap(), 60_000);
        assert_eq!(resolution_value("3600000").unwrap(), 3_600_000);
        assert!(resolution_value("2h").is_err());
    }

    #[test]
    fn trades_parser_bounds_limit() {
        let args = ["XLM", "XLM", "--limit", "201"].map(str::to_owned);
        assert_eq!(
            PairHistoryRequest::parse(&args, 20, "trades").unwrap_err(),
            "--limit must be from 1 to 200"
        );
    }

    #[test]
    fn candle_parser_accepts_time_window_and_offset() {
        let args = [
            "XLM",
            "USD:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            "--resolution",
            "1h",
            "--start",
            "1000",
            "--end",
            "2000",
            "--offset",
            "0",
        ]
        .map(str::to_owned);
        let request = CandleRequest::parse(&args).unwrap();
        assert_eq!(request.resolution_ms, 3_600_000);
        assert_eq!(request.start_time, Some(1000));
        assert_eq!(request.end_time, Some(2000));
        assert_eq!(request.offset, Some(0));
    }
}
