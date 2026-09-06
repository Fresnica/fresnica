use std::time::Duration;

use serde_json::Value;

use crate::storage::WalletRecord;

const TESTNET_FRIENDBOT_URL: &str = "https://friendbot.stellar.org";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestnetFunding {
    pub transaction_hash: Option<String>,
}

pub fn fund_testnet_wallet(network: &str, wallet: &WalletRecord) -> Result<TestnetFunding, String> {
    fund_testnet_wallet_with_url(network, wallet, TESTNET_FRIENDBOT_URL)
}

fn fund_testnet_wallet_with_url(
    network: &str,
    wallet: &WalletRecord,
    friendbot_url: &str,
) -> Result<TestnetFunding, String> {
    if network != "testnet" {
        return Err("Friendbot is only available on testnet".to_owned());
    }
    if wallet.network != network {
        return Err(format!(
            "wallet \"{}\" is configured for {}; invoke with --network {}",
            wallet.name, wallet.network, wallet.network
        ));
    }

    let text = FriendbotClient::new(friendbot_url).fund(&wallet.address)?;
    let transaction_hash = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|value| value.get("hash").and_then(Value::as_str).map(str::to_owned));
    Ok(TestnetFunding { transaction_hash })
}

struct FriendbotClient {
    base_url: String,
}

impl FriendbotClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    fn fund(&self, address: &str) -> Result<String, String> {
        let url = format!("{}?addr={address}", self.base_url);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        let mut response = agent
            .get(&url)
            .call()
            .map_err(|error| format!("Unable to fund testnet account: {error}"))?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|error| format!("Unable to fund testnet account: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn wallet(network: &str) -> WalletRecord {
        WalletRecord {
            name: "alpha".to_owned(),
            address: "GACCOUNT".to_owned(),
            wallet_type: "watch-only".to_owned(),
            network: network.to_owned(),
            secret: None,
            metadata: serde_json::Map::new(),
        }
    }

    fn mock_friendbot(status: u16, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.starts_with("GET /?addr=GACCOUNT HTTP/1.1"));
            let reason = if status == 200 { "OK" } else { "Bad Request" };
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
    fn funding_normalizes_transaction_hash() {
        let base = mock_friendbot(200, r#"{"hash":"abc123"}"#);
        let result = fund_testnet_wallet_with_url("testnet", &wallet("testnet"), &base).unwrap();
        assert_eq!(result.transaction_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn funding_accepts_non_json_success_without_provider_shape() {
        let base = mock_friendbot(200, "funded");
        let result = fund_testnet_wallet_with_url("testnet", &wallet("testnet"), &base).unwrap();
        assert_eq!(result.transaction_hash, None);
    }

    #[test]
    fn funding_rejects_non_testnet_network() {
        assert_eq!(
            fund_testnet_wallet_with_url("mainnet", &wallet("mainnet"), "http://unused")
                .unwrap_err(),
            "Friendbot is only available on testnet"
        );
    }

    #[test]
    fn funding_rejects_wallet_network_mismatch() {
        assert_eq!(
            fund_testnet_wallet_with_url("testnet", &wallet("mainnet"), "http://unused")
                .unwrap_err(),
            "wallet \"alpha\" is configured for mainnet; invoke with --network mainnet"
        );
    }

    #[test]
    fn funding_preserves_transport_failure_context() {
        let base = mock_friendbot(400, r#"{"detail":"bad account"}"#);
        let error = fund_testnet_wallet_with_url("testnet", &wallet("testnet"), &base).unwrap_err();
        assert!(error.starts_with("Unable to fund testnet account: "));
    }
}
