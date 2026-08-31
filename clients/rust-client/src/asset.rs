use std::str::FromStr;

use serde_json::Value;
use stellar_xdr::{
    AccountId, AlphaNum12, AlphaNum4, Asset, AssetCode12, AssetCode4, ChangeTrustAsset, PublicKey,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssetId(Asset);

impl AssetId {
    pub(crate) fn native() -> Self {
        Self(Asset::Native)
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("XLM") {
            Ok(Self::native())
        } else {
            Self::parse_issued(value)
        }
    }

    pub(crate) fn parse_issued(value: &str) -> Result<Self, String> {
        let (code, issuer) = value
            .split_once(':')
            .ok_or_else(|| "asset must be CODE:GISSUER".to_owned())?;
        Self::issued(code, issuer)
    }

    pub(crate) fn from_horizon(value: &Value) -> Result<Self, String> {
        match text(value, "asset_type") {
            Some("native") => Ok(Self::native()),
            Some("credit_alphanum4") | Some("credit_alphanum12") => {
                let asset_type = text(value, "asset_type").expect("matched asset type");
                let code = text(value, "asset_code")
                    .ok_or_else(|| "Horizon returned malformed asset code".to_owned())?;
                let issuer = text(value, "asset_issuer")
                    .ok_or_else(|| "Horizon returned malformed asset issuer".to_owned())?;
                let asset = Self::issued(code, issuer)?;
                if asset.horizon_type() != asset_type {
                    return Err("Horizon returned inconsistent asset type and code".to_owned());
                }
                Ok(asset)
            }
            _ => Err("Horizon returned unsupported asset type".to_owned()),
        }
    }

    pub(crate) fn display(&self) -> String {
        match &self.0 {
            Asset::Native => "XLM".to_owned(),
            Asset::CreditAlphanum4(asset) => {
                format!(
                    "{}:{}",
                    decode_code(&asset.asset_code.0),
                    account(&asset.issuer)
                )
            }
            Asset::CreditAlphanum12(asset) => {
                format!(
                    "{}:{}",
                    decode_code(&asset.asset_code.0),
                    account(&asset.issuer)
                )
            }
        }
    }

    pub(crate) fn is_native(&self) -> bool {
        matches!(self.0, Asset::Native)
    }

    pub(crate) fn code(&self) -> Option<String> {
        match &self.0 {
            Asset::Native => None,
            Asset::CreditAlphanum4(asset) => Some(decode_code(&asset.asset_code.0)),
            Asset::CreditAlphanum12(asset) => Some(decode_code(&asset.asset_code.0)),
        }
    }

    pub(crate) fn issuer(&self) -> Option<String> {
        match &self.0 {
            Asset::Native => None,
            Asset::CreditAlphanum4(asset) => Some(account(&asset.issuer)),
            Asset::CreditAlphanum12(asset) => Some(account(&asset.issuer)),
        }
    }

    pub(crate) fn issuer_is(&self, value: &str) -> bool {
        self.issuer().as_deref() == Some(value)
    }

    pub(crate) fn matches_balance(&self, balance: &Value) -> bool {
        match &self.0 {
            Asset::Native => text(balance, "asset_type") == Some("native"),
            _ => {
                text(balance, "asset_code") == self.code().as_deref()
                    && text(balance, "asset_issuer") == self.issuer().as_deref()
            }
        }
    }

    pub(crate) fn query(&self, prefix: &str) -> String {
        if self.is_native() {
            format!("{prefix}_asset_type=native")
        } else {
            format!(
                "{prefix}_asset_type={}&{prefix}_asset_code={}&{prefix}_asset_issuer={}",
                self.horizon_type(),
                self.code().expect("issued asset code"),
                self.issuer().expect("issued asset issuer")
            )
        }
    }

    pub(crate) fn to_xdr(&self) -> Asset {
        self.0.clone()
    }

    pub(crate) fn to_change_trust_xdr(&self) -> Result<ChangeTrustAsset, String> {
        match &self.0 {
            Asset::Native => Err("XLM does not use a trustline".to_owned()),
            Asset::CreditAlphanum4(asset) => Ok(ChangeTrustAsset::CreditAlphanum4(asset.clone())),
            Asset::CreditAlphanum12(asset) => Ok(ChangeTrustAsset::CreditAlphanum12(asset.clone())),
        }
    }

    fn issued(code: &str, issuer: &str) -> Result<Self, String> {
        validate_asset_code(code)?;
        let issuer = AccountId::from_str(issuer)
            .map_err(|_| "asset issuer must be a Classic G address".to_owned())?;
        if code.len() <= 4 {
            let mut raw = [0u8; 4];
            raw[..code.len()].copy_from_slice(code.as_bytes());
            Ok(Self(Asset::CreditAlphanum4(AlphaNum4 {
                asset_code: AssetCode4(raw),
                issuer,
            })))
        } else {
            let mut raw = [0u8; 12];
            raw[..code.len()].copy_from_slice(code.as_bytes());
            Ok(Self(Asset::CreditAlphanum12(AlphaNum12 {
                asset_code: AssetCode12(raw),
                issuer,
            })))
        }
    }

    fn horizon_type(&self) -> &'static str {
        match &self.0 {
            Asset::Native => "native",
            Asset::CreditAlphanum4(_) => "credit_alphanum4",
            Asset::CreditAlphanum12(_) => "credit_alphanum12",
        }
    }
}

fn validate_asset_code(code: &str) -> Result<(), String> {
    if code.is_empty()
        || code.len() > 12
        || !code.is_ascii()
        || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err("issued asset code must be 1-12 ASCII letters or digits".to_owned());
    }
    Ok(())
}

fn decode_code(raw: &[u8]) -> String {
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    String::from_utf8(raw[..end].to_vec()).expect("validated asset code is ASCII")
}

fn account(value: &AccountId) -> String {
    match &value.0 {
        PublicKey::PublicKeyTypeEd25519(key) => {
            format!("{}", PublicKey::PublicKeyTypeEd25519(key.clone()))
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
