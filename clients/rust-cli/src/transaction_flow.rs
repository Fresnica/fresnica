use std::io::{self, Write};

use fresnica_client::{
    resolve_local_signing_wallet as client_resolve_local_signing_wallet,
    resolve_signing_wallet as client_resolve_signing_wallet,
    sign_and_submit as client_sign_and_submit,
    sign_transaction_xdr_with_passcode as client_sign_transaction_xdr_with_passcode, HorizonClient,
    WalletRecord, WalletStorage,
};
use stellar_xdr::TransactionEnvelope;

pub(crate) use fresnica_client::{
    account_sequence, balance_stroops, build_operation_envelope, format_stroops,
    has_valid_transaction_signature, minimum_balance_stroops, network_client, network_passphrase,
    parse_stroops, parse_transaction_xdr, STROOPS_PER_XLM,
};

pub fn resolve_signing_wallet(
    storage: &WalletStorage,
    network: &str,
    name: Option<&str>,
) -> Result<WalletRecord, String> {
    let horizon = network_client(network)?;
    client_resolve_signing_wallet(storage, &horizon, network, name)
}

pub(crate) fn resolve_local_signing_wallet(
    storage: &WalletStorage,
    network: &str,
    name: Option<&str>,
) -> Result<WalletRecord, String> {
    client_resolve_local_signing_wallet(storage, network, name)
}

pub fn confirm_submission() -> Result<bool, String> {
    print!("Submit this transaction? [y/N] ");
    io::stdout()
        .flush()
        .map_err(|error| format!("unable to write prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("unable to read confirmation: {error}"))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

pub fn sign_and_submit(
    storage: &WalletStorage,
    record: &WalletRecord,
    network: &str,
    envelope: &mut TransactionEnvelope,
    horizon: &HorizonClient,
) -> Result<(), String> {
    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    let submission = client_sign_and_submit(
        storage,
        record,
        network,
        envelope,
        horizon,
        passcode.as_str().to_owned(),
    )?;
    println!("Submitted: {}", submission.hash);
    if let Some(ledger) = submission.ledger {
        println!("Ledger:    {ledger}");
    }
    Ok(())
}

pub(crate) fn sign_transaction_xdr_with_wallet(
    record: &WalletRecord,
    network: &str,
    transaction_xdr: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    client_sign_transaction_xdr_with_passcode(
        record,
        network,
        transaction_xdr,
        passcode.as_str().to_owned(),
    )
}
