mod anchor;
mod contacts;
mod dex;
mod friendbot;
mod read_commands;
mod send;
mod transaction_flow;
mod trust;

use std::env;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use fresnica_client::{
    wallet as wallet_ops, FresnicaClient, RevealedSigningMaterial, WalletRecord, WalletStorage,
};
use fresnica_sdk::{FresnicaSdk, SdkAccountKind};
use serde_json::Map;
use zeroize::Zeroizing;

const HELP: &str = r#"Fresnica native Rust CLI

Usage:
  fresnica [--home PATH] [--network mainnet|testnet] info [--wallet NAME]
  fresnica [--home PATH] [--network mainnet|testnet] account [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] balance [--wallet NAME] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] history [--wallet NAME] [--limit N] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]
  fresnica [--home PATH] contact COMMAND ...
  fresnica [--home PATH] [--network mainnet|testnet] trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] trust remove CODE:GISSUER [--wallet NAME] [-y]
  fresnica [--home PATH] [--network mainnet|testnet] dex orderbook SELLING BUYING [--json]
  fresnica [--home PATH] [--network mainnet|testnet] dex offers [--wallet NAME] [--limit N] [--json]
  fresnica [--network mainnet|testnet] anchor discover CODE:GISSUER [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor auth CODE:GISSUER [--wallet NAME]
  fresnica [--home PATH] [--network mainnet|testnet] anchor deposit CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor withdraw CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor status CODE:GISSUER ID [--wallet NAME] [--protocol sep24|sep6] [--pay] [-y] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] wallet COMMAND ...

Network commands:
  account                       Show current Horizon account state
  balance                       Show current account balances and liabilities
  history                       Show newest Horizon operations (default 20, max 200)
  send                          Review, sign through Fresnica SDK/Core, and submit a payment
  trust                         Add, change, or remove an issued-asset trustline
  dex                           Read and trade on the Stellar DEX
  anchor                        Discover anchor capabilities and start SEP-24/SEP-6 transfers

Contact commands:
  list
  add NAME G... [--memo TEXT]
  remove NAME

Wallet commands:
  list
  use NAME
  create NAME [--index N] [--language LANGUAGE] [--strength BITS]
  import-secret NAME
  import-mnemonic NAME [--index N] [--language LANGUAGE]
  import-watch NAME G...
  attach-secret NAME             Add matching S... signing material to watch-only wallet
  attach-mnemonic NAME [--index N] [--language LANGUAGE]
  detach-signer NAME             Remove local signing material and keep the G address
  testnet-fund [--wallet NAME]   Fund a testnet wallet with Friendbot
  fund [--wallet NAME]           Alias for testnet-fund
  reveal [NAME]
  backup NAME PATH [--force]
  restore PATH [--name NAME]
  delete NAME

The native client uses the platform-neutral Fresnica SDK for wallet protection and
signing, while low-level Stellar/XDR primitives remain in Rust Core. It uses the
same wallet files and version-1 encrypted backup format as the Python reference
client. All local software wallets share one Fresnica passphrase while retaining
independent Core salt/nonce-derived encryption keys.
"#;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let global = GlobalOptions::parse(&arguments)?;
    if global.command.is_empty() {
        print!("{HELP}");
        return Ok(());
    }
    if global.command == ["--help"] || global.command == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if global.command == ["--version"] || global.command == ["-V"] {
        println!("fresnica {} · Rust Core linked", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let client = FresnicaClient::new(&global.home, &global.network)?;
    let storage = client.storage();
    match global.command[0].as_str() {
        "info" => command_info(storage, &global.command[1..]),
        "account" => read_commands::command_account(&client, &global.command[1..]),
        "balance" | "assets" => read_commands::command_balance(&client, &global.command[1..]),
        "history" => read_commands::command_history(&client, &global.command[1..]),
        "send" => send::command_send(&client, &global.command[1..]),
        "contact" => contacts::command_contact(storage, &global.command[1..]),
        "trust" => trust::command_trust(&client, &global.command[1..]),
        "dex" => dex::command_dex(&client, &global.command[1..]),
        "anchor" => anchor::command_anchor(&client, &global.command[1..]),
        "wallet" => command_wallet(storage, &global.network, &global.command[1..]),
        other => Err(format!("unknown command: {other}\n\n{HELP}")),
    }
}

struct GlobalOptions {
    home: PathBuf,
    network: String,
    command: Vec<String>,
}

impl GlobalOptions {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let mut home = None;
        let mut network = "mainnet".to_owned();
        let mut index = 0;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--home" => {
                    index += 1;
                    let value = arguments
                        .get(index)
                        .ok_or_else(|| "--home requires a path".to_owned())?;
                    home = Some(expand_path(value)?);
                    index += 1;
                }
                "--network" => {
                    index += 1;
                    network = arguments
                        .get(index)
                        .ok_or_else(|| "--network requires mainnet or testnet".to_owned())?
                        .to_owned();
                    validate_network(&network)?;
                    index += 1;
                }
                _ => break,
            }
        }
        let home = match home {
            Some(home) => home,
            None => default_home()?,
        };
        Ok(Self {
            home,
            network,
            command: arguments[index..].to_vec(),
        })
    }
}

fn command_info(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let wallet_name = match arguments {
        [] => None,
        [flag, name] if flag == "--wallet" => Some(name.as_str()),
        _ => return Err("usage: fresnica info [--wallet NAME]".to_owned()),
    };
    let record = storage.resolve(wallet_name)?;
    let default = storage.default_name()?;
    println!("Name:       {}", record.name);
    println!("Address:    {}", record.address);
    println!("Network:    {}", record.network);
    println!("Type:       {}", record.wallet_type);
    println!(
        "Protection: {}",
        if record.watch_only() {
            "none"
        } else {
            "Fresnica passphrase envelope v1"
        }
    );
    println!(
        "Default:    {}",
        if default.as_deref() == Some(record.name.as_str()) {
            "yes"
        } else {
            "no"
        }
    );
    println!("SDK/Core:   Rust (direct link)");
    Ok(())
}

fn command_wallet(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err("wallet command is required\n\nSee `fresnica --help`.".to_owned());
    };
    match command {
        "list" if arguments.len() == 1 => wallet_list(storage),
        "use" if arguments.len() == 2 => {
            storage.set_default(&arguments[1])?;
            println!("Default wallet is now \"{}\"", arguments[1]);
            Ok(())
        }
        "create" => wallet_create(storage, network, &arguments[1..]),
        "import-secret" if arguments.len() == 2 => {
            wallet_import_secret(storage, network, &arguments[1])
        }
        "import-mnemonic" => wallet_import_mnemonic(storage, network, &arguments[1..]),
        "import-watch" if arguments.len() == 3 => {
            wallet_import_watch(storage, network, &arguments[1], &arguments[2])
        }
        "attach-secret" if arguments.len() == 2 => wallet_attach_secret(storage, &arguments[1]),
        "attach-mnemonic" => wallet_attach_mnemonic(storage, &arguments[1..]),
        "detach-signer" if arguments.len() == 2 => wallet_detach_signer(storage, &arguments[1]),
        "testnet-fund" | "fund" => friendbot::command_fund(storage, network, &arguments[1..]),
        "reveal" if arguments.len() <= 2 => {
            wallet_reveal(storage, arguments.get(1).map(String::as_str))
        }
        "backup" => wallet_backup(storage, &arguments[1..]),
        "restore" => wallet_restore(storage, &arguments[1..]),
        "delete" if arguments.len() == 2 => wallet_delete(storage, &arguments[1]),
        _ => Err(format!(
            "unknown or invalid wallet command: {command}\n\n{HELP}"
        )),
    }
}

fn wallet_list(storage: &WalletStorage) -> Result<(), String> {
    let records = storage.list()?;
    let default = storage.default_name()?;
    if records.is_empty() {
        println!("No local wallets.");
        return Ok(());
    }
    for record in records {
        let marker = if default.as_deref() == Some(record.name.as_str()) {
            "*"
        } else {
            " "
        };
        println!(
            "{marker} {:<20} {:<7} {:<10} {}",
            record.name, record.network, record.wallet_type, record.address
        );
    }
    Ok(())
}

fn wallet_create(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, true)?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let (record, mnemonic) = wallet_ops::create_mnemonic_record(
        name,
        network,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref().unwrap_or("english"),
        options.strength,
        &passcode,
    )?;
    save_new_record(storage, &record)?;
    println!("Created wallet \"{}\" [{}]", record.name, record.network);
    println!("Address: {}", record.address);
    println!("Mnemonic: {}", mnemonic.as_str());
    println!("Back up this mnemonic before using the wallet.");
    Ok(())
}

fn wallet_import_secret(storage: &WalletStorage, network: &str, name: &str) -> Result<(), String> {
    let secret = prompt_hidden("Stellar secret (S...): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let record = wallet_ops::import_secret_record(name, network, &secret, &passcode)?;
    save_new_record(storage, &record)?;
    println!("Imported wallet \"{}\"", record.name);
    println!("Address: {}", record.address);
    Ok(())
}

fn wallet_import_mnemonic(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, false)?;
    let mnemonic = prompt_hidden("Mnemonic phrase: ")?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let record = wallet_ops::import_mnemonic_record(
        name,
        network,
        &mnemonic,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref(),
        &passcode,
    )?;
    save_new_record(storage, &record)?;
    println!("Imported wallet \"{}\"", record.name);
    println!("Address: {}", record.address);
    Ok(())
}

fn wallet_import_watch(
    storage: &WalletStorage,
    network: &str,
    name: &str,
    address: &str,
) -> Result<(), String> {
    validate_network(network)?;
    if name.trim().is_empty() {
        return Err("wallet name cannot be empty".to_owned());
    }
    let identity = FresnicaSdk::new()
        .parse_account(address.to_owned())
        .map_err(|_| "invalid Stellar G address".to_owned())?;
    if identity.kind != SdkAccountKind::Classic {
        return Err("watch-only wallet requires a Classic G address".to_owned());
    }
    let record = WalletRecord {
        name: name.to_owned(),
        address: identity.address,
        wallet_type: "watch-only".to_owned(),
        network: network.to_owned(),
        secret: None,
        metadata: Map::new(),
    };
    save_new_record(storage, &record)?;
    println!("Added watch-only wallet \"{}\"", record.name);
    Ok(())
}

fn wallet_attach_secret(storage: &WalletStorage, name: &str) -> Result<(), String> {
    let record = storage.load(name)?;
    if !record.watch_only() {
        return Err("wallet already has signing material".to_owned());
    }
    let secret = prompt_hidden("Stellar secret (S...): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let updated = wallet_ops::attach_secret_record(&record, &secret, &passcode)?;
    storage.save(&updated, true)?;
    println!(
        "Attached matching secret signer to watch-only wallet \"{}\"",
        updated.name
    );
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_attach_mnemonic(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    let (name, options) = parse_mnemonic_options(arguments, false)?;
    let record = storage.load(name)?;
    if !record.watch_only() {
        return Err("wallet already has signing material".to_owned());
    }
    let mnemonic = prompt_hidden("Mnemonic phrase: ")?;
    let mnemonic_passphrase = prompt_hidden("BIP39 passphrase (optional; leave empty if none): ")?;
    let passcode = prompt_app_passcode(storage)?;
    let updated = wallet_ops::attach_mnemonic_record(
        &record,
        &mnemonic,
        &mnemonic_passphrase,
        options.index,
        options.language.as_deref(),
        &passcode,
    )?;
    storage.save(&updated, true)?;
    println!(
        "Attached matching mnemonic signer to watch-only wallet \"{}\"",
        updated.name
    );
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_detach_signer(storage: &WalletStorage, name: &str) -> Result<(), String> {
    let record = storage.load(name)?;
    if record.watch_only() {
        return Err("wallet is already watch-only".to_owned());
    }
    print!("Remove local signing material from wallet \"{name}\" and keep it watch-only? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Ok(());
    }
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    let updated = wallet_ops::detach_signer_record(&record, &passcode)?;
    storage.save(&updated, true)?;
    println!("Wallet \"{}\" is now watch-only", updated.name);
    println!("Address: {}", updated.address);
    Ok(())
}

fn wallet_reveal(storage: &WalletStorage, name: Option<&str>) -> Result<(), String> {
    let record = storage.resolve(name)?;
    if record.watch_only() {
        return Err("watch-only wallet has no signing material".to_owned());
    }
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    let material = wallet_ops::reveal_record(&record, &passcode)?;
    match material {
        RevealedSigningMaterial::Secret { secret } => {
            println!("Wallet: {}", record.name);
            println!("Stellar secret: {}", secret.as_str());
        }
        RevealedSigningMaterial::Mnemonic {
            mnemonic,
            mnemonic_passphrase,
            index,
            language,
        } => {
            println!("Wallet: {}", record.name);
            println!("Mnemonic: {}", mnemonic.as_str());
            if !mnemonic_passphrase.is_empty() {
                println!("BIP39 passphrase: {}", mnemonic_passphrase.as_str());
            }
            println!("Derivation index: {index}");
            println!("Language: {language}");
        }
    }
    Ok(())
}

fn wallet_backup(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 2 || arguments.len() > 3 {
        return Err("usage: fresnica wallet backup NAME PATH [--force]".to_owned());
    }
    let force = arguments.get(2).is_some_and(|value| value == "--force");
    if arguments.len() == 3 && !force {
        return Err("usage: fresnica wallet backup NAME PATH [--force]".to_owned());
    }
    let record = storage.load(&arguments[0])?;
    let destination = expand_path(&arguments[1])?;
    storage.write_backup(&record, &destination, force)?;
    println!(
        "Encrypted backup for \"{}\" written to {}",
        record.name,
        destination.display()
    );
    Ok(())
}

fn wallet_restore(storage: &WalletStorage, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() || arguments.len() > 3 {
        return Err("usage: fresnica wallet restore PATH [--name NAME]".to_owned());
    }
    let source = expand_path(&arguments[0])?;
    let mut record = WalletStorage::read_backup(&source)?;
    if arguments.len() > 1 {
        if arguments.len() != 3 || arguments[1] != "--name" {
            return Err("usage: fresnica wallet restore PATH [--name NAME]".to_owned());
        }
        if arguments[2].trim().is_empty() {
            return Err("restored wallet name cannot be empty".to_owned());
        }
        record.name = arguments[2].clone();
    }
    if !record.watch_only() && has_app_passcode(storage)? {
        let passcode = prompt_existing_app_passcode(storage)?;
        wallet_ops::verify_passcode(&record, &passcode)
            .map_err(|_| "backup does not use the current Fresnica passphrase".to_owned())?;
    }
    save_new_record(storage, &record)?;
    println!(
        "Restored wallet \"{}\" [{}]; unlock with the Fresnica passphrase",
        record.name, record.network
    );
    Ok(())
}

fn wallet_delete(storage: &WalletStorage, name: &str) -> Result<(), String> {
    storage.load(name)?;
    print!("Delete wallet \"{name}\" metadata and encrypted secret? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    if answer.trim().to_lowercase() != "y" {
        return Ok(());
    }
    let was_default = storage.default_name()?.as_deref() == Some(name);
    storage.delete(name)?;
    if was_default {
        if let Some(next) = storage.list()?.first() {
            storage.set_default(&next.name)?;
        }
    }
    println!("Deleted wallet \"{name}\"");
    Ok(())
}

fn save_new_record(storage: &WalletStorage, record: &WalletRecord) -> Result<(), String> {
    storage.save(record, false)?;
    if storage.default_name()?.is_none() {
        storage.set_default(&record.name)?;
    }
    Ok(())
}

fn signing_records(storage: &WalletStorage) -> Result<Vec<WalletRecord>, String> {
    Ok(storage
        .list()?
        .into_iter()
        .filter(|record| !record.watch_only() && record.secret.is_some())
        .collect())
}

fn has_app_passcode(storage: &WalletStorage) -> Result<bool, String> {
    Ok(!signing_records(storage)?.is_empty())
}

fn verify_app_passcode(storage: &WalletStorage, passcode: &str) -> Result<(), String> {
    for record in signing_records(storage)? {
        wallet_ops::verify_passcode(&record, passcode)?;
    }
    Ok(())
}

fn prompt_app_passcode(storage: &WalletStorage) -> Result<Zeroizing<String>, String> {
    if has_app_passcode(storage)? {
        prompt_existing_app_passcode(storage)
    } else {
        prompt_new_passcode()
    }
}

fn prompt_existing_app_passcode(storage: &WalletStorage) -> Result<Zeroizing<String>, String> {
    let passcode = prompt_hidden("Fresnica passphrase: ")?;
    if passcode.is_empty() {
        return Err("Fresnica passphrase cannot be empty".to_owned());
    }
    verify_app_passcode(storage, &passcode)?;
    Ok(passcode)
}

struct MnemonicOptions {
    index: usize,
    language: Option<String>,
    strength: usize,
}

fn parse_mnemonic_options<'a>(
    arguments: &'a [String],
    allow_strength: bool,
) -> Result<(&'a str, MnemonicOptions), String> {
    let name = arguments
        .first()
        .ok_or_else(|| "wallet name is required".to_owned())?;
    let mut index_value = 0usize;
    let mut language = if allow_strength {
        Some("english".to_owned())
    } else {
        None
    };
    let mut strength = 256usize;
    let mut cursor = 1;
    while cursor < arguments.len() {
        match arguments[cursor].as_str() {
            "--index" => {
                cursor += 1;
                index_value = arguments
                    .get(cursor)
                    .ok_or_else(|| "--index requires a number".to_owned())?
                    .parse()
                    .map_err(|_| "--index requires a non-negative integer".to_owned())?;
                cursor += 1;
            }
            "--language" => {
                cursor += 1;
                language = Some(
                    arguments
                        .get(cursor)
                        .ok_or_else(|| "--language requires a value".to_owned())?
                        .to_owned(),
                );
                cursor += 1;
            }
            "--strength" if allow_strength => {
                cursor += 1;
                strength = arguments
                    .get(cursor)
                    .ok_or_else(|| "--strength requires a value".to_owned())?
                    .parse()
                    .map_err(|_| "--strength requires a number".to_owned())?;
                if !matches!(strength, 128 | 160 | 192 | 224 | 256) {
                    return Err("--strength must be 128, 160, 192, 224, or 256".to_owned());
                }
                cursor += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok((
        name,
        MnemonicOptions {
            index: index_value,
            language,
            strength,
        },
    ))
}

fn prompt_hidden(prompt: &str) -> Result<Zeroizing<String>, String> {
    rpassword::prompt_password(prompt)
        .map(Zeroizing::new)
        .map_err(|error| format!("unable to read secret input: {error}"))
}

fn prompt_new_passcode() -> Result<Zeroizing<String>, String> {
    let passcode = prompt_hidden("Create Fresnica passphrase: ")?;
    let confirmation = prompt_hidden("Confirm Fresnica passphrase: ")?;
    wallet_ops::validate_new_passphrase(&passcode)?;
    if passcode.as_str() != confirmation.as_str() {
        return Err("Fresnica passphrases do not match".to_owned());
    }
    Ok(passcode)
}

fn validate_network(network: &str) -> Result<(), String> {
    if matches!(network, "mainnet" | "testnet") {
        Ok(())
    } else {
        Err(format!("unknown network: {network}"))
    }
}

fn default_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("FRESNICA_HOME") {
        let home = home.to_string_lossy();
        return expand_path(&home);
    }
    let base = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "unable to determine home directory; set FRESNICA_HOME".to_owned())?;
    Ok(PathBuf::from(base).join(".fresnica"))
}

fn expand_path(value: &str) -> Result<PathBuf, String> {
    if value == "~" {
        return env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| "unable to expand ~; set HOME or USERPROFILE".to_owned())?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(Path::new(value).to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_create_options_without_cli_framework() {
        let args = vec![
            "alpha".to_owned(),
            "--index".to_owned(),
            "4".to_owned(),
            "--language".to_owned(),
            "japanese".to_owned(),
            "--strength".to_owned(),
            "128".to_owned(),
        ];
        let (name, options) = parse_mnemonic_options(&args, true).unwrap();
        assert_eq!(name, "alpha");
        assert_eq!(options.index, 4);
        assert_eq!(options.language.as_deref(), Some("japanese"));
        assert_eq!(options.strength, 128);
    }
}
