use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
use ureq::{Agent, Body};
use url::{Host, Url};
use zeroize::Zeroizing;

pub(crate) const MAX_ANCHOR_RESPONSE_BYTES: u64 = 1_000_000;

const ANCHOR_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const ANCHOR_GLOBAL_TIMEOUT: Duration = Duration::from_secs(60);

pub(crate) fn agent() -> &'static Agent {
    static AGENT: OnceLock<Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        Agent::new_with_config(
            Agent::config_builder()
                .https_only(true)
                .max_redirects(0)
                .timeout_connect(Some(ANCHOR_CONNECT_TIMEOUT))
                .timeout_global(Some(ANCHOR_GLOBAL_TIMEOUT))
                .build(),
        )
    })
}

pub(crate) fn canonical_home_domain(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('.');
    let url = Url::parse(&format!("https://{value}/"))
        .map_err(|_| "Issuer home_domain is not a valid host name".to_owned())?;
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err("Issuer home_domain is not a valid host name".to_owned());
    }
    validate_anchor_https_url(&url, "Issuer home_domain")?;
    if url.port().is_some() {
        return Err("Issuer home_domain must not include a port".to_owned());
    }
    Ok(url
        .host_str()
        .expect("validated URL has a host")
        .trim_end_matches('.')
        .to_ascii_lowercase())
}

pub(crate) fn validate_anchor_https_url(url: &Url, label: &str) -> Result<(), String> {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(format!(
            "{label} must be an HTTPS URL without embedded credentials"
        ));
    }
    let host = match url.host() {
        Some(Host::Domain(host)) => host.trim_end_matches('.'),
        Some(Host::Ipv4(_)) | Some(Host::Ipv6(_)) => {
            return Err(format!(
                "{label} must use a DNS host name, not an IP address"
            ));
        }
        None => return Err(format!("{label} must include a host")),
    };
    if !is_external_dns_name(host) {
        return Err(format!("{label} must use an external DNS host name"));
    }
    Ok(())
}

pub(crate) fn endpoint_label(url: &Url) -> String {
    let mut sanitized = url.clone();
    sanitized.set_query(None);
    sanitized.set_fragment(None);
    sanitized.to_string()
}

pub(crate) fn reject_anchor_redirect(status: u16, endpoint: &str) -> Result<(), String> {
    if (300..400).contains(&status) {
        return Err(format!(
            "Anchor endpoint {endpoint} returned HTTP {status}; redirects are not allowed"
        ));
    }
    Ok(())
}

pub(crate) fn get_json(
    url: &Url,
    protocol: &str,
    token: Option<&str>,
) -> Result<(u16, Value), String> {
    validate_anchor_https_url(url, protocol)?;
    let authorization = token.map(|token| Zeroizing::new(format!("Bearer {token}")));
    let mut request = agent().get(url.as_str());
    if let Some(authorization) = authorization.as_ref() {
        request = request.header("Authorization", authorization.as_str());
    }
    let response = request
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|error| {
            format!(
                "Unable to call {protocol} endpoint {}: {error}",
                endpoint_label(url)
            )
        })?;
    read_json_response(protocol, url, response)
}

pub(crate) fn put_json(
    url: &Url,
    protocol: &str,
    token: &str,
    body: Value,
) -> Result<(u16, Value), String> {
    validate_anchor_https_url(url, protocol)?;
    let authorization = Zeroizing::new(format!("Bearer {token}"));
    let response = agent()
        .put(url.as_str())
        .header("Authorization", authorization.as_str())
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(body)
        .map_err(|error| {
            format!(
                "Unable to call {protocol} endpoint {}: {error}",
                endpoint_label(url)
            )
        })?;
    read_json_response(protocol, url, response)
}

pub(crate) fn post_json(url: &Url, protocol: &str, body: Value) -> Result<(u16, Value), String> {
    validate_anchor_https_url(url, protocol)?;
    let response = agent()
        .post(url.as_str())
        .config()
        .http_status_as_error(false)
        .build()
        .send_json(body)
        .map_err(|error| {
            format!(
                "Unable to call {protocol} endpoint {}: {error}",
                endpoint_label(url)
            )
        })?;
    read_json_response(protocol, url, response)
}

pub(crate) fn read_json_response(
    protocol: &str,
    url: &Url,
    mut response: ureq::http::Response<Body>,
) -> Result<(u16, Value), String> {
    let endpoint = endpoint_label(url);
    let status = response.status().as_u16();
    reject_anchor_redirect(status, &endpoint)?;
    let value = response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_RESPONSE_BYTES)
        .read_json::<Value>()
        .map_err(|error| format!("Invalid JSON from {protocol} endpoint {endpoint}: {error}"))?;
    Ok((status, value))
}

pub(crate) fn get_text(url: &Url, label: &str) -> Result<String, String> {
    validate_anchor_https_url(url, label)?;
    let mut response = agent()
        .get(url.as_str())
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|error| {
            format!(
                "Unable to load {label} from {}: {error}",
                endpoint_label(url)
            )
        })?;
    let endpoint = endpoint_label(url);
    let status = response.status().as_u16();
    reject_anchor_redirect(status, &endpoint)?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "{label} endpoint {endpoint} returned HTTP {status}"
        ));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_ANCHOR_RESPONSE_BYTES)
        .read_to_string()
        .map_err(|error| format!("Unable to read {label} from {endpoint}: {error}"))
}

fn is_external_dns_name(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    let labels = host.split('.').collect::<Vec<_>>();
    labels.len() >= 2
        && host.len() <= 253
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(|byte| byte.is_ascii_alphanumeric())
        })
        && host != "localhost"
        && !host.ends_with(".localhost")
        && host != "local"
        && !host.ends_with(".local")
        && host != "home.arpa"
        && !host.ends_with(".home.arpa")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_domain_is_structurally_validated() {
        assert_eq!(
            canonical_home_domain("Anchor.Example.").unwrap(),
            "anchor.example"
        );
        for value in [
            "https://anchor.example",
            "anchor.example/path",
            "127.0.0.1",
            "[::1]",
            "localhost",
            "wallet.local",
            "router.home.arpa",
            "intranet",
            "anchor.example:8443",
        ] {
            assert!(canonical_home_domain(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn anchor_urls_reject_local_ip_credentials_and_allow_declared_transport_ports() {
        assert!(validate_anchor_https_url(
            &Url::parse("https://anchor.example/sep6").unwrap(),
            "endpoint"
        )
        .is_ok());
        for value in [
            "http://anchor.example/sep6",
            "https://user:secret@anchor.example/sep6",
            "https://127.0.0.1/sep6",
            "https://[::1]/sep6",
            "https://localhost/sep6",
            "https://wallet.local/sep6",
        ] {
            assert!(
                validate_anchor_https_url(&Url::parse(value).unwrap(), "endpoint").is_err(),
                "accepted {value}"
            );
        }
        assert!(validate_anchor_https_url(
            &Url::parse("https://anchor.example:8443/sep6").unwrap(),
            "endpoint"
        )
        .is_ok());
    }

    #[test]
    fn endpoint_labels_drop_sensitive_query_values() {
        let url = Url::parse("https://anchor.example/customer?id=private#fragment").unwrap();
        assert_eq!(endpoint_label(&url), "https://anchor.example/customer");
    }
}
