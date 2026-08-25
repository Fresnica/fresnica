use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value as JsonValue;
use url::Url;

use crate::anchor_auth::verify_sep10_challenge;

const MAX_AUTH_RESPONSE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sep10Session {
    token: String,
}

impl Sep10Session {
    pub fn bearer_token(&self) -> &str {
        &self.token
    }
}

pub fn acquire_sep10_session<FGet, FPost, FSign>(
    web_auth_endpoint: &str,
    server_signing_key: &str,
    client_account: &str,
    home_domain: &str,
    network_passphrase: &str,
    now_unix: u64,
    mut get_json: FGet,
    mut post_json: FPost,
    mut sign_challenge: FSign,
) -> Result<Sep10Session, String>
where
    FGet: FnMut(&str) -> Result<JsonValue, String>,
    FPost: FnMut(&str, &JsonValue) -> Result<JsonValue, String>,
    FSign: FnMut(&[u8]) -> Result<Vec<u8>, String>,
{
    let endpoint = validated_auth_endpoint(web_auth_endpoint)?;
    let web_auth_domain = endpoint
        .host_str()
        .ok_or_else(|| "SEP-10 WEB_AUTH_ENDPOINT has no host".to_owned())?;
    let challenge_url = challenge_url(&endpoint, client_account, home_domain);
    let response = get_json(challenge_url.as_str())?;

    if let Some(value) = response.get("network_passphrase") {
        let advertised = value
            .as_str()
            .ok_or_else(|| "SEP-10 network_passphrase must be a string".to_owned())?;
        if advertised != network_passphrase {
            return Err("SEP-10 server network_passphrase does not match the selected network".to_owned());
        }
    }

    let challenge_b64 = response
        .get("transaction")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SEP-10 challenge response has no transaction".to_owned())?;
    let challenge_xdr = STANDARD
        .decode(challenge_b64)
        .map_err(|_| "SEP-10 challenge transaction is not valid base64".to_owned())?;

    verify_sep10_challenge(
        &challenge_xdr,
        server_signing_key,
        client_account,
        home_domain,
        web_auth_domain,
        network_passphrase,
        now_unix,
    )?;

    let signed_xdr = sign_challenge(&challenge_xdr)?;
    if signed_xdr.is_empty() {
        return Err("SEP-10 signer returned an empty transaction".to_owned());
    }
    let request = serde_json::json!({
        "transaction": STANDARD.encode(signed_xdr),
    });
    let response = post_json(endpoint.as_str(), &request)?;
    let token = response
        .get("token")
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SEP-10 authentication response has no token".to_owned())?;

    Ok(Sep10Session {
        token: token.to_owned(),
    })
}

pub fn acquire_sep10_session_http<FSign>(
    web_auth_endpoint: &str,
    server_signing_key: &str,
    client_account: &str,
    home_domain: &str,
    network_passphrase: &str,
    now_unix: u64,
    sign_challenge: FSign,
) -> Result<Sep10Session, String>
where
    FSign: FnMut(&[u8]) -> Result<Vec<u8>, String>,
{
    acquire_sep10_session(
        web_auth_endpoint,
        server_signing_key,
        client_account,
        home_domain,
        network_passphrase,
        now_unix,
        fetch_json,
        post_json,
        sign_challenge,
    )
}

fn validated_auth_endpoint(value: &str) -> Result<Url, String> {
    let endpoint = Url::parse(value).map_err(|_| "SEP-10 WEB_AUTH_ENDPOINT is invalid".to_owned())?;
    if endpoint.scheme() != "https"
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        return Err(
            "SEP-10 WEB_AUTH_ENDPOINT must be HTTPS without embedded credentials".to_owned(),
        );
    }
    Ok(endpoint)
}

fn challenge_url(endpoint: &Url, client_account: &str, home_domain: &str) -> Url {
    let mut url = endpoint.clone();
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("account", client_account);
        pairs.append_pair("home_domain", home_domain);
    }
    url
}

fn fetch_json(url: &str) -> Result<JsonValue, String> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|error| format!("Unable to load SEP-10 challenge from {url}: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_AUTH_RESPONSE_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| format!("Invalid SEP-10 JSON from {url}: {error}"))
}

fn post_json(url: &str, payload: &JsonValue) -> Result<JsonValue, String> {
    let mut response = ureq::post(url)
        .send_json(payload)
        .map_err(|error| format!("Unable to submit SEP-10 challenge to {url}: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_AUTH_RESPONSE_BYTES)
        .read_json::<JsonValue>()
        .map_err(|error| format!("Invalid SEP-10 JSON from {url}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const TESTNET: &str = "Test SDF Network ; September 2015";
    const SERVER: &str = "GDEISG5WA25KU6HHB7N4HVQKID4A7FDDR3FKD32R6C7KCV7YLYKVY7S7";
    const CLIENT: &str = "GBAQD4VYNI2255CFRDNDM4LVAEITMCNS7HJCI7I46XJE756ITCJXLV7E";
    const HOME_DOMAIN: &str = "thisisatest.sandbox.anchor.anchordomain.com";
    const ENDPOINT: &str = "https://auth.example.com/auth";
    const CHALLENGE: &str = concat!(
        "AAAAAgAAAADIiRu2BrqqeOcP28PWCkD4D5Rjjsqh71HwvqFX+F4VXAAAAGQAAAAAAAAA",
        "AAAAAAEAAAAAXzrUcQAAAABfOtf1AAAAAAAAAAEAAAABAAAAAEEB8rhqNa70RYjaNnF1",
        "ARE2CbL50iR9HPXST/fImJN1AAAACgAAADB0aGlzaXNhdGVzdC5zYW5kYm94LmFuY2hv",
        "ci5hbmNob3Jkb21haW4uY29tIGF1dGgAAAABAAAAQGdGOFlIQm1zaGpEWEY0L0VJUFZu",
        "cGVlRkxVTDY2V0tKMVBPYXZuUVVBNjBoL09XaC91M2Vvdk54WFJtSTAvQ2UAAAAAAAAA",
        "AfheFVwAAABAheKE1HjGnUCNwPbX8mz7CqotShKbA+xM2Hbjl6X0TBpEprVOUVjA6lqM",
        "J1j62vrxn1mF3eJzsLa9s9hRofG3Ag=="
    );

    #[test]
    fn acquires_token_only_after_verified_challenge_is_signed() {
        let signed = Cell::new(0_u32);
        let posted = Cell::new(0_u32);
        let session = acquire_sep10_session(
            ENDPOINT,
            SERVER,
            CLIENT,
            HOME_DOMAIN,
            TESTNET,
            1_597_691_000,
            |url| {
                let parsed = Url::parse(url).unwrap();
                assert_eq!(parsed.scheme(), "https");
                assert_eq!(parsed.host_str(), Some("auth.example.com"));
                let params: std::collections::HashMap<_, _> =
                    parsed.query_pairs().into_owned().collect();
                assert_eq!(params.get("account").map(String::as_str), Some(CLIENT));
                assert_eq!(
                    params.get("home_domain").map(String::as_str),
                    Some(HOME_DOMAIN)
                );
                Ok(serde_json::json!({
                    "transaction": CHALLENGE,
                    "network_passphrase": TESTNET,
                }))
            },
            |url, payload| {
                posted.set(posted.get() + 1);
                assert_eq!(url, ENDPOINT);
                assert!(payload["transaction"].as_str().unwrap().len() > 100);
                Ok(serde_json::json!({"token": "jwt-value"}))
            },
            |challenge| {
                signed.set(signed.get() + 1);
                Ok(challenge.to_vec())
            },
        )
        .unwrap();

        assert_eq!(session.bearer_token(), "jwt-value");
        assert_eq!(signed.get(), 1);
        assert_eq!(posted.get(), 1);
    }

    #[test]
    fn network_mismatch_never_invokes_signer_or_post() {
        let signed = Cell::new(false);
        let posted = Cell::new(false);
        let result = acquire_sep10_session(
            ENDPOINT,
            SERVER,
            CLIENT,
            HOME_DOMAIN,
            TESTNET,
            1_597_691_000,
            |_| {
                Ok(serde_json::json!({
                    "transaction": CHALLENGE,
                    "network_passphrase": "Public Global Stellar Network ; September 2015",
                }))
            },
            |_, _| {
                posted.set(true);
                Ok(serde_json::json!({"token": "should-not-exist"}))
            },
            |_| {
                signed.set(true);
                Ok(Vec::new())
            },
        );
        assert!(result.is_err());
        assert!(!signed.get());
        assert!(!posted.get());
    }

    #[test]
    fn invalid_server_signature_never_invokes_signer_or_post() {
        let signed = Cell::new(false);
        let posted = Cell::new(false);
        let result = acquire_sep10_session(
            ENDPOINT,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            CLIENT,
            HOME_DOMAIN,
            TESTNET,
            1_597_691_000,
            |_| Ok(serde_json::json!({"transaction": CHALLENGE})),
            |_, _| {
                posted.set(true);
                Ok(serde_json::json!({"token": "should-not-exist"}))
            },
            |_| {
                signed.set(true);
                Ok(Vec::new())
            },
        );
        assert!(result.is_err());
        assert!(!signed.get());
        assert!(!posted.get());
    }

    #[test]
    fn rejects_insecure_auth_endpoint_before_network_or_signing() {
        let result = acquire_sep10_session(
            "http://auth.example.com/auth",
            SERVER,
            CLIENT,
            HOME_DOMAIN,
            TESTNET,
            1_597_691_000,
            |_| panic!("GET must not run"),
            |_, _| panic!("POST must not run"),
            |_| panic!("signer must not run"),
        );
        assert!(result.is_err());
    }
}
