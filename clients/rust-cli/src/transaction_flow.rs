use std::io::{self, Write};

use fresnica_client::{
    sign_and_submit as client_sign_and_submit, HorizonGateway, WalletRecord, WalletStorage,
};
use stellar_xdr::TransactionEnvelope;

pub(crate) use fresnica_client::{
    format_stroops, has_valid_transaction_signature, network_gateway, network_passphrase,
    parse_stroops, parse_transaction_xdr,
};

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
    horizon: &HorizonGateway,
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
