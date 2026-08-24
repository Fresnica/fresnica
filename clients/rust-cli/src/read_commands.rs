use serde_json::Value;

use crate::horizon::{
    balance_asset_label, operation_summary, HorizonClient, MAINNET_HORIZON_URL,
    TESTNET_HORIZON_URL,
};
use crate::storage::{WalletRecord, WalletStorage};

pub fn command_account(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica account [--wallet NAME] [--json]")?;
    let record = resolve_network_wallet(storage, network, options.wallet.as_deref())?;
    let account = client(network)?.get_account(&record.address)?;
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&account)
                .map_err(|error| format!("unable to encode account data: {error}"))?
        );
        return Ok(());
    }

    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    println!("Wallet:       {}", record.name);
    println!("Address:      {}", record.address);
    println!("Network:      {}", record.network);
    println!("Sequence:     {}", display_value(account.get("sequence")));
    println!("Subentries:   {}", display_value(account.get("subentry_count")));
    println!("Sponsoring:   {}", display_value(account.get("num_sponsoring")));
    println!("Sponsored:    {}", display_value(account.get("num_sponsored")));
    println!(
        "Home domain:  {}",
        account
            .get("home_domain")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
    );
    println!("Assets:       {balances}");
    Ok(())
}

pub fn command_balance(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let options = parse_output_options(arguments, "fresnica balance [--wallet NAME] [--json]")?;
    let record = resolve_network_wallet(storage, network, options.wallet.as_deref())?;
    let account = client(network)?.get_account(&record.address)?;
    let balances = account
        .get("balances")
        .and_then(Value::as_array)
        .ok_or_else(|| "Horizon returned malformed balance data".to_owned())?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(balances)
                .map_err(|error| format!("unable to encode balance data: {error}"))?
        );
        return Ok(());
    }

    println!("Wallet: {} [{}]", record.name, record.network);
    println!(
        "{:<72} {:>16} {:>16} {:>16}",
        "Asset", "Balance", "Selling", "Buying"
    );
    for balance in balances {
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

pub fn command_history(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let options = parse_history_options(arguments)?;
    let record = resolve_network_wallet(storage, network, options.wallet.as_deref())?;
    let operations = client(network)?.get_operations(&record.address, options.limit)?;

    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&operations)
                .map_err(|error| format!("unable to encode history data: {error}"))?
        );
        return Ok(());
    }

    println!("Wallet: {} [{}]", record.name, record.network);
    if operations.is_empty() {
        println!("No account operations.");
        return Ok(());
    }
    for operation in operations {
        let created_at = text(&operation, "created_at").unwrap_or("?");
        let operation_type = text(&operation, "type").unwrap_or("unknown");
        println!(
            "{:<20} {:<28} {}",
            created_at,
            operation_type,
            operation_summary(&operation, &record.address)
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

fn client(network: &str) -> Result<HorizonClient, String> {
    let url = match network {
        "mainnet" => MAINNET_HORIZON_URL,
        "testnet" => TESTNET_HORIZON_URL,
        other => return Err(format!("unknown network: {other}")),
    };
    Ok(HorizonClient::new(url))
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
