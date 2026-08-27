use fresnica_client::{
    FresnicaClient, PreparedTrustline, TrustlineAction, TrustlineRequest, TrustlineReview,
};

use crate::transaction_flow::confirm_submission;

pub fn command_trust(client: &FresnicaClient, arguments: &[String]) -> Result<(), String> {
    let request = TrustRequest::parse(arguments)?;
    let prepared = client.prepare_trustline(&request.service_request())?;
    review_and_submit(client, &prepared, request.yes())
}

fn review_and_submit(
    client: &FresnicaClient,
    prepared: &PreparedTrustline,
    yes: bool,
) -> Result<(), String> {
    render_review(&prepared.review);
    if !yes && !confirm_submission()? {
        println!("Transaction cancelled.");
        return Ok(());
    }

    let passcode = crate::prompt_hidden("Fresnica passcode: ")?;
    let submission = client.submit_trustline(prepared, passcode.as_str().to_owned())?;
    println!("Submitted: {}", submission.hash);
    if let Some(ledger) = submission.ledger {
        println!("Ledger:    {ledger}");
    }
    Ok(())
}

fn render_review(review: &TrustlineReview) {
    println!("Review transaction");
    println!("Operation: ChangeTrust ({})", review.operation.label());
    println!("Wallet:    {} ({})", review.wallet_name, review.source);
    println!("Asset:     {}", review.asset);
    if let Some(limit) = &review.limit {
        println!("Limit:     {limit}");
    }
    if let Some(authorization) = review.authorization {
        println!("Auth:      {}", authorization.label());
    }
    if let Some(clawback_enabled) = review.clawback_enabled {
        println!(
            "Clawback:  {}",
            if clawback_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    println!("Fee:       {} XLM", review.fee_xlm);
    println!("Network:   {}", review.network);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrustRequest {
    Add {
        asset: String,
        limit: Option<String>,
        wallet: Option<String>,
        yes: bool,
    },
    Limit {
        asset: String,
        limit: String,
        wallet: Option<String>,
        yes: bool,
    },
    Remove {
        asset: String,
        wallet: Option<String>,
        yes: bool,
    },
}

impl TrustRequest {
    fn parse(arguments: &[String]) -> Result<Self, String> {
        let Some(command) = arguments.first().map(String::as_str) else {
            return Err(usage().to_owned());
        };
        match command {
            "add" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, limit) = parse_options(&arguments[2..], true)?;
                Ok(Self::Add {
                    asset,
                    limit,
                    wallet,
                    yes,
                })
            }
            "limit" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let limit = arguments.get(2).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra) = parse_options(&arguments[3..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                Ok(Self::Limit {
                    asset,
                    limit,
                    wallet,
                    yes,
                })
            }
            "remove" => {
                let asset = arguments.get(1).ok_or_else(|| usage().to_owned())?.clone();
                let (wallet, yes, extra) = parse_options(&arguments[2..], false)?;
                if extra.is_some() {
                    return Err(usage().to_owned());
                }
                Ok(Self::Remove { asset, wallet, yes })
            }
            _ => Err(usage().to_owned()),
        }
    }

    fn service_request(&self) -> TrustlineRequest {
        match self {
            Self::Add {
                asset,
                limit,
                wallet,
                ..
            } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::Add {
                    limit: limit.clone(),
                },
            },
            Self::Limit {
                asset,
                limit,
                wallet,
                ..
            } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::SetLimit {
                    limit: limit.clone(),
                },
            },
            Self::Remove { asset, wallet, .. } => TrustlineRequest {
                wallet: wallet.clone(),
                asset: asset.clone(),
                action: TrustlineAction::Remove,
            },
        }
    }

    fn yes(&self) -> bool {
        match self {
            Self::Add { yes, .. } | Self::Limit { yes, .. } | Self::Remove { yes, .. } => *yes,
        }
    }
}

fn parse_options(
    arguments: &[String],
    allow_limit: bool,
) -> Result<(Option<String>, bool, Option<String>), String> {
    let mut wallet = None;
    let mut yes = false;
    let mut limit = None;
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
            "--limit" if allow_limit => {
                index += 1;
                limit = Some(
                    arguments
                        .get(index)
                        .ok_or_else(|| usage().to_owned())?
                        .clone(),
                );
                index += 1;
            }
            "-y" | "--yes" => {
                yes = true;
                index += 1;
            }
            _ => return Err(usage().to_owned()),
        }
    }
    Ok((wallet, yes, limit))
}

fn usage() -> &'static str {
    "usage: fresnica trust add CODE:GISSUER [--limit VALUE] [--wallet NAME] [-y]\n       fresnica trust limit CODE:GISSUER LIMIT [--wallet NAME] [-y]\n       fresnica trust remove CODE:GISSUER [--wallet NAME] [-y]"
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn add_parser_accepts_limit_wallet_and_yes() {
        let args = [
            "add",
            &format!("USD:{ISSUER}"),
            "--limit",
            "1000",
            "--wallet",
            "alpha",
            "-y",
        ]
        .map(str::to_owned);
        let request = TrustRequest::parse(&args).unwrap();
        assert!(request.yes());
        assert_eq!(
            request.service_request(),
            TrustlineRequest {
                wallet: Some("alpha".to_owned()),
                asset: format!("USD:{ISSUER}"),
                action: TrustlineAction::Add {
                    limit: Some("1000".to_owned())
                }
            }
        );
    }
}
