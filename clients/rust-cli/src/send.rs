use fresnica_client::{
    FresnicaClient, PaymentMemo, PaymentRequest, PaymentReview, PreparedPayment, WalletRecord,
};

use crate::transaction_flow::confirm_submission;

pub fn command_send(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = SendRequest::parse(arguments)?;
    let prepared = client.prepare_payment(&PaymentRequest {
        wallet: request.wallet.clone(),
        amount: request.amount,
        asset: request.asset,
        destination: request.destination,
        memo: request.memo,
    })?;
    review_and_submit_prepared(client, &prepared, request.yes)
}

pub(crate) fn review_and_submit_payment(
    client: &FresnicaClient,
    record: &WalletRecord,
    amount_text: &str,
    asset_text: &str,
    destination_address: &str,
    contact_name: Option<&str>,
    memo: PaymentMemo,
    yes: bool,
) -> Result<(), String> {
    let prepared = client.prepare_payment_to_address(
        record,
        amount_text,
        asset_text,
        destination_address,
        contact_name,
        memo,
    )?;
    review_and_submit_prepared(client, &prepared, yes)
}

fn review_and_submit_prepared(
    client: &FresnicaClient,
    prepared: &PreparedPayment,
    yes: bool,
) -> Result<(), String> {
    render_review(&prepared.review);
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }

    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    let submission = client.submit_payment(prepared, passcode.as_str().to_owned())?;
    println!("Submitted: {}", submission.hash);
    if let Some(ledger) = submission.ledger {
        println!("Ledger:    {ledger}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SendRequest {
    amount: String,
    asset: String,
    destination: String,
    wallet: Option<String>,
    memo: Option<String>,
    yes: bool,
}

impl SendRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        const USAGE: &str =
            "usage: fresnica send AMOUNT ASSET to DESTINATION [--wallet NAME] [--memo TEXT] [-y]";
        if arguments.len() < 4 || arguments[2].to_lowercase() != "to" {
            return Err(USAGE.to_owned());
        }
        let mut wallet = None;
        let mut memo = None;
        let mut yes = false;
        let mut index = 4;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--wallet" => {
                    index += 1;
                    wallet = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "--memo" => {
                    index += 1;
                    memo = Some(
                        arguments
                            .get(index)
                            .ok_or_else(|| USAGE.to_owned())?
                            .clone(),
                    );
                    index += 1;
                }
                "-y" | "--yes" => {
                    yes = true;
                    index += 1;
                }
                _ => return Err(USAGE.to_owned()),
            }
        }
        Ok(Self {
            amount: arguments[0].clone(),
            asset: arguments[1].clone(),
            destination: arguments[3].clone(),
            wallet,
            memo,
            yes,
        })
    }
}

fn render_review(review: &PaymentReview) {
    println!("Review transaction");
    println!("Operation: {}", review.operation.label());
    println!("From:      {} ({})", review.wallet_name, review.source);
    if let Some(name) = &review.contact_name {
        println!("To:        {name} ({})", review.destination);
    } else {
        println!("To:        {}", review.destination);
    }
    println!("Amount:    {} {}", review.amount, review.asset);
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
    if let Some(memo) = &review.memo {
        println!("Memo:      {} ({})", memo.value, memo.memo_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESTINATION: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn send_parser_matches_python_cli_shape() {
        let args = [
            "1.5",
            "XLM",
            "to",
            DESTINATION,
            "--memo",
            "hello",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = SendRequest::parse(&args).unwrap();
        assert_eq!(request.amount, "1.5");
        assert_eq!(request.asset, "XLM");
        assert_eq!(request.destination, DESTINATION);
        assert_eq!(request.memo.as_deref(), Some("hello"));
        assert_eq!(request.wallet.as_deref(), Some("alpha"));
        assert!(request.yes);
    }
}
