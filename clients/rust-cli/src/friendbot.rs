use std::time::Duration;

use serde_json::Value;

use fresnica_client::WalletStorage;

pub const FRIENDBOT_URL: &str = "https://friendbot.stellar.org";

pub fn command_fund(
    storage: &WalletStorage,
    network: &str,
    arguments: &[String],
) -> Result<(), String> {
    if network != "testnet" {
        return Err("Friendbot is only available on testnet".to_owned());
    }
    let wallet = parse_wallet_option(arguments)?;
    let record = storage.resolve(wallet.as_deref())?;
    if record.network != network {
        return Err(format!(
            "wallet \"{}\" is configured for {}; invoke with --network {}",
            record.name, record.network, record.network
        ));
    }

    let result = FriendbotClient::new(FRIENDBOT_URL).fund(&record.address)?;
    let mut message = format!("Funded wallet \"{}\" on testnet", record.name);
    if let Some(hash) = result.get("hash").and_then(Value::as_str) {
        message.push_str("; transaction ");
        message.push_str(hash);
    }
    println!("{message}");
    Ok(())
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

    fn fund(&self, address: &str) -> Result<Value, String> {
        let url = format!("{}?addr={address}", self.base_url);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        let mut response = agent
            .get(&url)
            .call()
            .map_err(|_| "Unable to fund testnet account".to_owned())?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|_| "Unable to fund testnet account".to_owned())?;
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({"status": text})))
    }
}

fn parse_wallet_option(arguments: &[String]) -> Result<Option<String>, String> {
    match arguments {
        [] => Ok(None),
        [flag, name] if flag == "--wallet" && !name.trim().is_empty() => Ok(Some(name.clone())),
        _ => Err("usage: fresnica wallet testnet-fund [--wallet NAME]".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

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
    fn parses_friendbot_json_response() {
        let base = mock_friendbot(200, r#"{"hash":"abc123"}"#);
        let result = FriendbotClient::new(&base).fund("GACCOUNT").unwrap();
        assert_eq!(result["hash"], "abc123");
    }

    #[test]
    fn wraps_non_json_success_response() {
        let base = mock_friendbot(200, "funded");
        let result = FriendbotClient::new(&base).fund("GACCOUNT").unwrap();
        assert_eq!(result["status"], "funded");
    }

    #[test]
    fn rejects_unsuccessful_friendbot_response() {
        let base = mock_friendbot(400, r#"{"detail":"bad account"}"#);
        assert_eq!(
            FriendbotClient::new(&base).fund("GACCOUNT").unwrap_err(),
            "Unable to fund testnet account"
        );
    }

    #[test]
    fn wallet_option_matches_python_cli_shape() {
        assert_eq!(parse_wallet_option(&[]).unwrap(), None);
        assert_eq!(
            parse_wallet_option(&["--wallet".to_owned(), "alpha".to_owned()]).unwrap(),
            Some("alpha".to_owned())
        );
    }
}
