use fresnica_client::{balance_asset_label, operation_summary, FresnicaClient};
use serde_json::Value;

pub fn command_account(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica account [--wallet NAME] [--json]")?;
    let snapshot = client.account(options.wallet.as_deref())?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.account)
                .map_err(|error| format!("unable to encode account data: {error}"))?
        );
        return Ok(());
    }

    let balances = snapshot
        .account
        .get("balances")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    println!("Wallet:       {}", snapshot.wallet.name);
    println!("Address:      {}", snapshot.wallet.address);
    println!("Network:      {}", snapshot.wallet.network);
    println!(
        "Sequence:     {}",
        display_value(snapshot.account.get("sequence"))
    );
    println!(
        "Subentries:   {}",
        display_value(snapshot.account.get("subentry_count"))
    );
    println!(
        "Sponsoring:   {}",
        display_value(snapshot.account.get("num_sponsoring"))
    );
    println!(
        "Sponsored:    {}",
        display_value(snapshot.account.get("num_sponsored"))
    );
    println!(
        "Home domain:  {}",
        snapshot
            .account
            .get("home_domain")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
    );
    println!("Assets:       {balances}");
    Ok(())
}

pub fn command_balance(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica balance [--wallet NAME] [--json]")?;
    let snapshot = client.balances(options.wallet.as_deref())?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.balances)
                .map_err(|error| format!("unable to encode balance data: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Wallet: {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    println!(
        "{:<72} {:>16} {:>16} {:>16}",
        "Asset", "Balance", "Selling", "Buying"
    );
    for balance in &snapshot.balances {
        println!(
            "{:<72} {:>16} {:>16} {:>16}",
            balance_asset_label(balance),
            text(balance, "balance").unwrap_or("0"),
            text(balance, "selling_liabilities").unwrap_or("0"),
            text(balance, "buying_liabilities").unwrap_or("0"),
        );
    }
    Ok(())
}

pub fn command_history(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let options = parse_history_options(arguments)?;
    let snapshot = client.history(options.wallet.as_deref(), options.limit)?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&snapshot.operations)
                .map_err(|error| format!("unable to encode history data: {error}"))?
        );
        return Ok(());
    }

    println!(
        "Wallet: {} [{}]",
        snapshot.wallet.name, snapshot.wallet.network
    );
    if snapshot.operations.is_empty() {
        println!("No account operations.");
        return Ok(());
    }
    for operation in &snapshot.operations {
        let created_at = text(operation, "created_at").unwrap_or("?");
        let operation_type = text(operation, "type").unwrap_or("unknown");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation_type,
            operation_summary(operation, &snapshot.wallet.address)
        );
    }
    Ok(())
}

struct OutputOptions {
    wallet: Option<String>,
    json: bool,
}

#[derive(Debug)]
struct HistoryOptions {
    wallet: Option<String>,
    json: bool,
    limit: usize,
}

fn parse_output_options(arguments: &[String], usage: &str) -> Result<OutputOptions, String> {
    let mut wallet = None;
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
                        .to_owned(),
                );
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage.to_owned()),
        }
    }
    Ok(OutputOptions { wallet, json })
}

fn parse_history_options(arguments: &[String]) -> Result<HistoryOptions, String> {
    let usage = "fresnica history [--wallet NAME] [--limit N] [--json]";
    let mut wallet = None;
    let mut json = false;
    let mut limit = 20usize;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage.to_owned())?
                        .to_owned(),
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
                if !(1..=200).contains(&limit) {
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
    Ok(HistoryOptions {
        wallet,
        json,
        limit,
    })
}

fn display_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => "-".to_owned(),
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_limit_is_bounded_by_horizon_page_size() {
        let args = vec!["--limit".to_owned(), "201".to_owned()];
        assert_eq!(
            parse_history_options(&args).unwrap_err(),
            "--limit must be from 1 to 200"
        );
    }

    #[test]
    fn output_options_accept_wallet_and_json_in_either_order() {
        let args = vec![
            "--json".to_owned(),
            "--wallet".to_owned(),
            "alpha".to_owned(),
        ];
        let options = parse_output_options(&args, "usage").unwrap();
        assert!(options.json);
        assert_eq!(options.wallet.as_deref(), Some("alpha"));
    }
}
