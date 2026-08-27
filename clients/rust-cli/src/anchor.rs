use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value as JsonValue;
use zeroize::Zeroizing;

use crate::send::review_and_submit_payment;
use crate::transaction_flow::{network_passphrase, parse_transaction_xdr};
use fresnica_client::{
    anchor_status_requires_sep10, anchor_transaction_text as transaction_text,
    anchor_transfer_requires_sep10,
    anchor_withdrawal_payment_from_transaction as withdrawal_payment_from_transaction,
    exchange_anchor_sep10_challenge, fetch_anchor_transaction, get_anchor_customer,
    prepare_anchor_sep10_challenge, put_anchor_customer, satisfied_ed25519_conditions,
    select_anchor_status_protocol as select_status_protocol,
    select_anchor_transfer_protocol as select_transfer_protocol, sep10_authorization_plan,
    sign_needed_local_ed25519, start_anchor_sep24_transfer, start_anchor_sep6_transfer,
    AnchorAsset as IssuedAsset, AnchorCapabilities, AnchorCustomerFile, AnchorCustomerQuery,
    AnchorCustomerSnapshot, AnchorCustomerUpdate, AnchorProtocol,
    AnchorSep24InteractiveResult as Sep24InteractiveResult, AnchorTransferKind, FresnicaClient,
    LedgerSignerCondition, LedgerSignerKind, WalletRecord,
};

pub fn command_anchor(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let Some(command) = arguments.first().map(String::as_str) else {
        return Err(usage().to_owned());
    };
    match command {
        "discover" => command_discover(client, &arguments[1..]),
        "auth" => command_auth(client, &arguments[1..]),
        "deposit" => command_transfer(client, AnchorTransferKind::Deposit, &arguments[1..]),
        "withdraw" => command_transfer(client, AnchorTransferKind::Withdraw, &arguments[1..]),
        "status" => command_status(client, &arguments[1..]),
        "customer" => command_customer(client, &arguments[1..]),
        _ => Err(usage().to_owned()),
    }
}

fn command_discover(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }
    let mut json = false;
    for argument in &arguments[1..] {
        match argument.as_str() {
            "--json" => json = true,
            _ => return Err(usage().to_owned()),
        }
    }

    let discovery = client.discover_anchor(&arguments[0])?;
    let asset = &discovery.asset;
    let capabilities = &discovery.capabilities;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": client.network(),
                "capabilities": capabilities,
            }))
            .map_err(|error| format!("unable to encode anchor capabilities: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor · {} [{}]", asset.display(), client.network());
    println!("Domain:     {}", capabilities.domain);
    println!(
        "SEP-6:      deposit={} withdraw={}{}",
        yes_no(capabilities.sep6_deposit),
        yes_no(capabilities.sep6_withdraw),
        capabilities
            .sep6_url
            .as_deref()
            .map(|url| format!(" · {url}"))
            .unwrap_or_default()
    );
    println!(
        "SEP-24:     deposit={} withdraw={}{}",
        yes_no(capabilities.sep24_deposit),
        yes_no(capabilities.sep24_withdraw),
        capabilities
            .sep24_url
            .as_deref()
            .map(|url| format!(" · {url}"))
            .unwrap_or_default()
    );
    if let Some(url) = &capabilities.web_auth_url {
        println!("SEP-10:     {url}");
    }
    if let Some(url) = &capabilities.web_auth_for_contracts_url {
        println!("SEP-45:     {url}");
    }
    if let Some(url) = &capabilities.kyc_url {
        println!("KYC:        {url}");
    }
    for warning in &capabilities.warnings {
        println!("Warning:    {warning}");
    }
    Ok(())
}

fn command_auth(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }
    let mut wallet = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }

    let discovery = client.discover_anchor(&arguments[0])?;
    let record = client.resolve_wallet(wallet)?;
    let token = authenticate_anchor_sep10(
        client,
        &record,
        client.network(),
        &discovery.home_domain,
        &discovery.capabilities,
    )?;

    println!(
        "Authenticated · {} [{}]",
        discovery.capabilities.domain,
        client.network()
    );
    println!("Wallet:        {}", record.name);
    println!("Address:       {}", record.address);
    println!("Token:         verified in memory, then discarded");
    drop(token);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AnchorCustomerInput {
    #[serde(default)]
    fields: BTreeMap<String, JsonValue>,
    #[serde(default)]
    files: BTreeMap<String, AnchorCustomerInputFile>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AnchorCustomerInputFile {
    Path(String),
    Detail {
        path: String,
        #[serde(default)]
        content_type: Option<String>,
    },
}

fn command_customer(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }

    let mut wallet = None;
    let mut customer_id = None;
    let mut transaction_id = None;
    let mut customer_type = None;
    let mut lang = None;
    let mut input = None;
    let mut json = false;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--id" => {
                index += 1;
                customer_id = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--id requires a customer id".to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--transaction" => {
                index += 1;
                transaction_id = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| {
                            "--transaction requires an anchor transaction id".to_owned()
                        })?
                        .to_owned(),
                );
                index += 1;
            }
            "--type" => {
                index += 1;
                customer_type = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--type requires a customer type".to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--lang" => {
                index += 1;
                lang = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--lang requires a language code".to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--input" => {
                index += 1;
                input = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--input requires a JSON path or - for stdin".to_owned())?
                        .to_owned(),
                );
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    if input.is_some() && lang.is_some() {
        return Err("--lang is only valid for SEP-12 customer status lookup".to_owned());
    }

    let discovery = client.discover_anchor(&arguments[0])?;
    let asset = &discovery.asset;
    let capabilities = &discovery.capabilities;
    let server = capabilities.customer_server().ok_or_else(|| {
        format!(
            "{} does not advertise KYC_SERVER or TRANSFER_SERVER for SEP-12",
            capabilities.domain
        )
    })?;
    let record = client.resolve_wallet(wallet)?;
    let token = authenticate_anchor_sep10(
        client,
        &record,
        client.network(),
        &discovery.home_domain,
        capabilities,
    )?;

    if let Some(input) = input.as_deref() {
        let customer_input = read_anchor_customer_input(input)?;
        let update = build_anchor_customer_update(
            customer_id,
            transaction_id,
            customer_type,
            customer_input,
        )?;
        let result = put_anchor_customer(server, token.as_str(), &update)?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "asset": asset.display(),
                    "network": client.network(),
                    "anchor": capabilities.domain,
                    "wallet": record.name,
                    "customer": result,
                }))
                .map_err(|error| format!("unable to encode SEP-12 customer result: {error}"))?
            );
        } else {
            println!(
                "Anchor customer updated · {} [{}]",
                asset.display(),
                client.network()
            );
            println!("Anchor:      {}", capabilities.domain);
            println!("Wallet:      {}", record.name);
            println!("Customer:    {}", result.id);
            println!("Next:        query customer status with --id {}", result.id);
        }
        return Ok(());
    }

    let query = AnchorCustomerQuery {
        id: customer_id,
        customer_type,
        transaction_id,
        lang,
    };
    let snapshot = get_anchor_customer(server, token.as_str(), &query)?;
    render_anchor_customer(
        asset,
        &record,
        client.network(),
        &capabilities.domain,
        &snapshot,
        json,
    )
}

fn read_anchor_customer_input(source: &str) -> Result<AnchorCustomerInput, String> {
    let text = if source == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("unable to read SEP-12 JSON from stdin: {error}"))?;
        text
    } else {
        std::fs::read_to_string(source)
            .map_err(|error| format!("unable to read SEP-12 input {source}: {error}"))?
    };
    serde_json::from_str(&text).map_err(|error| format!("invalid SEP-12 input JSON: {error}"))
}

fn build_anchor_customer_update(
    id: Option<String>,
    transaction_id: Option<String>,
    customer_type: Option<String>,
    input: AnchorCustomerInput,
) -> Result<AnchorCustomerUpdate, String> {
    let mut fields = BTreeMap::new();
    for (name, value) in input.fields {
        let value = match value {
            JsonValue::String(value) => value,
            JsonValue::Number(value) => value.to_string(),
            _ => {
                return Err(format!(
                    "SEP-12 field {name} must be a string or number in the Rust CLI input"
                ))
            }
        };
        fields.insert(name, value);
    }

    let mut files = Vec::with_capacity(input.files.len());
    for (name, file) in input.files {
        let (path, content_type) = match file {
            AnchorCustomerInputFile::Path(path) => (path, None),
            AnchorCustomerInputFile::Detail { path, content_type } => (path, content_type),
        };
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("unable to read SEP-12 file {path}: {error}"))?;
        let file_name = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("SEP-12 file path has no usable file name: {path}"))?;
        files.push(AnchorCustomerFile {
            name,
            file_name: file_name.to_owned(),
            content_type,
            bytes,
        });
    }

    Ok(AnchorCustomerUpdate {
        id,
        customer_type,
        transaction_id,
        fields,
        files,
    })
}

fn render_anchor_customer(
    asset: &IssuedAsset,
    record: &WalletRecord,
    network: &str,
    domain: &str,
    snapshot: &AnchorCustomerSnapshot,
    json: bool,
) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": network,
                "anchor": domain,
                "wallet": record.name,
                "customer": snapshot,
            }))
            .map_err(|error| format!("unable to encode SEP-12 customer status: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor customer · {} [{}]", asset.display(), network);
    println!("Anchor:      {domain}");
    println!("Wallet:      {}", record.name);
    println!("Status:      {}", snapshot.status.label());
    if let Some(id) = snapshot.id.as_deref() {
        println!("Customer:    {id}");
    }
    if let Some(message) = snapshot.message.as_deref() {
        println!("Message:     {message}");
    }
    if !snapshot.required_fields.is_empty() {
        println!("Required:");
        for field in &snapshot.required_fields {
            let kind = field.field_type.as_deref().unwrap_or("unknown");
            let required = if field.optional {
                "optional"
            } else {
                "required"
            };
            let mut detail = format!("  {} [{} · {}]", field.name, kind, required);
            if !field.choices.is_empty() {
                detail.push_str(&format!(" choices={}", field.choices.join("|")));
            }
            println!("{detail}");
            if let Some(description) = field.description.as_deref() {
                println!("    {description}");
            }
        }
    }
    if !snapshot.provided_fields.is_empty() {
        println!("Provided:");
        for field in &snapshot.provided_fields {
            let status = field
                .status
                .map(|value| value.label())
                .unwrap_or("RECEIVED");
            println!("  {} [{}]", field.name, status);
            if let Some(error) = field.error.as_deref() {
                println!("    {error}");
            }
        }
    }
    Ok(())
}

fn command_transfer(
    client: &FresnicaClient,
    kind: AnchorTransferKind,
    arguments: &[String],
) -> Result<(), String> {
    if arguments.is_empty() {
        return Err(usage().to_owned());
    }
    let mut wallet = None;
    let mut json = false;
    let mut fields = BTreeMap::new();
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--field" => {
                index += 1;
                let raw = arguments
                    .get(index)
                    .ok_or_else(|| "--field requires NAME=VALUE".to_owned())?;
                let (name, value) = parse_transfer_field(raw)?;
                if fields.insert(name.clone(), value).is_some() {
                    return Err(format!("duplicate anchor field: {name}"));
                }
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }

    let discovery = client.discover_anchor(&arguments[0])?;
    let asset = &discovery.asset;
    let capabilities = &discovery.capabilities;
    let protocol = select_transfer_protocol(capabilities, kind)?;
    let record = client.resolve_wallet(wallet)?;
    let token = if anchor_transfer_requires_sep10(capabilities, protocol, kind) {
        Some(authenticate_anchor_sep10(
            client,
            &record,
            client.network(),
            &discovery.home_domain,
            capabilities,
        )?)
    } else {
        None
    };

    match protocol {
        AnchorProtocol::Sep24 => {
            let token = token
                .as_ref()
                .ok_or_else(|| "SEP-24 requires SEP-10 authentication".to_owned())?;
            let result = start_anchor_sep24_transfer(
                &record.address,
                asset,
                capabilities,
                kind,
                &fields,
                token.as_str(),
            )?;
            render_sep24_result(
                asset,
                &record,
                client.network(),
                &capabilities.domain,
                kind,
                &result,
                json,
            )
        }
        AnchorProtocol::Sep6 => {
            let response = start_anchor_sep6_transfer(
                &record.address,
                asset,
                capabilities,
                kind,
                &fields,
                token.as_ref().map(|token| token.as_str()),
            )?;
            render_sep6_result(
                asset,
                &record,
                client.network(),
                &capabilities.domain,
                kind,
                response,
                json,
            )
        }
    }
}

fn command_status(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    if arguments.len() < 2 {
        return Err(usage().to_owned());
    }
    let transaction_id = arguments[1].trim();
    if transaction_id.is_empty() {
        return Err("anchor transaction id must not be empty".to_owned());
    }

    let mut wallet = None;
    let mut protocol = None;
    let mut pay = false;
    let mut yes = false;
    let mut json = false;
    let mut index = 2;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| "--wallet requires a wallet name".to_owned())?
                        .as_str(),
                );
                index += 1;
            }
            "--protocol" => {
                index += 1;
                protocol =
                    Some(parse_anchor_protocol(arguments.get(index).ok_or_else(
                        || "--protocol requires sep24 or sep6".to_owned(),
                    )?)?);
                index += 1;
            }
            "--pay" => {
                pay = true;
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    if yes && !pay {
        return Err("--yes is only valid together with --pay".to_owned());
    }
    if json && pay {
        return Err(
            "--json cannot be combined with --pay because payment requires transaction review"
                .to_owned(),
        );
    }

    let discovery = client.discover_anchor(&arguments[0])?;
    let asset = &discovery.asset;
    let capabilities = &discovery.capabilities;
    let protocol = select_status_protocol(capabilities, protocol)?;
    let record = client.resolve_wallet(wallet)?;
    let token = if anchor_status_requires_sep10(capabilities, protocol) {
        Some(authenticate_anchor_sep10(
            client,
            &record,
            client.network(),
            &discovery.home_domain,
            capabilities,
        )?)
    } else {
        None
    };
    let transaction = fetch_anchor_transaction(
        capabilities,
        protocol,
        transaction_id,
        token.as_ref().map(|token| token.as_str()),
    )?;

    render_anchor_transaction_status(
        asset,
        &record,
        client.network(),
        &capabilities.domain,
        protocol,
        &transaction,
        json,
    )?;

    if !pay {
        return Ok(());
    }

    let payment = withdrawal_payment_from_transaction(&transaction, &record.address, asset)?;
    println!();
    println!("Anchor withdrawal payment handoff");
    println!("Anchor:     {}", capabilities.domain);
    println!("Transfer:   {transaction_id}");
    if let Some(value) = transaction_text(&transaction, "to") {
        println!("External:   {value}");
    }
    if let Some(value) = transaction_text(&transaction, "external_extra_text") {
        println!("Details:    {value}");
    }
    if let Some(value) = transaction_text(&transaction, "more_info_url") {
        println!("More info:  {value}");
    }

    review_and_submit_payment(
        client,
        &record,
        &payment.amount,
        &asset.display(),
        &payment.destination,
        None,
        payment.memo,
        yes,
    )
}

fn parse_anchor_protocol(value: &str) -> Result<AnchorProtocol, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "sep24" | "sep-24" => Ok(AnchorProtocol::Sep24),
        "sep6" | "sep-6" => Ok(AnchorProtocol::Sep6),
        _ => Err("--protocol must be sep24 or sep6".to_owned()),
    }
}

fn render_anchor_transaction_status(
    asset: &IssuedAsset,
    record: &WalletRecord,
    network: &str,
    domain: &str,
    protocol: AnchorProtocol,
    transaction: &JsonValue,
    json: bool,
) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": network,
                "wallet": record.name,
                "address": record.address,
                "domain": domain,
                "protocol": protocol,
                "transaction": transaction,
            }))
            .map_err(|error| format!("unable to encode anchor transaction status: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor transaction · {} [{}]", asset.display(), network);
    println!("Protocol:    {}", protocol.label());
    println!("Domain:      {domain}");
    println!("Wallet:      {}", record.name);
    for (label, key) in [
        ("ID", "id"),
        ("Kind", "kind"),
        ("Status", "status"),
        ("Amount in", "amount_in"),
        ("Amount out", "amount_out"),
        ("Fee", "amount_fee"),
        ("Stellar TX", "stellar_transaction_id"),
        ("External TX", "external_transaction_id"),
        ("Action by", "user_action_required_by"),
        ("More info", "more_info_url"),
    ] {
        if let Some(value) = transaction_text(transaction, key) {
            println!("{label:<12} {value}");
        }
    }
    if transaction_text(transaction, "status") == Some("pending_user_transfer_start")
        && matches!(
            transaction_text(transaction, "kind"),
            Some("withdrawal") | Some("withdraw")
        )
    {
        println!("Next action: withdrawal payment is ready; rerun with --pay to review it.");
    }
    if transaction_text(transaction, "status") == Some("pending_customer_info_update") {
        println!(
            "Next action: anchor requires customer information; use `anchor customer` for SEP-12 status/update."
        );
    }
    Ok(())
}

fn parse_transfer_field(value: &str) -> Result<(String, String), String> {
    let (name, value) = value
        .split_once('=')
        .ok_or_else(|| "--field requires NAME=VALUE".to_owned())?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() || value.is_empty() {
        return Err("--field requires non-empty NAME=VALUE".to_owned());
    }
    if matches!(name, "asset_code" | "asset_issuer" | "account") {
        return Err(format!("anchor field is managed by Fresnica: {name}"));
    }
    Ok((name.to_owned(), value.to_owned()))
}

fn authenticate_anchor_sep10(
    client: &FresnicaClient,
    record: &WalletRecord,
    network: &str,
    home_domain: &str,
    capabilities: &AnchorCapabilities,
) -> Result<Zeroizing<String>, String> {
    let ledger_account = client.ledger_account(&record.address)?;
    let authorization = sep10_authorization_plan(ledger_account.as_ref(), &record.address)?;
    let challenge =
        prepare_anchor_sep10_challenge(network, &record.address, home_domain, capabilities)?;
    let mut envelope = parse_transaction_xdr(challenge.transaction_xdr())?;
    let mut satisfied =
        satisfied_ed25519_conditions(&authorization, &envelope, network_passphrase(network)?)?;
    satisfied.remove(&LedgerSignerCondition {
        kind: LedgerSignerKind::Ed25519PublicKey,
        key: challenge.server_signing_key().to_owned(),
    });
    let excluded = BTreeSet::from([challenge.server_signing_key().to_owned()]);
    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    sign_needed_local_ed25519(
        client.storage(),
        &authorization,
        &satisfied,
        &excluded,
        1,
        network,
        &mut envelope,
        passcode.as_str(),
    )?;
    exchange_anchor_sep10_challenge(network, &challenge, &authorization, &envelope)
}

fn render_sep24_result(
    asset: &IssuedAsset,
    record: &WalletRecord,
    network: &str,
    domain: &str,
    kind: AnchorTransferKind,
    result: &Sep24InteractiveResult,
    json: bool,
) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": network,
                "wallet": record.name,
                "address": record.address,
                "domain": domain,
                "kind": kind,
                "protocol": AnchorProtocol::Sep24,
                "url": result.url,
                "id": result.transaction_id,
            }))
            .map_err(|error| format!("unable to encode anchor transfer result: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor · {} [{}]", asset.display(), network);
    println!("Action:      {} via SEP-24", kind.endpoint());
    println!("Domain:      {domain}");
    println!("Wallet:      {}", record.name);
    println!("Open URL:    {}", result.url);
    println!("Transfer ID: {}", result.transaction_id);
    println!(
        "Status:      fresnica --network {network} anchor status {} {} --protocol sep24 --wallet {}",
        asset.display(),
        result.transaction_id,
        record.name
    );
    Ok(())
}

fn render_sep6_result(
    asset: &IssuedAsset,
    record: &WalletRecord,
    network: &str,
    domain: &str,
    kind: AnchorTransferKind,
    response: JsonValue,
    json: bool,
) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "asset": asset.display(),
                "network": network,
                "wallet": record.name,
                "address": record.address,
                "domain": domain,
                "kind": kind,
                "protocol": AnchorProtocol::Sep6,
                "response": response,
            }))
            .map_err(|error| format!("unable to encode anchor transfer result: {error}"))?
        );
        return Ok(());
    }

    println!("Anchor · {} [{}]", asset.display(), network);
    println!("Action:      {} via SEP-6", kind.endpoint());
    println!("Domain:      {domain}");
    println!("Wallet:      {}", record.name);
    println!("Instructions:");
    println!(
        "{}",
        serde_json::to_string_pretty(&response)
            .map_err(|error| format!("unable to encode anchor response: {error}"))?
    );
    if let Some(transaction_id) = transaction_text(&response, "id") {
        println!(
            "Status:      fresnica --network {network} anchor status {} {transaction_id} --protocol sep6 --wallet {}",
            asset.display(),
            record.name
        );
    }
    Ok(())
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn usage() -> &'static str {
    "usage:\n  fresnica [--network mainnet|testnet] anchor discover CODE:GISSUER [--json]\n  fresnica [--home PATH] [--network mainnet|testnet] anchor auth CODE:GISSUER [--wallet NAME]\n  fresnica [--home PATH] [--network mainnet|testnet] anchor deposit CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]\n  fresnica [--home PATH] [--network mainnet|testnet] anchor withdraw CODE:GISSUER [--wallet NAME] [--field NAME=VALUE]... [--json]\n  fresnica [--home PATH] [--network mainnet|testnet] anchor status CODE:GISSUER ID [--wallet NAME] [--protocol sep24|sep6] [--pay] [-y] [--json]
  fresnica [--home PATH] [--network mainnet|testnet] anchor customer CODE:GISSUER [--wallet NAME] [--id CUSTOMER_ID] [--transaction ID --type TYPE] [--type TYPE] [--lang en] [--input PATH|-] [--json]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_fields_reject_reserved_names_and_accept_custom_values() {
        assert!(parse_transfer_field("account=GABC").is_err());
        assert!(parse_transfer_field("asset_code=USD").is_err());
        assert_eq!(
            parse_transfer_field("bank_account=1234").unwrap(),
            ("bank_account".to_owned(), "1234".to_owned())
        );
    }

    #[test]
    fn sep12_customer_input_accepts_scalar_fields_and_binary_files() {
        let path =
            std::env::temp_dir().join(format!("fresnica-sep12-{}-id.jpg", std::process::id()));
        std::fs::write(&path, [1_u8, 2, 3]).unwrap();
        let input = AnchorCustomerInput {
            fields: BTreeMap::from([
                ("first_name".to_owned(), JsonValue::String("Ada".to_owned())),
                ("annual_income".to_owned(), serde_json::json!(42)),
            ]),
            files: BTreeMap::from([(
                "photo_id_front".to_owned(),
                AnchorCustomerInputFile::Detail {
                    path: path.to_string_lossy().into_owned(),
                    content_type: Some("image/jpeg".to_owned()),
                },
            )]),
        };
        let update = build_anchor_customer_update(
            Some("customer-1".to_owned()),
            None,
            Some("sep6".to_owned()),
            input,
        )
        .unwrap();
        assert_eq!(
            update.fields.get("first_name").map(String::as_str),
            Some("Ada")
        );
        assert_eq!(
            update.fields.get("annual_income").map(String::as_str),
            Some("42")
        );
        assert_eq!(update.files.len(), 1);
        assert_eq!(update.files[0].name, "photo_id_front");
        assert_eq!(update.files[0].content_type.as_deref(), Some("image/jpeg"));
        assert_eq!(update.files[0].bytes, vec![1, 2, 3]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn sep12_customer_input_rejects_nested_values() {
        let input = AnchorCustomerInput {
            fields: BTreeMap::from([(
                "organization".to_owned(),
                serde_json::json!({"name": "Example"}),
            )]),
            files: BTreeMap::new(),
        };
        assert_eq!(
            build_anchor_customer_update(None, None, None, input).unwrap_err(),
            "SEP-12 field organization must be a string or number in the Rust CLI input"
        );
    }
}
