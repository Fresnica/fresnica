use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use jsonrpsee_core::{client::ClientT, params::ObjectParams};
use stellar_rpc_client::{
    Client as StellarRpcClient, GetTransactionResponse, SendTransactionResponse,
    SimulateTransactionResponse,
};
use stellar_xdr::{Hash, TransactionEnvelope};

use crate::network_passphrase;
use crate::transaction::transaction_xdr_bytes;

pub const TESTNET_RPC_URL: &str = "https://soroban-testnet.stellar.org:443";

#[derive(Clone, Debug)]
pub struct RpcGateway {
    client: StellarRpcClient,
    network: String,
    network_passphrase: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcSubmissionError {
    Rejected(String),
    Uncertain(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcTransactionStatus {
    NotFound,
    Success {
        ledger: Option<u64>,
    },
    Failed {
        ledger: Option<u64>,
        details: String,
    },
}

impl RpcTransactionStatus {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Self::NotFound)
    }
}

impl RpcGateway {
    pub fn new(network: &str, rpc_url: &str) -> Result<Self, String> {
        let network_passphrase = network_passphrase(network)?;
        let client = StellarRpcClient::new(rpc_url)
            .map_err(|error| format!("Invalid Stellar RPC endpoint: {error}"))?;
        Ok(Self {
            client,
            network: network.to_owned(),
            network_passphrase,
        })
    }

    pub fn network(&self) -> &str {
        &self.network
    }

    pub fn network_passphrase(&self) -> &'static str {
        self.network_passphrase
    }

    pub fn rpc_url(&self) -> &str {
        self.client.base_url()
    }

    pub async fn verify_network(&self) -> Result<(), String> {
        self.client
            .verify_network_passphrase(Some(self.network_passphrase))
            .await
            .map(|_| ())
            .map_err(|error| format!("Stellar RPC network verification failed: {error}"))
    }

    pub async fn account_sequence(&self, address: &str) -> Result<i64, String> {
        let account = self.client.get_account(address).await.map_err(|error| {
            format!("Unable to load account {address} from Stellar RPC: {error}")
        })?;
        Ok(account.seq_num.0)
    }

    pub async fn default_soroban_inclusion_fee(&self) -> Result<u32, String> {
        let stats = self
            .client
            .get_fee_stats()
            .await
            .map_err(|error| format!("Unable to load Stellar RPC fee stats: {error}"))?;
        let fee = stats
            .soroban_inclusion_fee
            .mode
            .parse::<u32>()
            .map_err(|_| "Stellar RPC returned an invalid Soroban inclusion fee".to_owned())?;
        Ok(fee.max(100))
    }

    pub async fn simulate_transaction(
        &self,
        envelope: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, String> {
        // The official client currently exposes record mode but not RPC's transitional
        // `useUpgradedAuth` flag. Fresnica accepts either Address or AddressV2 entries
        // returned by the server and keeps the SDK/Core signing contract version-neutral.
        self.client
            .simulate_transaction_envelope(envelope, None)
            .await
            .map_err(|error| format!("Stellar RPC simulation failed: {error}"))
    }

    pub async fn submit_transaction(
        &self,
        envelope: &TransactionEnvelope,
    ) -> Result<String, RpcSubmissionError> {
        let transaction = transaction_xdr_bytes(envelope)
            .map_err(|error| RpcSubmissionError::Rejected(error.to_string()))?;
        let mut params = ObjectParams::new();
        params
            .insert("transaction", STANDARD.encode(transaction))
            .map_err(|error| RpcSubmissionError::Rejected(error.to_string()))?;

        let response: SendTransactionResponse = self
            .client
            .client()
            .request("sendTransaction", params)
            .await
            .map_err(|error| RpcSubmissionError::Uncertain(error.to_string()))?;

        match response.status.as_str() {
            "PENDING" | "DUPLICATE" => Ok(response.hash),
            "ERROR" => Err(RpcSubmissionError::Rejected(
                response
                    .error_result_xdr
                    .unwrap_or_else(|| "Stellar RPC rejected the transaction".to_owned()),
            )),
            "TRY_AGAIN_LATER" => Err(RpcSubmissionError::Rejected(
                "Stellar RPC did not accept the transaction; try again later".to_owned(),
            )),
            other => Err(RpcSubmissionError::Rejected(format!(
                "Unsupported Stellar RPC submission status: {other}"
            ))),
        }
    }

    pub async fn transaction_status(&self, tx_hash: &str) -> Result<RpcTransactionStatus, String> {
        let hash = Hash::from_str(tx_hash)
            .map_err(|_| format!("Invalid Stellar transaction hash: {tx_hash}"))?;
        let response = self
            .client
            .get_transaction(&hash)
            .await
            .map_err(|error| format!("Unable to reconcile transaction {tx_hash}: {error}"))?;
        normalize_transaction_status(response)
    }
}

fn normalize_transaction_status(
    response: GetTransactionResponse,
) -> Result<RpcTransactionStatus, String> {
    let ledger = response.ledger.map(u64::from);
    match response.status.as_str() {
        "NOT_FOUND" => Ok(RpcTransactionStatus::NotFound),
        "SUCCESS" => Ok(RpcTransactionStatus::Success { ledger }),
        "FAILED" => Ok(RpcTransactionStatus::Failed {
            ledger,
            details: response
                .result
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "Stellar RPC reported transaction failure".to_owned()),
        }),
        other => Err(format!(
            "Unsupported Stellar RPC transaction status: {other}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_gateway_rejects_unknown_network_before_endpoint_use() {
        assert!(RpcGateway::new("private", "https://rpc.example").is_err());
    }

    #[test]
    fn rpc_transaction_status_terminal_classification_is_explicit() {
        assert!(!RpcTransactionStatus::NotFound.is_terminal());
        assert!(RpcTransactionStatus::Success { ledger: Some(1) }.is_terminal());
        assert!(RpcTransactionStatus::Failed {
            ledger: None,
            details: "failed".to_owned(),
        }
        .is_terminal());
    }
}
