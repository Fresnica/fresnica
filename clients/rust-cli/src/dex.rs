#[path = "dex_history.rs"]
mod history;
#[path = "dex_write.rs"]
mod write;

use fresnica_client::FresnicaClient;

const MAX_PAGE_LIMIT: usize = 200;

pub fn command_dex(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "orderbook" => command_orderbook(client, &arguments[1..]),
        "offers" => command_offers(client, &arguments[1..]),
        "buy" | "sell" | "update" | "cancel" => write::command_dex_write(client, arguments),
        "trades" | "fills" | "candles" => history::command_dex_history(client, arguments),
        _ => Err(usage().to_owned()),
    }
}

fn command_orderbook(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = OrderbookRequest::parse(arguments)?;
    let orderbook = client.order_book(&request.selling, &request.buying)?;
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&orderbook)
                .map_err(|error| format!("unable to encode order book: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Stellar DEX · {}/{} [{}]",
        orderbook.base,
        orderbook.counter,
        client.network()
    );
    println!("BID · BUY                              ASK · SELL");
    println!(
        "{:>16} {:>14}    {:<14} {:<16}",
        "Amount", "Price", "Price", "Amount"
    );
    let count = orderbook.bids.len().max(orderbook.asks.len());
    for index in 0..count {
        let bid = orderbook.bids.get(index);
        let ask = orderbook.asks.get(index);
        println!(
            "{:>16} {:>14}    {:<14} {:<16}",
            bid.map(|level| level.amount.as_str()).unwrap_or(""),
            bid.map(|level| level.price.as_str()).unwrap_or(""),
            ask.map(|level| level.price.as_str()).unwrap_or(""),
            ask.map(|level| level.amount.as_str()).unwrap_or("")
        );
    }
    Ok(())
}

fn command_offers(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = OffersRequest::parse(arguments)?;
    let snapshot = client.open_offers(request.wallet.as_deref(), request.limit)?;
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.offers)
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

fn usage() -> &'static str {
    "usage:\n  fresnica dex orderbook SELLING BUYING [--json]\n  fresnica dex offers [--wallet NAME] [--limit N] [--json]\n  fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]\n  fresnica dex cancel OFFER_ID [--wallet NAME] [-y]\n  fresnica dex trades BASE COUNTER [--limit N] [--json]\n  fresnica dex fills [--wallet NAME] [--limit N] [--json]\n  fresnica dex candles BASE COUNTER [--resolution 1m|5m|15m|1h|1d|1w] [--start MS] [--end MS] [--offset MS] [--limit N] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_orderbook_request() {
        let request = OrderbookRequest::parse(&[
            "XLM".to_owned(),
            "USDC:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF".to_owned(),
            "--json".to_owned(),
        ])
        .unwrap();
        assert_eq!(request.selling, "XLM");
        assert_eq!(
            request.buying,
            "USDC:GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
        );
        assert!(request.json);
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
