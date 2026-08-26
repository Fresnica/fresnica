use fresnica_client::{FresnicaClient, OfferRequest, OfferReview, OfferReviewDetails, OfferSide};

use crate::transaction_flow::confirm_submission;

pub fn command_dex_write(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = WriteRequest::parse(arguments)?;
    let prepared = client.prepare_offer(&request.service)?;
    render_offer_review(&prepared.review);
    if !request.yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }
    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    let submission = client.submit_offer(&prepared, passcode.as_str().to_owned())?;
    println!("Submitted: {}", submission.hash);
    if let Some(ledger) = submission.ledger {
        println!("Ledger:    {ledger}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WriteRequest {
    service: OfferRequest,
    yes: bool,
}

impl WriteRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "buy" | "sell" => {
                if arguments.len() < 5 {
                    return Err(usage().to_owned());
                }
                let side = if command == "buy" {
                    OfferSide::Buy
                } else {
                    OfferSide::Sell
                };
                let (wallet, allow_trustline, yes) = parse_options(&arguments[5..], true)?;
                Ok(Self {
                    service: OfferRequest::Create {
                        side,
                        base: arguments[1].clone(),
                        counter: arguments[2].clone(),
                        amount: arguments[3].clone(),
                        price: arguments[4].clone(),
                        wallet,
                        allow_trustline,
                    },
                    yes,
                })
            }
            "update" => {
                if arguments.len() < 6 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes) = parse_options(&arguments[6..], false)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self {
                    service: OfferRequest::Update {
                        offer_id,
                        base: arguments[2].clone(),
                        counter: arguments[3].clone(),
                        amount: arguments[4].clone(),
                        price: arguments[5].clone(),
                        wallet,
                    },
                    yes,
                })
            }
            "cancel" => {
                if arguments.len() < 2 {
                    return Err(usage().to_owned());
                }
                let offer_id = parse_offer_id(&arguments[1])?;
                let (wallet, allow_trustline, yes) = parse_options(&arguments[2..], false)?;
                if allow_trustline {
                    return Err(usage().to_owned());
                }
                Ok(Self {
                    service: OfferRequest::Cancel { wallet, offer_id },
                    yes,
                })
            }
            _ => Err(usage().to_owned()),
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_trustline_option: bool,
) -> Result<(Option<String>, bool, bool), String> {
    let mut wallet = None;
    let mut allow_trustline = false;
    let mut yes = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .clone(),
                );
                index += 1;
            }
            "--allow-trustline" if allow_trustline_option => {
                allow_trustline = true;
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, allow_trustline, yes))
}

fn parse_offer_id(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| "offer id must be a positive integer".to_owned())
}

fn render_offer_review(review: &OfferReview) {
    println!("Review transaction");
    println!(
        "Operation: {} ({})",
        review.operation.label(),
        review.action.label()
    );
    println!("Wallet:    {} ({})", review.wallet_name, review.source);
    if let Some(offer_id) = review.offer_id {
        println!("Offer:     #{offer_id}");
    }
    match &review.details {
        OfferReviewDetails::Trade {
            side,
            base,
            counter,
            amount,
            price,
            requested_price,
            price_n,
            price_d,
            total,
            trustline_asset,
            trustline_limit,
        } => {
            println!("Side:      {}", side.label());
            println!("Pair:      {base} / {counter}");
            println!("Amount:    {amount} {base}");
            println!("Price:     {price} {counter}/{base}");
            println!("Encoded:   {price_n}/{price_d}");
            if let Some(requested) = requested_price {
                println!("Requested: {requested} {counter}/{base}");
            }
            println!("Total:     {total} {counter}");
            if let Some(asset) = trustline_asset {
                let limit = trustline_limit
                    .as_deref()
                    .map(|value| format!("; limit {value}"))
                    .unwrap_or_default();
                println!("Trustline: + {asset}{limit} (explicitly approved)");
            }
        }
        OfferReviewDetails::Cancel { selling, buying } => {
            println!("Selling:   {selling}");
            println!("Buying:    {buying}");
        }
    }
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
}

fn usage() -> &'static str {
    "usage:\n  fresnica dex buy BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex sell BASE COUNTER AMOUNT PRICE [--wallet NAME] [--allow-trustline] [-y]\n  fresnica dex update OFFER_ID BASE COUNTER AMOUNT PRICE [--wallet NAME] [-y]\n  fresnica dex cancel OFFER_ID [--wallet NAME] [-y]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn parser_matches_python_cli_write_shape() {
        let args = [
            "buy",
            "XLM",
            &format!("USD:{ISSUER}"),
            "10",
            "2.5",
            "--allow-trustline",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = WriteRequest::parse(&args).unwrap();
        assert!(request.yes);
        assert!(matches!(
            request.service,
            OfferRequest::Create {
                side: OfferSide::Buy,
                allow_trustline: true,
                wallet: Some(ref wallet),
                ..
            } if wallet == "alpha"
        ));
    }

    #[test]
    fn update_and_cancel_require_positive_offer_ids() {
        assert!(WriteRequest::parse(&[
            "update".to_owned(),
            "0".to_owned(),
            "XLM".to_owned(),
            format!("USD:{ISSUER}"),
            "1".to_owned(),
            "1".to_owned(),
        ])
        .is_err());
        assert!(WriteRequest::parse(&["cancel".to_owned(), "-1".to_owned()]).is_err());
    }
}
