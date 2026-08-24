use serde_json::Value;

pub const MAINNET_HORIZON_URL: &str = "https://horizon.stellar.org";
pub const TESTNET_HORIZON_URL: &str = "https://horizon-testnet.stellar.org";

pub struct HorizonClient {
    base_url: String,
}

impl HorizonClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub fn get_account(&self, address: &str) -> Result<Value, String> {
        self.get_json(
            &format!("/accounts/{address}"),
            &format!("Stellar account not found: {address}"),
        )
    }

    pub fn get_operations(&self, address: &str, limit: usize) -> Result<Vec<Value>, String> {
        let response = self.get_json(
            &format!("/accounts/{address}/operations?order=desc&limit={limit}"),
            &format!("Stellar account not found: {address}"),
        )?;
        response
            .get("_embedded")
            .and_then(|value| value.get("records"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "Horizon returned malformed operation data".to_owned())
    }

    fn get_json(&self, path: &str, not_found: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut response = match ureq::get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Err(not_found.to_owned()),
            Err(ureq::Error::StatusCode(code)) => {
                return Err(format!("Horizon returned HTTP {code} for {url}"))
            }
            Err(error) => return Err(format!("Unable to contact Horizon at {url}: {error}")),
        };
        response
            .body_mut()
            .read_json::<Value>()
            .map_err(|error| format!("Horizon returned invalid JSON for {url}: {error}"))
    }
}

pub fn balance_asset_label(balance: &Value) -> String {
    match text(balance, "asset_type") {
        Some("native") => "XLM".to_owned(),
        Some("liquidity_pool_shares") => format!(
            "LP:{}",
            text(balance, "liquidity_pool_id").unwrap_or("unknown")
        ),
        _ => {
            let code = text(balance, "asset_code").unwrap_or("asset");
            match text(balance, "asset_issuer") {
                Some(issuer) => format!("{code}:{issuer}"),
                None => code.to_owned(),
            }
        }
    }
}

pub fn operation_summary(operation: &Value, account: &str) -> String {
    match text(operation, "type").unwrap_or("unknown") {
        "payment" => {
            let amount = amount(operation, "amount");
            let asset = operation_asset(operation, "");
            let source = text(operation, "from")
                .or_else(|| text(operation, "source_account"))
                .unwrap_or("?");
            let destination = text(operation, "to").unwrap_or("?");
            if destination == account {
                format!("Received {amount} {asset} from {}", short_address(source))
            } else if source == account {
                format!("Sent {amount} {asset} to {}", short_address(destination))
            } else {
                format!(
                    "{amount} {asset}: {} -> {}",
                    short_address(source),
                    short_address(destination)
                )
            }
        }
        "create_account" => {
            let created = text(operation, "account").unwrap_or("?");
            let funder = text(operation, "funder")
                .or_else(|| text(operation, "source_account"))
                .unwrap_or("?");
            let starting = amount(operation, "starting_balance");
            if created == account {
                format!("Account created with {starting} XLM")
            } else if funder == account {
                format!("Created {} with {starting} XLM", short_address(created))
            } else {
                format!("Created {} with {starting} XLM", short_address(created))
            }
        }
        "manage_sell_offer" | "create_passive_sell_offer" => {
            let offer_id = text(operation, "offer_id").unwrap_or("0");
            let amount = amount(operation, "amount");
            if amount == "0" {
                return format!("Cancelled offer #{offer_id}");
            }
            let selling = operation_asset(operation, "selling_");
            let buying = operation_asset(operation, "buying_");
            let price = amount(operation, "price");
            if text(operation, "type") == Some("create_passive_sell_offer") {
                format!("Placed passive SELL {amount} {selling} @ {price} {buying}/{selling}")
            } else {
                let verb = if offer_id == "0" {
                    "Placed".to_owned()
                } else {
                    format!("Updated #{offer_id}")
                };
                format!("{verb} SELL {amount} {selling} @ {price} {buying}/{selling}")
            }
        }
        "manage_buy_offer" => {
            let offer_id = text(operation, "offer_id").unwrap_or("0");
            let amount = amount(operation, "amount");
            if amount == "0" {
                return format!("Cancelled offer #{offer_id}");
            }
            let selling = operation_asset(operation, "selling_");
            let buying = operation_asset(operation, "buying_");
            let price = amount(operation, "price");
            let verb = if offer_id == "0" {
                "Placed".to_owned()
            } else {
                format!("Updated #{offer_id}")
            };
            format!("{verb} BUY {amount} {buying} @ {price} {selling}/{buying}")
        }
        "change_trust" => {
            let asset = if text(operation, "asset_type") == Some("liquidity_pool_shares") {
                let pool = text(operation, "liquidity_pool_id").unwrap_or("unknown");
                format!("liquidity pool {}", short_id(pool))
            } else {
                operation_asset(operation, "")
            };
            let limit = amount(operation, "limit");
            if limit == "0" {
                format!("Removed trustline for {asset}")
            } else {
                format!("Set trustline for {asset} · limit {limit}")
            }
        }
        "invoke_host_function" => "Contract call".to_owned(),
        "liquidity_pool_deposit" => "Added liquidity".to_owned(),
        "liquidity_pool_withdraw" => "Removed liquidity".to_owned(),
        "account_merge" => format!(
            "Merged account into {}",
            short_address(
                text(operation, "into")
                    .or_else(|| text(operation, "account"))
                    .unwrap_or("?")
            )
        ),
        "manage_data" => format!(
            "Updated account data: {}",
            text(operation, "name").unwrap_or("data entry")
        ),
        "set_options" => "Updated account settings".to_owned(),
        "bump_sequence" => format!(
            "Bumped sequence to {}",
            text(operation, "bump_to").unwrap_or("?")
        ),
        other => other.replace('_', " "),
    }
}

fn operation_asset(operation: &Value, prefix: &str) -> String {
    let asset_type = text(operation, &format!("{prefix}asset_type"));
    if asset_type == Some("native") {
        return "XLM".to_owned();
    }
    let code = text(operation, &format!("{prefix}asset_code")).unwrap_or("asset");
    match text(operation, &format!("{prefix}asset_issuer")) {
        Some(issuer) => format!("{code}:{}", short_address(issuer)),
        None => code.to_owned(),
    }
}

fn amount(value: &Value, key: &str) -> String {
    text(value, key).map(clean_decimal).unwrap_or_else(|| "?".to_owned())
}

fn clean_decimal(value: &str) -> String {
    if !value.contains('.') {
        return value.to_owned();
    }
    let trimmed = value.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn short_address(value: &str) -> String {
    if value.len() <= 16 {
        return value.to_owned();
    }
    format!("{}...{}", &value[..6], &value[value.len() - 6..])
}

fn short_id(value: &str) -> String {
    if value.len() <= 12 {
        value.to_owned()
    } else {
        format!("{}...", &value[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn mock_server(expected_target: &'static str, status: u16, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(
                request.starts_with(&format!("GET {expected_target} HTTP/1.1")),
                "unexpected request: {request}"
            );
            let reason = if status == 200 { "OK" } else { "Not Found" };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        format!("http://{address}")
    }

    #[test]
    fn account_and_balance_helpers_use_horizon_shape() {
        let body = r#"{"account_id":"GTEST","balances":[{"asset_type":"native","balance":"12.5000000"},{"asset_type":"credit_alphanum4","asset_code":"USDC","asset_issuer":"GISSUER","balance":"7.0000000"}]}"#;
        let base = mock_server("/accounts/GTEST", 200, body);
        let account = HorizonClient::new(&base).get_account("GTEST").unwrap();
        let balances = account["balances"].as_array().unwrap();
        assert_eq!(balance_asset_label(&balances[0]), "XLM");
        assert_eq!(balance_asset_label(&balances[1]), "USDC:GISSUER");
    }

    #[test]
    fn operations_are_requested_newest_first_with_limit() {
        let body = r#"{"_embedded":{"records":[{"type":"payment","amount":"1.0000000","asset_type":"native","from":"GSOURCE","to":"GACCOUNT"}]}}"#;
        let base = mock_server(
            "/accounts/GACCOUNT/operations?order=desc&limit=2",
            200,
            body,
        );
        let operations = HorizonClient::new(&base)
            .get_operations("GACCOUNT", 2)
            .unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(
            operation_summary(&operations[0], "GACCOUNT"),
            "Received 1 XLM from GSOURCE"
        );
    }

    #[test]
    fn account_404_is_distinct_from_transport_failure() {
        let base = mock_server("/accounts/GNOTFOUND", 404, r#"{}"#);
        assert_eq!(
            HorizonClient::new(&base).get_account("GNOTFOUND").unwrap_err(),
            "Stellar account not found: GNOTFOUND"
        );
    }
}
