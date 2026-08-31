//! Transitional Horizon-backed implementation of Fresnica Network / Gateway.
//! Fresnica does not expose or maintain a general Horizon SDK; provider-specific
//! data is isolated here so endpoint families can migrate to RPC/Portfolio.

use serde_json::Value;

pub const MAINNET_HORIZON_URL: &str = "https://horizon.stellar.org";
pub const TESTNET_HORIZON_URL: &str = "https://horizon-testnet.stellar.org";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerParameters {
    pub base_fee_in_stroops: u32,
    pub base_reserve_in_stroops: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionError {
    Rejected(String),
    Uncertain(String),
}

/// Current Horizon-backed Network / Gateway adapter.
pub struct HorizonGateway {
    base_url: String,
}

impl HorizonGateway {
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

    pub fn get_account_optional(&self, address: &str) -> Result<Option<Value>, String> {
        let url = format!("{}/accounts/{address}", self.base_url);
        let mut response = match ureq::get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(ureq::Error::StatusCode(code)) => {
                return Err(format!("Horizon returned HTTP {code} for {url}"))
            }
            Err(error) => return Err(format!("Unable to contact Horizon at {url}: {error}")),
        };
        response
            .body_mut()
            .read_json::<Value>()
            .map(Some)
            .map_err(|error| format!("Horizon returned invalid JSON for {url}: {error}"))
    }

    pub fn get_liquidity_pool(&self, liquidity_pool_id: &str) -> Result<Value, String> {
        self.get_json(
            &format!("/liquidity_pools/{liquidity_pool_id}"),
            &format!("Liquidity pool not found: {liquidity_pool_id}"),
        )
    }

    pub fn account_exists(&self, address: &str) -> Result<bool, String> {
        let url = format!("{}/accounts/{address}", self.base_url);
        match ureq::get(&url).call() {
            Ok(_) => Ok(true),
            Err(ureq::Error::StatusCode(404)) => Ok(false),
            Err(ureq::Error::StatusCode(code)) => {
                Err(format!("Horizon returned HTTP {code} for {url}"))
            }
            Err(error) => Err(format!("Unable to contact Horizon at {url}: {error}")),
        }
    }

    pub fn get_offer(&self, offer_id: i64) -> Result<Value, String> {
        if offer_id <= 0 {
            return Err("offer id must be a positive integer".to_owned());
        }
        self.get_json(
            &format!("/offers/{offer_id}"),
            &format!("Offer not found: {offer_id}"),
        )
    }

    pub fn get_offers(&self, address: &str, limit: usize) -> Result<Vec<Value>, String> {
        if !(1..=200).contains(&limit) {
            return Err("offers limit must be from 1 to 200".to_owned());
        }
        let response = self.get_json(
            &format!("/accounts/{address}/offers?order=desc&limit={limit}"),
            &format!("Stellar account not found: {address}"),
        )?;
        response
            .get("_embedded")
            .and_then(|value| value.get("records"))
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "Horizon returned malformed offer data".to_owned())
    }

    pub fn get_order_book(&self, selling_query: &str, buying_query: &str) -> Result<Value, String> {
        self.get_json(
            &format!("/order_book?{selling_query}&{buying_query}"),
            "Unable to load Stellar order book",
        )
    }

    pub fn get_trades(
        &self,
        base_query: &str,
        counter_query: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        if !(1..=200).contains(&limit) {
            return Err("trades limit must be from 1 to 200".to_owned());
        }
        let response = self.get_json(
            &format!("/trades?{base_query}&{counter_query}&order=desc&limit={limit}"),
            "Unable to load Stellar trades",
        )?;
        records(response, "trade")
    }

    pub fn get_account_trades(&self, address: &str, limit: usize) -> Result<Vec<Value>, String> {
        if !(1..=200).contains(&limit) {
            return Err("account trades limit must be from 1 to 200".to_owned());
        }
        let response = self.get_json(
            &format!("/accounts/{address}/trades?order=desc&limit={limit}"),
            &format!("Stellar account not found: {address}"),
        )?;
        records(response, "account trade")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_trade_aggregations(
        &self,
        base_query: &str,
        counter_query: &str,
        resolution: u64,
        start_time: Option<u64>,
        end_time: Option<u64>,
        offset: Option<u64>,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        if !(1..=200).contains(&limit) {
            return Err("trade aggregation limit must be from 1 to 200".to_owned());
        }
        let mut path =
            format!("/trade_aggregations?{base_query}&{counter_query}&resolution={resolution}");
        if let Some(value) = start_time {
            path.push_str(&format!("&start_time={value}"));
        }
        if let Some(value) = end_time {
            path.push_str(&format!("&end_time={value}"));
        }
        if let Some(value) = offset {
            path.push_str(&format!("&offset={value}"));
        }
        path.push_str(&format!("&order=desc&limit={limit}"));
        let response = self.get_json(&path, "Unable to load Stellar trade aggregations")?;
        records(response, "trade aggregation")
    }

    pub fn get_transaction(&self, transaction_hash: &str) -> Result<Option<Value>, String> {
        let url = format!("{}/transactions/{transaction_hash}", self.base_url);
        let mut response = match ureq::get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(ureq::Error::StatusCode(code)) => {
                return Err(format!("Horizon returned HTTP {code} for {url}"))
            }
            Err(error) => return Err(format!("Unable to contact Horizon at {url}: {error}")),
        };
        response
            .body_mut()
            .read_json::<Value>()
            .map(Some)
            .map_err(|error| format!("Horizon returned invalid JSON for {url}: {error}"))
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

    pub fn get_ledger_parameters(&self) -> Result<LedgerParameters, String> {
        let response = self.get_json(
            "/ledgers?order=desc&limit=1",
            "Horizon returned no ledger data",
        )?;
        let ledger = response
            .get("_embedded")
            .and_then(|value| value.get("records"))
            .and_then(Value::as_array)
            .and_then(|records| records.first())
            .ok_or_else(|| "Horizon returned no ledger data".to_owned())?;
        let base_fee = integer(ledger.get("base_fee_in_stroops"))
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "Horizon returned invalid base fee".to_owned())?;
        let base_reserve = integer(ledger.get("base_reserve_in_stroops"))
            .ok_or_else(|| "Horizon returned invalid base reserve".to_owned())?;
        if base_reserve < 0 {
            return Err("Horizon returned invalid base reserve".to_owned());
        }
        Ok(LedgerParameters {
            base_fee_in_stroops: base_fee,
            base_reserve_in_stroops: base_reserve,
        })
    }

    pub fn submit_transaction(&self, transaction_xdr: &str) -> Result<Value, SubmissionError> {
        let url = format!("{}/transactions", self.base_url);
        let mut response = match ureq::post(&url).send_form([("tx", transaction_xdr)]) {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(code)) if code < 500 => {
                return Err(SubmissionError::Rejected(format!(
                    "Horizon rejected the transaction with HTTP {code}"
                )))
            }
            Err(ureq::Error::StatusCode(code)) => {
                return Err(SubmissionError::Uncertain(format!(
                    "Horizon returned HTTP {code} while submitting"
                )))
            }
            Err(error) => {
                return Err(SubmissionError::Uncertain(format!(
                    "Unable to contact Horizon while submitting: {error}"
                )))
            }
        };
        response.body_mut().read_json::<Value>().map_err(|error| {
            SubmissionError::Uncertain(format!("Horizon returned invalid submission JSON: {error}"))
        })
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

fn records(value: Value, label: &str) -> Result<Vec<Value>, String> {
    value
        .get("_embedded")
        .and_then(|value| value.get("records"))
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| format!("Horizon returned malformed {label} data"))
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
            let offer_amount = amount(operation, "amount");
            if offer_amount == "0" {
                return format!("Cancelled offer #{offer_id}");
            }
            let selling = operation_asset(operation, "selling_");
            let buying = operation_asset(operation, "buying_");
            let price = amount(operation, "price");
            if text(operation, "type") == Some("create_passive_sell_offer") {
                format!("Placed passive SELL {offer_amount} {selling} @ {price} {buying}/{selling}")
            } else {
                let verb = if offer_id == "0" {
                    "Placed".to_owned()
                } else {
                    format!("Updated #{offer_id}")
                };
                format!("{verb} SELL {offer_amount} {selling} @ {price} {buying}/{selling}")
            }
        }
        "manage_buy_offer" => {
            let offer_id = text(operation, "offer_id").unwrap_or("0");
            let offer_amount = amount(operation, "amount");
            if offer_amount == "0" {
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
            format!("{verb} BUY {offer_amount} {buying} @ {price} {selling}/{buying}")
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
    text(value, key)
        .map(clean_decimal)
        .unwrap_or_else(|| "?".to_owned())
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

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(value) => value.as_i64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
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
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn mock_server(
        expected_method: &'static str,
        expected_target: &'static str,
        status: u16,
        body: &'static str,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request_line = {
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).unwrap();

                let mut content_length = 0usize;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            content_length = value.trim().parse().unwrap();
                        }
                    }
                }

                let mut body = vec![0u8; content_length];
                reader.read_exact(&mut body).unwrap();
                request_line
            };
            assert!(
                request_line.starts_with(&format!("{expected_method} {expected_target} HTTP/1.1")),
                "unexpected request: {request_line}"
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
        let base = mock_server("GET", "/accounts/GTEST", 200, body);
        let account = HorizonGateway::new(&base).get_account("GTEST").unwrap();
        let balances = account["balances"].as_array().unwrap();
        assert_eq!(balance_asset_label(&balances[0]), "XLM");
        assert_eq!(balance_asset_label(&balances[1]), "USDC:GISSUER");
    }

    #[test]
    fn operations_are_requested_newest_first_with_limit() {
        let body = r#"{"_embedded":{"records":[{"type":"payment","amount":"1.0000000","asset_type":"native","from":"GSOURCE","to":"GACCOUNT"}]}}"#;
        let base = mock_server(
            "GET",
            "/accounts/GACCOUNT/operations?order=desc&limit=2",
            200,
            body,
        );
        let operations = HorizonGateway::new(&base)
            .get_operations("GACCOUNT", 2)
            .unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(
            operation_summary(&operations[0], "GACCOUNT"),
            "Received 1 XLM from GSOURCE"
        );
    }

    #[test]
    fn offer_lookup_uses_offer_endpoint_and_404_message() {
        let body = r#"{"id":"42","seller":"GSELLER"}"#;
        let base = mock_server("GET", "/offers/42", 200, body);
        let offer = HorizonGateway::new(&base).get_offer(42).unwrap();
        assert_eq!(offer["id"], "42");

        let base = mock_server("GET", "/offers/43", 404, r#"{}"#);
        assert_eq!(
            HorizonGateway::new(&base).get_offer(43).unwrap_err(),
            "Offer not found: 43"
        );
    }

    #[test]
    fn offers_are_requested_newest_first_with_limit() {
        let body = r#"{"_embedded":{"records":[{"id":"42","seller":"GACCOUNT"}]}}"#;
        let base = mock_server(
            "GET",
            "/accounts/GACCOUNT/offers?order=desc&limit=20",
            200,
            body,
        );
        let offers = HorizonGateway::new(&base)
            .get_offers("GACCOUNT", 20)
            .unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0]["id"], "42");
    }

    #[test]
    fn order_book_uses_explicit_selling_and_buying_asset_queries() {
        let body = r#"{"bids":[],"asks":[]}"#;
        let base = mock_server(
            "GET",
            "/order_book?selling_asset_type=native&buying_asset_type=credit_alphanum4&buying_asset_code=USD&buying_asset_issuer=GISSUER",
            200,
            body,
        );
        let order_book = HorizonGateway::new(&base)
            .get_order_book(
                "selling_asset_type=native",
                "buying_asset_type=credit_alphanum4&buying_asset_code=USD&buying_asset_issuer=GISSUER",
            )
            .unwrap();
        assert_eq!(order_book["bids"], serde_json::json!([]));
        assert_eq!(order_book["asks"], serde_json::json!([]));
    }

    #[test]
    fn trades_are_requested_newest_first_for_explicit_pair() {
        let body = r#"{"_embedded":{"records":[{"id":"7"}]}}"#;
        let base = mock_server(
            "GET",
            "/trades?base_asset_type=native&counter_asset_type=credit_alphanum4&counter_asset_code=USD&counter_asset_issuer=GISSUER&order=desc&limit=20",
            200,
            body,
        );
        let trades = HorizonGateway::new(&base)
            .get_trades(
                "base_asset_type=native",
                "counter_asset_type=credit_alphanum4&counter_asset_code=USD&counter_asset_issuer=GISSUER",
                20,
            )
            .unwrap();
        assert_eq!(trades[0]["id"], "7");
    }

    #[test]
    fn account_trades_are_requested_newest_first() {
        let body = r#"{"_embedded":{"records":[{"id":"8"}]}}"#;
        let base = mock_server(
            "GET",
            "/accounts/GACCOUNT/trades?order=desc&limit=200",
            200,
            body,
        );
        let trades = HorizonGateway::new(&base)
            .get_account_trades("GACCOUNT", 200)
            .unwrap();
        assert_eq!(trades[0]["id"], "8");
    }

    #[test]
    fn trade_aggregations_preserve_time_window_offset_and_limit() {
        let body = r#"{"_embedded":{"records":[{"timestamp":1}]}}"#;
        let base = mock_server(
            "GET",
            "/trade_aggregations?base_asset_type=native&counter_asset_type=credit_alphanum4&counter_asset_code=USD&counter_asset_issuer=GISSUER&resolution=3600000&start_time=1000&end_time=2000&offset=0&order=desc&limit=10",
            200,
            body,
        );
        let candles = HorizonGateway::new(&base)
            .get_trade_aggregations(
                "base_asset_type=native",
                "counter_asset_type=credit_alphanum4&counter_asset_code=USD&counter_asset_issuer=GISSUER",
                3_600_000,
                Some(1000),
                Some(2000),
                Some(0),
                10,
            )
            .unwrap();
        assert_eq!(candles[0]["timestamp"], 1);
    }

    #[test]
    fn account_404_is_distinct_from_transport_failure() {
        let base = mock_server("GET", "/accounts/GNOTFOUND", 404, r#"{}"#);
        assert_eq!(
            HorizonGateway::new(&base)
                .get_account("GNOTFOUND")
                .unwrap_err(),
            "Stellar account not found: GNOTFOUND"
        );
    }

    #[test]
    fn optional_account_maps_404_to_none() {
        let base = mock_server("GET", "/accounts/GNEW", 404, r#"{}"#);
        assert_eq!(
            HorizonGateway::new(&base)
                .get_account_optional("GNEW")
                .unwrap(),
            None
        );
    }

    #[test]
    fn optional_account_returns_existing_account_json() {
        let body = r#"{"account_id":"GACCOUNT","thresholds":{"med_threshold":0}}"#;
        let base = mock_server("GET", "/accounts/GACCOUNT", 200, body);
        let account = HorizonGateway::new(&base)
            .get_account_optional("GACCOUNT")
            .unwrap()
            .unwrap();
        assert_eq!(account["account_id"], "GACCOUNT");
        assert_eq!(account["thresholds"]["med_threshold"], 0);
    }

    #[test]
    fn account_exists_maps_404_to_false() {
        let base = mock_server("GET", "/accounts/GNEW", 404, r#"{}"#);
        assert!(!HorizonGateway::new(&base).account_exists("GNEW").unwrap());
    }

    #[test]
    fn ledger_parameters_match_horizon_fields() {
        let body = r#"{"_embedded":{"records":[{"base_fee_in_stroops":100,"base_reserve_in_stroops":5000000}]}}"#;
        let base = mock_server("GET", "/ledgers?order=desc&limit=1", 200, body);
        assert_eq!(
            HorizonGateway::new(&base).get_ledger_parameters().unwrap(),
            LedgerParameters {
                base_fee_in_stroops: 100,
                base_reserve_in_stroops: 5_000_000,
            }
        );
    }

    #[test]
    fn submission_uses_horizon_transaction_endpoint() {
        let body = r#"{"hash":"abc","ledger":7,"successful":true}"#;
        let base = mock_server("POST", "/transactions", 200, body);
        let result = HorizonGateway::new(&base)
            .submit_transaction("AAAA")
            .unwrap();
        assert_eq!(result["hash"], "abc");
    }
}
