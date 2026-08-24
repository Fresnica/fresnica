use stellar_strkey::Strkey;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKind {
    Classic,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountIdentity {
    kind: AccountKind,
    address: String,
    public_key: Option<String>,
}

impl AccountIdentity {
    pub fn parse(address: &str) -> Result<Self, AccountError> {
        let address = address.trim();
        let parsed = Strkey::from_string(address).map_err(|_| AccountError::InvalidAddress)?;

        match parsed {
            Strkey::PublicKeyEd25519(_) => Ok(Self {
                kind: AccountKind::Classic,
                address: address.to_owned(),
                public_key: Some(address.to_owned()),
            }),
            Strkey::Contract(_) => Ok(Self {
                kind: AccountKind::Contract,
                address: address.to_owned(),
                public_key: None,
            }),
            _ => Err(AccountError::UnsupportedAddressKind),
        }
    }

    pub fn kind(&self) -> AccountKind {
        self.kind
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn public_key(&self) -> Option<&str> {
        self.public_key.as_deref()
    }

    pub fn is_classic(&self) -> bool {
        self.kind == AccountKind::Classic
    }

    pub fn is_contract(&self) -> bool {
        self.kind == AccountKind::Contract
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AccountError {
    #[error("invalid Stellar address")]
    InvalidAddress,
    #[error("unsupported Stellar address kind")]
    UnsupportedAddressKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASSIC: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";

    #[test]
    fn parses_classic_identity() {
        let account = AccountIdentity::parse(CLASSIC).unwrap();

        assert_eq!(account.kind(), AccountKind::Classic);
        assert_eq!(account.address(), CLASSIC);
        assert_eq!(account.public_key(), Some(CLASSIC));
        assert!(account.is_classic());
        assert!(!account.is_contract());
    }

    #[test]
    fn parses_contract_identity_without_classic_public_key() {
        let account = AccountIdentity::parse(CONTRACT).unwrap();

        assert_eq!(account.kind(), AccountKind::Contract);
        assert_eq!(account.address(), CONTRACT);
        assert_eq!(account.public_key(), None);
        assert!(account.is_contract());
        assert!(!account.is_classic());
    }

    #[test]
    fn rejects_invalid_and_non_account_strkeys() {
        assert_eq!(
            AccountIdentity::parse("not-a-stellar-address"),
            Err(AccountError::InvalidAddress)
        );
    }
}
