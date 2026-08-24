use stellar_strkey::ed25519::PublicKey;
use stellar_xdr::{MuxedAccount, OperationType, TransactionEnvelope};
use thiserror::Error;

use crate::{sign_transaction_envelope, ClassicSigner, TransactionSigningError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCapability {
    public_key: String,
    network_passphrase: String,
    allowed_operations: Vec<OperationType>,
    max_operations: usize,
    max_fee: u32,
    expires_at_unix: Option<u64>,
}

impl AgentCapability {
    pub fn new(
        public_key: &str,
        network_passphrase: &str,
        mut allowed_operations: Vec<OperationType>,
        max_operations: usize,
        max_fee: u32,
        expires_at_unix: Option<u64>,
    ) -> Result<Self, AgentAccessError> {
        let public = PublicKey::from_string(public_key.trim())
            .map_err(|_| AgentAccessError::InvalidCapabilityAccount)?;
        if network_passphrase.is_empty() {
            return Err(AgentAccessError::EmptyNetworkPassphrase);
        }
        if allowed_operations.is_empty() {
            return Err(AgentAccessError::EmptyOperationAllowlist);
        }
        if max_operations == 0 {
            return Err(AgentAccessError::InvalidMaxOperations);
        }

        allowed_operations.sort_unstable();
        allowed_operations.dedup();

        Ok(Self {
            public_key: format!("{public}"),
            network_passphrase: network_passphrase.to_owned(),
            allowed_operations,
            max_operations,
            max_fee,
            expires_at_unix,
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn network_passphrase(&self) -> &str {
        &self.network_passphrase
    }

    pub fn allowed_operations(&self) -> &[OperationType] {
        &self.allowed_operations
    }

    pub fn max_operations(&self) -> usize {
        self.max_operations
    }

    pub fn max_fee(&self) -> u32 {
        self.max_fee
    }

    pub fn expires_at_unix(&self) -> Option<u64> {
        self.expires_at_unix
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentAuthorization {
    pub public_key: String,
    pub operation_types: Vec<OperationType>,
    pub fee: u32,
}

pub fn authorize_agent_transaction(
    capability: &AgentCapability,
    envelope: &TransactionEnvelope,
    network_passphrase: &str,
    now_unix: u64,
) -> Result<AgentAuthorization, AgentAccessError> {
    if network_passphrase != capability.network_passphrase {
        return Err(AgentAccessError::NetworkNotAllowed);
    }
    if capability
        .expires_at_unix
        .is_some_and(|expires_at| now_unix >= expires_at)
    {
        return Err(AgentAccessError::CapabilityExpired);
    }

    let envelope = match envelope {
        TransactionEnvelope::Tx(envelope) => envelope,
        TransactionEnvelope::TxV0(_) | TransactionEnvelope::TxFeeBump(_) => {
            return Err(AgentAccessError::UnsupportedEnvelope)
        }
    };
    if !envelope.signatures.is_empty() {
        return Err(AgentAccessError::PreexistingSignatures);
    }

    let tx = &envelope.tx;
    if muxed_account_public_key(&tx.source_account) != capability.public_key {
        return Err(AgentAccessError::TransactionSourceNotAllowed);
    }
    if tx.fee > capability.max_fee {
        return Err(AgentAccessError::FeeLimitExceeded {
            fee: tx.fee,
            max_fee: capability.max_fee,
        });
    }

    let operation_count = tx.operations.len();
    if operation_count == 0 {
        return Err(AgentAccessError::NoOperations);
    }
    if operation_count > capability.max_operations {
        return Err(AgentAccessError::OperationCountExceeded {
            count: operation_count,
            max: capability.max_operations,
        });
    }

    let mut operation_types = Vec::with_capacity(operation_count);
    for (index, operation) in tx.operations.iter().enumerate() {
        let source = operation
            .source_account
            .as_ref()
            .unwrap_or(&tx.source_account);
        if muxed_account_public_key(source) != capability.public_key {
            return Err(AgentAccessError::OperationSourceNotAllowed { index });
        }

        let operation_type = operation.body.discriminant();
        if !capability.allowed_operations.contains(&operation_type) {
            return Err(AgentAccessError::OperationNotAllowed {
                index,
                operation: operation_type,
            });
        }
        operation_types.push(operation_type);
    }

    Ok(AgentAuthorization {
        public_key: capability.public_key.clone(),
        operation_types,
        fee: tx.fee,
    })
}

pub fn sign_agent_transaction<S: ClassicSigner + ?Sized>(
    capability: &AgentCapability,
    envelope: &mut TransactionEnvelope,
    network_passphrase: &str,
    now_unix: u64,
    signer: &S,
) -> Result<AgentAuthorization, AgentAccessError> {
    let signer_public = PublicKey::from_string(signer.public_key())
        .map_err(|_| AgentAccessError::SignerNotAllowed)?;
    if format!("{signer_public}") != capability.public_key {
        return Err(AgentAccessError::SignerNotAllowed);
    }

    let authorization = authorize_agent_transaction(
        capability,
        envelope,
        network_passphrase,
        now_unix,
    )?;
    sign_transaction_envelope(envelope, network_passphrase, signer)?;
    Ok(authorization)
}

fn muxed_account_public_key(account: &MuxedAccount) -> String {
    let bytes = match account {
        MuxedAccount::Ed25519(ed25519) => ed25519.0,
        MuxedAccount::MuxedEd25519(muxed) => muxed.ed25519.0,
    };
    format!("{}", PublicKey(bytes))
}

#[derive(Debug, Error)]
pub enum AgentAccessError {
    #[error("agent capability account must be a valid Stellar G address")]
    InvalidCapabilityAccount,
    #[error("agent capability network passphrase cannot be empty")]
    EmptyNetworkPassphrase,
    #[error("agent capability must explicitly allow at least one operation type")]
    EmptyOperationAllowlist,
    #[error("agent capability max operation count must be positive")]
    InvalidMaxOperations,
    #[error("agent capability does not allow this Stellar network")]
    NetworkNotAllowed,
    #[error("agent capability has expired")]
    CapabilityExpired,
    #[error("agent access currently supports unsigned Classic V1 transaction envelopes only")]
    UnsupportedEnvelope,
    #[error("agent access does not accept transactions that already contain signatures")]
    PreexistingSignatures,
    #[error("transaction source account is outside the agent capability")]
    TransactionSourceNotAllowed,
    #[error("transaction fee {fee} exceeds agent capability limit {max_fee}")]
    FeeLimitExceeded { fee: u32, max_fee: u32 },
    #[error("agent transaction must contain at least one operation")]
    NoOperations,
    #[error("transaction has {count} operations; capability allows at most {max}")]
    OperationCountExceeded { count: usize, max: usize },
    #[error("operation {index} uses a source account outside the agent capability")]
    OperationSourceNotAllowed { index: usize },
    #[error("operation {index} type {operation:?} is not allowed by the agent capability")]
    OperationNotAllowed {
        index: usize,
        operation: OperationType,
    },
    #[error("signer public key does not match the agent capability account")]
    SignerNotAllowed,
    #[error(transparent)]
    TransactionSigning(#[from] TransactionSigningError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_transaction_envelope_xdr, SoftwareSigner};
    use stellar_xdr::{
        BumpSequenceOp, Operation, OperationBody, SequenceNumber, Uint256,
    };

    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PUBLIC: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const OTHER_PUBLIC: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const UNSIGNED_XDR_HEX: &str = concat!(
        "0000000200000000d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325",
        "af021a68f707511a000000640000000000000001000000000000000000000000",
        "0000000000000000"
    );

    fn decode_hex(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }

    fn one_operation_envelope() -> TransactionEnvelope {
        let mut envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        let TransactionEnvelope::Tx(value) = &mut envelope else {
            panic!("test vector must be V1")
        };
        value.tx.operations = vec![Operation {
            source_account: None,
            body: OperationBody::BumpSequence(BumpSequenceOp {
                bump_to: SequenceNumber(2),
            }),
        }]
        .try_into()
        .unwrap();
        envelope
    }

    fn capability() -> AgentCapability {
        AgentCapability::new(
            PUBLIC,
            TESTNET,
            vec![OperationType::BumpSequence],
            1,
            100,
            Some(2_000),
        )
        .unwrap()
    }

    #[test]
    fn authorizes_exact_account_network_operation_and_limits() {
        let authorization =
            authorize_agent_transaction(&capability(), &one_operation_envelope(), TESTNET, 1_999)
                .unwrap();

        assert_eq!(authorization.public_key, PUBLIC);
        assert_eq!(authorization.operation_types, vec![OperationType::BumpSequence]);
        assert_eq!(authorization.fee, 100);
    }

    #[test]
    fn rejects_expired_or_wrong_network_capability() {
        assert!(matches!(
            authorize_agent_transaction(&capability(), &one_operation_envelope(), TESTNET, 2_000),
            Err(AgentAccessError::CapabilityExpired)
        ));
        assert!(matches!(
            authorize_agent_transaction(
                &capability(),
                &one_operation_envelope(),
                "Public Global Stellar Network ; September 2015",
                1_000,
            ),
            Err(AgentAccessError::NetworkNotAllowed)
        ));
    }

    #[test]
    fn rejects_unlisted_operation_and_fee_over_limit() {
        let mut operation_capability = capability();
        operation_capability.allowed_operations = vec![OperationType::Payment];
        assert!(matches!(
            authorize_agent_transaction(
                &operation_capability,
                &one_operation_envelope(),
                TESTNET,
                1_000,
            ),
            Err(AgentAccessError::OperationNotAllowed {
                operation: OperationType::BumpSequence,
                ..
            })
        ));

        let mut fee_capability = capability();
        fee_capability.max_fee = 99;
        assert!(matches!(
            authorize_agent_transaction(
                &fee_capability,
                &one_operation_envelope(),
                TESTNET,
                1_000,
            ),
            Err(AgentAccessError::FeeLimitExceeded { fee: 100, max_fee: 99 })
        ));
    }

    #[test]
    fn rejects_operation_source_outside_capability() {
        let mut envelope = one_operation_envelope();
        let TransactionEnvelope::Tx(value) = &mut envelope else {
            panic!("test vector must be V1")
        };
        let other = PublicKey::from_string(OTHER_PUBLIC).unwrap();
        let mut operations: Vec<_> = value.tx.operations.clone().into();
        operations[0].source_account = Some(MuxedAccount::Ed25519(Uint256(other.0)));
        value.tx.operations = operations.try_into().unwrap();

        assert!(matches!(
            authorize_agent_transaction(&capability(), &envelope, TESTNET, 1_000),
            Err(AgentAccessError::OperationSourceNotAllowed { index: 0 })
        ));
    }

    #[test]
    fn rejects_preexisting_signatures() {
        let mut envelope = one_operation_envelope();
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();
        sign_transaction_envelope(&mut envelope, TESTNET, &signer).unwrap();

        assert!(matches!(
            authorize_agent_transaction(&capability(), &envelope, TESTNET, 1_000),
            Err(AgentAccessError::PreexistingSignatures)
        ));
    }

    #[test]
    fn authorized_signing_checks_signer_identity_and_signs_same_envelope() {
        let mut envelope = one_operation_envelope();
        let signer = SoftwareSigner::from_secret(SECRET).unwrap();

        let authorization =
            sign_agent_transaction(&capability(), &mut envelope, TESTNET, 1_000, &signer)
                .unwrap();

        assert_eq!(authorization.operation_types, vec![OperationType::BumpSequence]);
        let TransactionEnvelope::Tx(value) = envelope else {
            panic!("test vector must be V1")
        };
        assert_eq!(value.signatures.len(), 1);
    }

    #[test]
    fn rejects_zero_operation_transaction() {
        let envelope = parse_transaction_envelope_xdr(&decode_hex(UNSIGNED_XDR_HEX)).unwrap();
        assert!(matches!(
            authorize_agent_transaction(&capability(), &envelope, TESTNET, 1_000),
            Err(AgentAccessError::NoOperations)
        ));
    }
}
