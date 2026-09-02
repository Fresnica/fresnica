use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};
use ureq::unversioned::multipart::{Form, Part};
use url::Url;

use crate::anchor_http::{
    agent as anchor_http_agent, endpoint_label, get_json, put_json, read_json_response,
    validate_anchor_https_url,
};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnchorCustomerStatus {
    Accepted,
    Processing,
    NeedsInfo,
    Rejected,
}

impl AnchorCustomerStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED",
            Self::Processing => "PROCESSING",
            Self::NeedsInfo => "NEEDS_INFO",
            Self::Rejected => "REJECTED",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ACCEPTED" => Ok(Self::Accepted),
            "PROCESSING" => Ok(Self::Processing),
            "NEEDS_INFO" => Ok(Self::NeedsInfo),
            "REJECTED" => Ok(Self::Rejected),
            other => Err(format!("SEP-12 returned unknown customer status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnchorCustomerFieldStatus {
    Accepted,
    Processing,
    Rejected,
    VerificationRequired,
}

impl AnchorCustomerFieldStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED",
            Self::Processing => "PROCESSING",
            Self::Rejected => "REJECTED",
            Self::VerificationRequired => "VERIFICATION_REQUIRED",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ACCEPTED" => Ok(Self::Accepted),
            "PROCESSING" => Ok(Self::Processing),
            "REJECTED" => Ok(Self::Rejected),
            "VERIFICATION_REQUIRED" => Ok(Self::VerificationRequired),
            other => Err(format!("SEP-12 returned unknown field status: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorCustomerField {
    pub name: String,
    pub description: Option<String>,
    pub field_type: Option<String>,
    pub optional: bool,
    pub choices: Vec<String>,
    pub status: Option<AnchorCustomerFieldStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorCustomerSnapshot {
    pub id: Option<String>,
    pub status: AnchorCustomerStatus,
    pub message: Option<String>,
    pub required_fields: Vec<AnchorCustomerField>,
    pub provided_fields: Vec<AnchorCustomerField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnchorCustomerQuery {
    pub id: Option<String>,
    pub customer_type: Option<String>,
    pub transaction_id: Option<String>,
    pub lang: Option<String>,
}

impl AnchorCustomerQuery {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("customer id", self.id.as_deref()),
            ("customer type", self.customer_type.as_deref()),
            ("transaction id", self.transaction_id.as_deref()),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(format!("{label} must not be empty"));
            }
        }
        if self.transaction_id.is_some() && self.customer_type.is_none() {
            return Err("SEP-12 transaction_id requires a customer type".to_owned());
        }
        if let Some(lang) = self.lang.as_deref() {
            if lang.len() != 2 || !lang.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                return Err("SEP-12 lang must be a two-letter ISO 639-1 code".to_owned());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorCustomerFile {
    pub name: String,
    pub file_name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnchorCustomerUpdate {
    pub id: Option<String>,
    pub customer_type: Option<String>,
    pub transaction_id: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub files: Vec<AnchorCustomerFile>,
}

impl AnchorCustomerUpdate {
    fn validate(&self) -> Result<(), String> {
        AnchorCustomerQuery {
            id: self.id.clone(),
            customer_type: self.customer_type.clone(),
            transaction_id: self.transaction_id.clone(),
            lang: None,
        }
        .validate()?;
        if self.fields.is_empty() && self.files.is_empty() {
            return Err("SEP-12 customer update requires at least one field".to_owned());
        }
        for (name, value) in &self.fields {
            validate_field_name(name)?;
            if value.trim().is_empty() {
                return Err(format!("SEP-12 field {name} must not be empty"));
            }
        }
        for file in &self.files {
            validate_field_name(&file.name)?;
            if self.fields.contains_key(&file.name) {
                return Err(format!(
                    "SEP-12 field {} cannot be both text and binary",
                    file.name
                ));
            }
            if file.file_name.trim().is_empty() || file.bytes.is_empty() {
                return Err(format!("SEP-12 binary field {} is empty", file.name));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnchorCustomerUpdateResult {
    pub id: String,
}

pub fn get_anchor_customer(
    server: &str,
    token: &str,
    query: &AnchorCustomerQuery,
) -> Result<AnchorCustomerSnapshot, String> {
    query.validate()?;
    let mut url = customer_url(server)?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(value) = query.id.as_deref() {
            pairs.append_pair("id", value);
        }
        if let Some(value) = query.customer_type.as_deref() {
            pairs.append_pair("type", value);
        }
        if let Some(value) = query.transaction_id.as_deref() {
            pairs.append_pair("transaction_id", value);
        }
        if let Some(value) = query.lang.as_deref() {
            pairs.append_pair("lang", value);
        }
    }

    let value = send_authenticated_json("GET", &url, token, None)?;
    parse_customer_snapshot(&value)
}

pub fn put_anchor_customer(
    server: &str,
    token: &str,
    update: &AnchorCustomerUpdate,
) -> Result<AnchorCustomerUpdateResult, String> {
    update.validate()?;
    let url = customer_url(server)?;
    let value = if update.files.is_empty() {
        let mut body = update_metadata(update);
        for (name, value) in &update.fields {
            body.insert(name.clone(), Value::String(value.clone()));
        }
        send_authenticated_json("PUT", &url, token, Some(Value::Object(body)))?
    } else {
        let authorization = Zeroizing::new(format!("Bearer {token}"));
        let mut form = Form::new();
        if let Some(value) = update.id.as_deref() {
            form = form.text("id", value);
        }
        if let Some(value) = update.customer_type.as_deref() {
            form = form.text("type", value);
        }
        if let Some(value) = update.transaction_id.as_deref() {
            form = form.text("transaction_id", value);
        }
        for (name, value) in &update.fields {
            form = form.text(name.as_str(), value.as_str());
        }
        for file in &update.files {
            let mut part = Part::bytes(&file.bytes).file_name(&file.file_name);
            if let Some(content_type) = file.content_type.as_deref() {
                part = part.mime_str(content_type).map_err(|error| {
                    format!("invalid MIME type for SEP-12 field {}: {error}", file.name)
                })?;
            }
            form = form.part(file.name.as_str(), part);
        }
        let request = anchor_http_agent()
            .put(url.as_str())
            .header("Authorization", authorization.as_str())
            .config()
            .http_status_as_error(false)
            .build();
        let response = request.send(form).map_err(|error| {
            format!(
                "Unable to call SEP-12 endpoint {}: {error}",
                endpoint_label(&url)
            )
        })?;
        let (status, value) = read_json_response("SEP-12", &url, response)?;
        if !(200..300).contains(&status) {
            let detail = value
                .get("error")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("request failed");
            return Err(format!("SEP-12 returned HTTP {status}: {detail}"));
        }
        value
    };
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "SEP-12 customer update returned no customer id".to_owned())?;
    Ok(AnchorCustomerUpdateResult { id: id.to_owned() })
}

fn send_authenticated_json(
    method: &str,
    url: &Url,
    token: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let (status, value) = match method {
        "GET" => get_json(url, "SEP-12", Some(token))?,
        "PUT" => put_json(
            url,
            "SEP-12",
            token,
            body.unwrap_or(Value::Object(Map::new())),
        )?,
        _ => return Err(format!("unsupported SEP-12 HTTP method: {method}")),
    };
    if !(200..300).contains(&status) {
        let detail = value
            .get("error")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("request failed");
        return Err(format!("SEP-12 returned HTTP {status}: {detail}"));
    }
    Ok(value)
}

fn customer_url(server: &str) -> Result<Url, String> {
    let mut url =
        Url::parse(server.trim()).map_err(|_| "SEP-12 server must be a valid HTTPS URL")?;
    validate_anchor_https_url(&url, "SEP-12 server")?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err("SEP-12 server must not contain a query or fragment".to_owned());
    }
    let path = format!("{}/customer", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn update_metadata(update: &AnchorCustomerUpdate) -> Map<String, Value> {
    let mut body = Map::new();
    if let Some(value) = update.id.as_deref() {
        body.insert("id".to_owned(), Value::String(value.to_owned()));
    }
    if let Some(value) = update.customer_type.as_deref() {
        body.insert("type".to_owned(), Value::String(value.to_owned()));
    }
    if let Some(value) = update.transaction_id.as_deref() {
        body.insert("transaction_id".to_owned(), Value::String(value.to_owned()));
    }
    body
}

fn parse_customer_snapshot(value: &Value) -> Result<AnchorCustomerSnapshot, String> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "SEP-12 customer response has no status".to_owned())?;
    Ok(AnchorCustomerSnapshot {
        id: optional_text(value.get("id")),
        status: AnchorCustomerStatus::parse(status)?,
        message: optional_text(value.get("message")),
        required_fields: parse_fields(value.get("fields"))?,
        provided_fields: parse_fields(value.get("provided_fields"))?,
    })
}

fn parse_fields(value: Option<&Value>) -> Result<Vec<AnchorCustomerField>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "SEP-12 customer fields must be an object".to_owned())?;
    let mut fields = Vec::with_capacity(object.len());
    for (name, raw) in object {
        let spec = raw
            .as_object()
            .ok_or_else(|| format!("SEP-12 field {name} must be an object"))?;
        let choices =
            spec.get("choices")
                .map(|value| {
                    value
                        .as_array()
                        .ok_or_else(|| format!("SEP-12 field {name} choices must be an array"))?
                        .iter()
                        .map(|choice| {
                            choice.as_str().map(str::to_owned).ok_or_else(|| {
                                format!("SEP-12 field {name} choice must be a string")
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
        let status = spec
            .get("status")
            .and_then(Value::as_str)
            .map(AnchorCustomerFieldStatus::parse)
            .transpose()?;
        fields.push(AnchorCustomerField {
            name: name.clone(),
            description: optional_text(spec.get("description")),
            field_type: optional_text(spec.get("type")),
            optional: spec
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            choices,
            status,
            error: optional_text(spec.get("error")),
        });
    }
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(fields)
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_field_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err("SEP-12 field name contains unsupported characters".to_owned());
    }
    if matches!(
        name,
        "id" | "type" | "transaction_id" | "account" | "memo" | "memo_type" | "lang"
    ) {
        return Err(format!(
            "SEP-12 field {name} is request metadata and must use the dedicated request option"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_url_preserves_server_path_and_rejects_unsafe_urls() {
        assert_eq!(
            customer_url("https://anchor.example/kyc/")
                .unwrap()
                .as_str(),
            "https://anchor.example/kyc/customer"
        );
        assert!(customer_url("http://anchor.example/kyc").is_err());
        assert!(customer_url("https://user:secret@anchor.example/kyc").is_err());
        assert!(customer_url("https://127.0.0.1/kyc").is_err());
        assert!(customer_url("https://localhost/kyc").is_err());
        assert!(customer_url("https://wallet.local/kyc").is_err());
        assert!(customer_url("https://anchor.example:8443/kyc").is_ok());
        assert!(customer_url("https://anchor.example/kyc?token=x").is_err());
    }

    #[test]
    fn endpoint_label_does_not_expose_customer_query_values() {
        let url = Url::parse(
            "https://anchor.example/kyc/customer?id=customer-secret&transaction_id=tx-secret",
        )
        .unwrap();
        let label = endpoint_label(&url);
        assert_eq!(label, "https://anchor.example/kyc/customer");
        assert!(!label.contains("customer-secret"));
        assert!(!label.contains("tx-secret"));
    }

    #[test]
    fn transaction_customer_query_requires_type() {
        let query = AnchorCustomerQuery {
            transaction_id: Some("tx-1".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            query.validate().unwrap_err(),
            "SEP-12 transaction_id requires a customer type"
        );
    }

    #[test]
    fn parses_needs_info_without_transport_json_leakage() {
        let snapshot = parse_customer_snapshot(&serde_json::json!({
            "id": "customer-1",
            "status": "NEEDS_INFO",
            "message": "More information required",
            "fields": {
                "photo_id_front": {
                    "description": "Photo ID",
                    "type": "binary"
                },
                "id_type": {
                    "description": "Government ID",
                    "type": "string",
                    "choices": ["Passport", "Drivers License"]
                }
            },
            "provided_fields": {
                "email_address": {
                    "type": "string",
                    "status": "ACCEPTED"
                }
            }
        }))
        .unwrap();

        assert_eq!(snapshot.status, AnchorCustomerStatus::NeedsInfo);
        assert_eq!(snapshot.required_fields.len(), 2);
        assert_eq!(snapshot.required_fields[0].name, "id_type");
        assert_eq!(
            snapshot.required_fields[0].choices,
            vec!["Passport", "Drivers License"]
        );
        assert_eq!(
            snapshot.provided_fields[0].status,
            Some(AnchorCustomerFieldStatus::Accepted)
        );
    }

    #[test]
    fn customer_update_rejects_duplicate_text_and_binary_field() {
        let mut update = AnchorCustomerUpdate::default();
        update
            .fields
            .insert("photo_id_front".to_owned(), "not-a-file".to_owned());
        update.files.push(AnchorCustomerFile {
            name: "photo_id_front".to_owned(),
            file_name: "id.jpg".to_owned(),
            content_type: Some("image/jpeg".to_owned()),
            bytes: vec![1, 2, 3],
        });
        assert!(update
            .validate()
            .unwrap_err()
            .contains("both text and binary"));
    }

    #[test]
    fn customer_update_reserves_request_metadata_fields() {
        let mut update = AnchorCustomerUpdate::default();
        update
            .fields
            .insert("transaction_id".to_owned(), "tx-1".to_owned());
        assert_eq!(
            update.validate().unwrap_err(),
            "SEP-12 field transaction_id is request metadata and must use the dedicated request option"
        );
    }

    #[test]
    fn update_metadata_uses_modern_sep12_identity_fields_only() {
        let update = AnchorCustomerUpdate {
            id: Some("customer-1".to_owned()),
            customer_type: Some("sep6".to_owned()),
            transaction_id: Some("tx-1".to_owned()),
            fields: BTreeMap::from([("first_name".to_owned(), "Ada".to_owned())]),
            files: Vec::new(),
        };
        let metadata = update_metadata(&update);
        assert_eq!(
            metadata.get("id").and_then(Value::as_str),
            Some("customer-1")
        );
        assert_eq!(metadata.get("type").and_then(Value::as_str), Some("sep6"));
        assert_eq!(
            metadata.get("transaction_id").and_then(Value::as_str),
            Some("tx-1")
        );
        assert!(!metadata.contains_key("account"));
        assert!(!metadata.contains_key("memo"));
    }
}
