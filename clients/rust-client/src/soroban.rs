use std::collections::BTreeMap;
use std::str::FromStr;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_sdk::{FresnicaSdk, SdkErrorCode};
use stellar_rpc_client::SimulateTransactionResponse;
use stellar_strkey::{ed25519::PublicKey as StrkeyPublicKey, Contract as StrkeyContract};
use stellar_xdr::{
    ContractId, Hash, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, Limits,
    OperationBody, PublicKey, ReadXdr, ScAddress, ScSymbol, ScVal, SorobanAddressCredentials,
    SorobanAuthorizationEntry, SorobanCredentials, TransactionEnvelope, TransactionExt, Uint256,
    VecM, WriteXdr,
};

use crate::ledger_authorization::load_classic_ledger_authorization_plan;
use crate::rpc_gateway::{RpcGateway, RpcSubmissionError, RpcTransactionStatus};
use crate::signing_coordination::sign_with_local_ed25519;
use crate::transaction::{
    build_single_operation_envelope, ensure_transaction_not_expired, network_passphrase,
    resolve_network_wallet, transaction_hash_bytes, transaction_xdr_bytes, PendingTransactionStore,
};
use crate::{HorizonGateway, TransactionSubmission, WalletRecord, WalletStorage};

const DEFAULT_AUTHORIZATION_LIFETIME_LEDGERS: u32 = 100;
const SUBMISSION_POLL_ATTEMPTS: usize = 30;
const SUBMISSION_POLL_DELAY: Duration = Duration::from_secs(1);
const XDR_DEPTH_LIMIT: u32 = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct SorobanInvokeRequest {
    pub wallet: Option<String>,
    pub contract_id: String,
    pub function_name: String,
    pub args: Vec<ScVal>,
    pub inclusion_fee_stroops: Option<u32>,
    pub authorization_lifetime_ledgers: u32,
}

impl SorobanInvokeRequest {
    pub fn new(
        contract_id: impl Into<String>,
        function_name: impl Into<String>,
        args: Vec<ScVal>,
    ) -> Self {
        Self {
            wallet: None,
            contract_id: contract_id.into(),
            function_name: function_name.into(),
            args,
            inclusion_fee_stroops: None,
            authorization_lifetime_ledgers: DEFAULT_AUTHORIZATION_LIFETIME_LEDGERS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SorobanReview {
    pub wallet_name: String,
    pub fee_payer: String,
    pub operation_source: String,
    pub contract_id: String,
    pub function_name: String,
    pub argument_count: usize,
    pub authorizers: Vec<String>,
    pub credential_types: Vec<String>,
    pub auth_entry_count: usize,
    pub total_fee_stroops: u32,
    pub resource_fee_stroops: i64,
    pub inclusion_fee_stroops: u32,
    pub min_resource_fee_stroops: u64,
    pub simulation_ledger: u32,
    pub authorization_expiration_ledger: Option<u32>,
    pub network: String,
    pub transaction_hash: String,
}

#[derive(Clone, Debug)]
pub struct PreparedSorobanTransaction {
    pub envelope: TransactionEnvelope,
    pub review: SorobanReview,
    reviewed_envelope_xdr: Vec<u8>,
    reviewed_transaction_hash: [u8; 32],
    authorized_envelope_xdr: Option<Vec<u8>>,
    authorized_transaction_hash: Option<[u8; 32]>,
    envelope_signing_complete: bool,
}

impl PreparedSorobanTransaction {
    pub fn signing_transaction_hash(&self) -> [u8; 32] {
        self.authorized_transaction_hash
            .unwrap_or(self.reviewed_transaction_hash)
    }

    pub fn signing_transaction_hash_hex(&self) -> String {
        hex(&self.signing_transaction_hash())
    }

    pub fn assert_review_binding(&self) -> Result<(), String> {
        let current_xdr = transaction_xdr_bytes(&self.envelope)?;
        let current_hash = transaction_hash_bytes(&self.envelope, &self.review.network)?;
        let expected_xdr = self
            .authorized_envelope_xdr
            .as_ref()
            .unwrap_or(&self.reviewed_envelope_xdr);
        if &current_xdr != expected_xdr || current_hash != self.signing_transaction_hash() {
            return Err(
                "Soroban transaction changed after review; prepare and review it again".to_owned(),
            );
        }
        Ok(())
    }

    pub fn assert_submit_binding(&self) -> Result<(), String> {
        let current_hash = transaction_hash_bytes(&self.envelope, &self.review.network)?;
        if current_hash != self.signing_transaction_hash() {
            return Err(
                "Soroban transaction changed after signing; prepare and review it again".to_owned(),
            );
        }
        Ok(())
    }

    fn bind_authorized_envelope(&mut self) -> Result<(), String> {
        self.authorized_envelope_xdr = Some(transaction_xdr_bytes(&self.envelope)?);
        self.authorized_transaction_hash = Some(transaction_hash_bytes(
            &self.envelope,
            &self.review.network,
        )?);
        Ok(())
    }
}

pub async fn prepare_soroban_invoke(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    request: SorobanInvokeRequest,
) -> Result<PreparedSorobanTransaction, String> {
    if request.authorization_lifetime_ledgers == 0 {
        return Err("Soroban authorization lifetime must be at least one ledger".to_owned());
    }
    rpc.verify_network().await?;
    let wallet = resolve_network_wallet(storage, rpc.network(), request.wallet.as_deref())?;
    reconcile_pending_soroban(storage, rpc, &wallet).await?;

    let sequence = rpc.account_sequence(&wallet.address).await?;
    let inclusion_fee = match request.inclusion_fee_stroops {
        Some(fee) if fee >= 100 => fee,
        Some(_) => return Err("Soroban inclusion fee must be at least 100 stroops".to_owned()),
        None => rpc.default_soroban_inclusion_fee().await?,
    };
    let candidate = build_candidate(&wallet, &request, sequence, inclusion_fee)?;
    let simulation = rpc.simulate_transaction(&candidate).await?;
    assemble_reviewed_transaction(wallet.name, rpc.network(), &request, candidate, simulation)
}

pub fn authorize_prepared_soroban(
    storage: &WalletStorage,
    prepared: &mut PreparedSorobanTransaction,
    passcode: &str,
) -> Result<(), String> {
    prepared.assert_review_binding()?;
    if prepared.authorized_envelope_xdr.is_some() {
        return Err("Soroban authorization has already been applied".to_owned());
    }
    let network = prepared.review.network.clone();
    let signers = local_signing_records(storage, &network)?;
    let mut entries = invoke_auth_entries(&prepared.envelope)?.to_vec();
    if entries.len() != prepared.review.auth_entry_count {
        return Err("Soroban authorization entry count changed after review".to_owned());
    }

    for entry in entries.iter_mut() {
        match &entry.credentials {
            SorobanCredentials::SourceAccount => continue,
            SorobanCredentials::Address(credentials)
            | SorobanCredentials::AddressV2(credentials) => {
                let authorizer = direct_classic_authorizer(credentials)?;
                let signer = signers.get(&authorizer).ok_or_else(|| {
                    format!("No local signer capability for Soroban authorizer {authorizer}")
                })?;
                let unsigned = authorization_entry_xdr(entry)?;
                let signed = sign_authorization_entry(
                    signer,
                    &network,
                    &authorizer,
                    unsigned.clone(),
                    passcode,
                )?;
                if signed == unsigned {
                    return Err("Soroban authorization signer returned no signature".to_owned());
                }
                let signed_entry = parse_authorization_entry_xdr(&signed)?;
                validate_signed_authorization(entry, &signed_entry)?;
                *entry = signed_entry;
            }
            SorobanCredentials::AddressWithDelegates(_) => {
                return Err(
                    "Delegated Soroban authorization requires a concrete provider".to_owned(),
                )
            }
        }
    }

    replace_invoke_auth_entries(&mut prepared.envelope, entries)?;
    prepared.bind_authorized_envelope()?;
    prepared.assert_review_binding()
}

pub fn sign_prepared_soroban(
    storage: &WalletStorage,
    prepared: &mut PreparedSorobanTransaction,
    horizon: &HorizonGateway,
    passcode: &str,
) -> Result<(), String> {
    if prepared.authorized_envelope_xdr.is_none() {
        return Err(
            "Authorize the reviewed Soroban transaction before envelope signing".to_owned(),
        );
    }
    if prepared.envelope_signing_complete {
        return Err("Soroban transaction envelope signing has already completed".to_owned());
    }
    prepared.assert_review_binding()?;
    ensure_transaction_not_expired(&prepared.envelope)?;
    let authorization = load_classic_ledger_authorization_plan(horizon, &prepared.envelope)?;
    sign_with_local_ed25519(
        storage,
        &authorization,
        &prepared.review.network,
        &mut prepared.envelope,
        passcode,
    )?;
    prepared.assert_submit_binding()?;
    prepared.envelope_signing_complete = true;
    Ok(())
}

pub async fn submit_prepared_soroban(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    prepared: &PreparedSorobanTransaction,
) -> Result<TransactionSubmission, String> {
    if !prepared.envelope_signing_complete {
        return Err("Sign the authorized Soroban transaction before submission".to_owned());
    }
    if rpc.network() != prepared.review.network {
        return Err("Soroban transaction network does not match RPC gateway".to_owned());
    }
    prepared.assert_submit_binding()?;

    let expected_hash = prepared.signing_transaction_hash_hex();
    match rpc.submit_transaction(&prepared.envelope).await {
        Ok(returned_hash) if returned_hash != expected_hash => remember_uncertain(
            storage,
            prepared,
            &expected_hash,
            "Stellar RPC returned a different transaction hash",
        ),
        Ok(_) => poll_submission(storage, rpc, prepared, &expected_hash).await,
        Err(RpcSubmissionError::Rejected(message)) => Err(format!(
            "Soroban transaction rejected ({expected_hash}): {message}"
        )),
        Err(RpcSubmissionError::Uncertain(message)) => {
            remember_uncertain(storage, prepared, &expected_hash, &message)
        }
    }
}

async fn poll_submission(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    prepared: &PreparedSorobanTransaction,
    tx_hash: &str,
) -> Result<TransactionSubmission, String> {
    for attempt in 0..SUBMISSION_POLL_ATTEMPTS {
        match rpc.transaction_status(tx_hash).await {
            Ok(RpcTransactionStatus::Success { ledger }) => {
                return Ok(TransactionSubmission {
                    hash: tx_hash.to_owned(),
                    ledger,
                })
            }
            Ok(RpcTransactionStatus::Failed { details, .. }) => {
                return Err(format!("Soroban transaction failed ({tx_hash}): {details}"))
            }
            Ok(RpcTransactionStatus::NotFound) if attempt + 1 < SUBMISSION_POLL_ATTEMPTS => {
                tokio::time::sleep(SUBMISSION_POLL_DELAY).await;
            }
            Ok(RpcTransactionStatus::NotFound) => {
                return remember_uncertain(
                    storage,
                    prepared,
                    tx_hash,
                    "Stellar RPC did not report a terminal transaction status in time",
                )
            }
            Err(message) => return remember_uncertain(storage, prepared, tx_hash, &message),
        }
    }
    unreachable!("submission polling loop always returns")
}

async fn reconcile_pending_soroban(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    wallet: &WalletRecord,
) -> Result<(), String> {
    PendingTransactionStore::for_home(storage.home())
        .reconcile_with_async(rpc.network(), &wallet.address, |tx_hash| async move {
            Ok(rpc.transaction_status(&tx_hash).await?.is_terminal())
        })
        .await
}

fn remember_uncertain(
    storage: &WalletStorage,
    prepared: &PreparedSorobanTransaction,
    tx_hash: &str,
    message: &str,
) -> Result<TransactionSubmission, String> {
    let persist_result = PendingTransactionStore::for_home(storage.home()).remember(
        &prepared.review.network,
        &prepared.review.fee_payer,
        tx_hash,
        "soroban",
    );
    match persist_result {
        Ok(()) => Err(format!(
            "Soroban transaction submission status is uncertain for {tx_hash}: {message}. A pending record was saved; Fresnica will reconcile this hash before allowing another write from the account."
        )),
        Err(persist_error) => Err(format!(
            "Soroban transaction submission status is uncertain for {tx_hash}: {message}. Fresnica could not persist pending-transaction protection: {persist_error}. Do not retry until you verify the transaction hash manually."
        )),
    }
}

fn build_candidate(
    wallet: &WalletRecord,
    request: &SorobanInvokeRequest,
    current_sequence: i64,
    inclusion_fee_stroops: u32,
) -> Result<TransactionEnvelope, String> {
    let contract = StrkeyContract::from_str(&request.contract_id)
        .map_err(|_| "Soroban contract_id must be a valid C address".to_owned())?;
    let function_name = ScSymbol::try_from(request.function_name.as_bytes().to_vec())
        .map_err(|_| "Soroban function name must fit an ScSymbol".to_owned())?;
    let args: VecM<ScVal> = request
        .args
        .clone()
        .try_into()
        .map_err(|_| "Too many Soroban invocation arguments".to_owned())?;
    let host_function = HostFunction::InvokeContract(InvokeContractArgs {
        contract_address: ScAddress::Contract(ContractId(Hash(contract.0))),
        function_name,
        args,
    });
    build_single_operation_envelope(
        &wallet.address,
        OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function,
            auth: VecM::default(),
        }),
        current_sequence,
        inclusion_fee_stroops,
        None,
    )
}

fn assemble_reviewed_transaction(
    wallet_name: String,
    network: &str,
    request: &SorobanInvokeRequest,
    candidate: TransactionEnvelope,
    simulation: SimulateTransactionResponse,
) -> Result<PreparedSorobanTransaction, String> {
    if let Some(error) = simulation.error.as_deref() {
        return Err(format!("Soroban transaction simulation failed: {error}"));
    }
    if simulation.restore_preamble.is_some() {
        return Err(
            "Soroban transaction requires an explicit footprint restore before review".to_owned(),
        );
    }
    if simulation.results.len() != 1 {
        return Err(format!(
            "Soroban simulation returned {} host-function results; expected one",
            simulation.results.len()
        ));
    }

    let TransactionEnvelope::Tx(mut envelope) = candidate else {
        return Err("Soroban v1 requires a TransactionEnvelope v1".to_owned());
    };
    if !envelope.signatures.is_empty() {
        return Err("Soroban candidate must be unsigned before simulation".to_owned());
    }
    if envelope.tx.operations.len() != 1 {
        return Err("Soroban v1 requires exactly one InvokeHostFunction operation".to_owned());
    }
    if !matches!(envelope.tx.ext, TransactionExt::V0) {
        return Err("Soroban candidate must not contain preassembled transaction data".to_owned());
    }

    let operation_source = operation_source_string(&envelope)?;
    let mut operations: Vec<_> = envelope.tx.operations.clone().into();
    let operation = operations
        .get_mut(0)
        .ok_or_else(|| "Soroban transaction is missing its operation".to_owned())?;
    let OperationBody::InvokeHostFunction(invoke) = &mut operation.body else {
        return Err("Soroban v1 supports only InvokeHostFunction".to_owned());
    };
    if !invoke.auth.is_empty() {
        return Err(
            "Soroban candidate must not contain preexisting authorization entries".to_owned(),
        );
    }
    if !matches!(invoke.host_function, HostFunction::InvokeContract(_)) {
        return Err("Soroban v1 supports only invoke-contract host functions".to_owned());
    }

    let mut auth = Vec::new();
    for encoded in &simulation.results[0].auth {
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "Stellar RPC returned invalid authorization base64".to_owned())?;
        auth.push(
            SorobanAuthorizationEntry::from_xdr(&bytes, Limits::depth(XDR_DEPTH_LIMIT))
                .map_err(|_| "Stellar RPC returned invalid authorization XDR".to_owned())?,
        );
    }
    let authorization_expiration_ledger = simulation
        .latest_ledger
        .checked_add(request.authorization_lifetime_ledgers)
        .ok_or_else(|| {
            "Soroban authorization expiration exceeds ledger sequence range".to_owned()
        })?;
    let mut detached_auth_count = 0usize;
    for entry in &mut auth {
        if let Some(credentials) = address_credentials_mut(entry) {
            if !matches!(credentials.signature, ScVal::Void) {
                return Err("Soroban simulation produced pre-signed authorization".to_owned());
            }
            credentials.signature_expiration_ledger = authorization_expiration_ledger;
            detached_auth_count += 1;
        }
    }
    invoke.auth = auth
        .try_into()
        .map_err(|_| "Stellar RPC returned too many authorization entries".to_owned())?;
    envelope.tx.operations = operations
        .try_into()
        .map_err(|_| "Soroban transaction contains too many operations".to_owned())?;

    let transaction_data = simulation
        .transaction_data()
        .map_err(|error| format!("Stellar RPC returned invalid transaction data: {error}"))?;
    if transaction_data.resource_fee < 0 {
        return Err("Soroban simulation produced a negative resource fee".to_owned());
    }
    let inclusion_fee_stroops = envelope.tx.fee;
    let total_fee = u64::from(inclusion_fee_stroops)
        .checked_add(simulation.min_resource_fee)
        .ok_or_else(|| "Soroban total fee overflow".to_owned())?;
    envelope.tx.fee = u32::try_from(total_fee).map_err(|_| {
        "Soroban total fee exceeds a v1 transaction fee; fee-bump support is not implemented"
            .to_owned()
    })?;
    envelope.tx.ext = TransactionExt::V1(transaction_data.clone());

    let envelope = TransactionEnvelope::Tx(envelope);
    let (authorizers, credential_types) = authorization_summary(&envelope, &operation_source)?;
    let reviewed_envelope_xdr = transaction_xdr_bytes(&envelope)?;
    let reviewed_transaction_hash = transaction_hash_bytes(&envelope, network)?;
    let review = SorobanReview {
        wallet_name,
        fee_payer: transaction_source_string(&envelope)?,
        operation_source,
        contract_id: request.contract_id.clone(),
        function_name: request.function_name.clone(),
        argument_count: request.args.len(),
        authorizers,
        credential_types,
        auth_entry_count: invoke_auth_entries(&envelope)?.len(),
        total_fee_stroops: u32::try_from(total_fee).expect("total fee was range checked"),
        resource_fee_stroops: transaction_data.resource_fee,
        inclusion_fee_stroops,
        min_resource_fee_stroops: simulation.min_resource_fee,
        simulation_ledger: simulation.latest_ledger,
        authorization_expiration_ledger: (detached_auth_count > 0)
            .then_some(authorization_expiration_ledger),
        network: network.to_owned(),
        transaction_hash: hex(&reviewed_transaction_hash),
    };

    Ok(PreparedSorobanTransaction {
        envelope,
        review,
        reviewed_envelope_xdr,
        reviewed_transaction_hash,
        authorized_envelope_xdr: None,
        authorized_transaction_hash: None,
        envelope_signing_complete: false,
    })
}

fn authorization_entry_xdr(entry: &SorobanAuthorizationEntry) -> Result<Vec<u8>, String> {
    entry
        .to_xdr(Limits::depth(XDR_DEPTH_LIMIT))
        .map_err(|error| format!("Unable to encode Soroban authorization: {error}"))
}

fn parse_authorization_entry_xdr(encoded: &[u8]) -> Result<SorobanAuthorizationEntry, String> {
    SorobanAuthorizationEntry::from_xdr(encoded, Limits::depth(XDR_DEPTH_LIMIT))
        .map_err(|error| format!("Signed Soroban authorization is invalid: {error}"))
}

fn authorization_summary(
    envelope: &TransactionEnvelope,
    operation_source: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut authorizers = Vec::new();
    let mut credential_types = Vec::new();
    for entry in invoke_auth_entries(envelope)? {
        match &entry.credentials {
            SorobanCredentials::SourceAccount => {
                authorizers.push(operation_source.to_owned());
                credential_types.push("source-account".to_owned());
            }
            SorobanCredentials::Address(credentials) => {
                authorizers.push(sc_address_string(&credentials.address)?);
                credential_types.push("address".to_owned());
            }
            SorobanCredentials::AddressV2(credentials) => {
                authorizers.push(sc_address_string(&credentials.address)?);
                credential_types.push("address-v2".to_owned());
            }
            SorobanCredentials::AddressWithDelegates(credentials) => {
                authorizers.push(sc_address_string(&credentials.address_credentials.address)?);
                credential_types.push("address-with-delegates".to_owned());
            }
        }
    }
    Ok((authorizers, credential_types))
}

fn direct_classic_authorizer(credentials: &SorobanAddressCredentials) -> Result<String, String> {
    match &credentials.address {
        ScAddress::Account(_) => sc_address_string(&credentials.address),
        _ => Err(
            "Unsupported detached Soroban authorizer; only Classic G accounts are supported"
                .to_owned(),
        ),
    }
}

fn validate_signed_authorization(
    reviewed: &SorobanAuthorizationEntry,
    signed: &SorobanAuthorizationEntry,
) -> Result<(), String> {
    let mut reviewed_semantics = reviewed.clone();
    let mut signed_semantics = signed.clone();
    set_signature_void(&mut reviewed_semantics)?;
    set_signature_void(&mut signed_semantics)?;
    if reviewed_semantics != signed_semantics {
        return Err("Soroban signer changed reviewed authorization meaning".to_owned());
    }
    if reviewed == signed {
        return Err("Soroban authorization signer returned no signature".to_owned());
    }
    Ok(())
}

fn set_signature_void(entry: &mut SorobanAuthorizationEntry) -> Result<(), String> {
    match &mut entry.credentials {
        SorobanCredentials::Address(credentials) | SorobanCredentials::AddressV2(credentials) => {
            credentials.signature = ScVal::Void;
            Ok(())
        }
        SorobanCredentials::AddressWithDelegates(_) => {
            Err("Delegated Soroban authorization requires a concrete provider".to_owned())
        }
        SorobanCredentials::SourceAccount => {
            Err("Source-account authorization does not carry a detached signature".to_owned())
        }
    }
}

fn sign_authorization_entry(
    record: &WalletRecord,
    network: &str,
    expected_authorizer: &str,
    authorization_entry_xdr: Vec<u8>,
    passcode: &str,
) -> Result<Vec<u8>, String> {
    if record.watch_only() || record.secret.is_none() {
        return Err(format!("wallet \"{}\" is watch-only", record.name));
    }
    let protected_json = serde_json::to_string(
        record
            .secret
            .as_ref()
            .ok_or_else(|| "wallet has no protected signing material".to_owned())?,
    )
    .map_err(|error| format!("Unable to encode protected signing material: {error}"))?;
    FresnicaSdk::new()
        .sign_soroban_authorization_xdr_with_passcode(
            protected_json,
            passcode.to_owned(),
            expected_authorizer.to_owned(),
            authorization_entry_xdr,
            network_passphrase(network)?.to_owned(),
        )
        .map_err(|error| match error.code {
            SdkErrorCode::InvalidPasscode => {
                "Unable to unlock Soroban authorizer: invalid Fresnica passcode".to_owned()
            }
            _ => format!("Unable to sign Soroban authorization: {error}"),
        })
}

fn local_signing_records(
    storage: &WalletStorage,
    network: &str,
) -> Result<BTreeMap<String, WalletRecord>, String> {
    Ok(storage
        .list()?
        .into_iter()
        .filter(|record| {
            record.network == network && !record.watch_only() && record.secret.is_some()
        })
        .map(|record| (record.address.clone(), record))
        .collect())
}

fn address_credentials_mut(
    entry: &mut SorobanAuthorizationEntry,
) -> Option<&mut SorobanAddressCredentials> {
    match &mut entry.credentials {
        SorobanCredentials::Address(credentials) | SorobanCredentials::AddressV2(credentials) => {
            Some(credentials)
        }
        SorobanCredentials::AddressWithDelegates(credentials) => {
            Some(&mut credentials.address_credentials)
        }
        SorobanCredentials::SourceAccount => None,
    }
}

fn invoke_auth_entries(
    envelope: &TransactionEnvelope,
) -> Result<&[SorobanAuthorizationEntry], String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err("Soroban v1 requires a TransactionEnvelope v1".to_owned());
    };
    if envelope.tx.operations.len() != 1 {
        return Err("Soroban v1 requires exactly one InvokeHostFunction operation".to_owned());
    }
    let OperationBody::InvokeHostFunction(invoke) = &envelope.tx.operations[0].body else {
        return Err("Soroban v1 supports only InvokeHostFunction".to_owned());
    };
    Ok(invoke.auth.as_ref())
}

fn replace_invoke_auth_entries(
    envelope: &mut TransactionEnvelope,
    entries: Vec<SorobanAuthorizationEntry>,
) -> Result<(), String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err("Soroban v1 requires a TransactionEnvelope v1".to_owned());
    };
    if envelope.tx.operations.len() != 1 {
        return Err("Soroban v1 requires exactly one InvokeHostFunction operation".to_owned());
    }
    let mut operations: Vec<_> = envelope.tx.operations.clone().into();
    let OperationBody::InvokeHostFunction(invoke) = &mut operations[0].body else {
        return Err("Soroban v1 supports only InvokeHostFunction".to_owned());
    };
    invoke.auth = entries
        .try_into()
        .map_err(|_| "Too many Soroban authorization entries".to_owned())?;
    envelope.tx.operations = operations
        .try_into()
        .map_err(|_| "Soroban transaction contains too many operations".to_owned())?;
    Ok(())
}

fn transaction_source_string(envelope: &TransactionEnvelope) -> Result<String, String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err("Soroban v1 requires a TransactionEnvelope v1".to_owned());
    };
    Ok(muxed_account_string(&envelope.tx.source_account))
}

fn operation_source_string(
    envelope: &stellar_xdr::TransactionV1Envelope,
) -> Result<String, String> {
    let operation = envelope
        .tx
        .operations
        .first()
        .ok_or_else(|| "Soroban transaction is missing its operation".to_owned())?;
    Ok(operation
        .source_account
        .as_ref()
        .map(muxed_account_string)
        .unwrap_or_else(|| muxed_account_string(&envelope.tx.source_account)))
}

fn muxed_account_string(account: &stellar_xdr::MuxedAccount) -> String {
    match account {
        stellar_xdr::MuxedAccount::Ed25519(Uint256(bytes)) => {
            format!("{}", StrkeyPublicKey(*bytes))
        }
        stellar_xdr::MuxedAccount::MuxedEd25519(muxed) => {
            format!("{}", StrkeyPublicKey(muxed.ed25519.0))
        }
    }
}

fn sc_address_string(address: &ScAddress) -> Result<String, String> {
    match address {
        ScAddress::Account(stellar_xdr::AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            bytes,
        )))) => Ok(format!("{}", StrkeyPublicKey(*bytes))),
        ScAddress::Contract(ContractId(Hash(bytes))) => Ok(format!("{}", StrkeyContract(*bytes))),
        _ => Err("Unsupported Soroban authorization address type".to_owned()),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{import_secret_record, plan_classic_ledger_authorization, AuthorizationThreshold};
    use stellar_rpc_client::SimulateHostFunctionResultRaw;
    use stellar_xdr::{
        LedgerFootprint, PublicKey, SequenceNumber, SorobanAuthorizedFunction,
        SorobanAuthorizedInvocation, SorobanResources, SorobanTransactionData,
        SorobanTransactionDataExt, WriteXdr,
    };

    const ACCOUNT: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const SECRET: &str = "SCOWDMM5576VUYF2QRFPJEXMFTCEISOFNF5TE2IZOA52YAY4VZ7WBQNO";
    const PASSCODE: &str = "correct horse battery staple";
    const TESTNET: &str = "testnet";

    fn transaction_data(resource_fee: i64) -> SorobanTransactionData {
        SorobanTransactionData {
            resources: SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: 100,
                disk_read_bytes: 5,
                write_bytes: 6,
            },
            resource_fee,
            ext: SorobanTransactionDataExt::V0,
        }
    }

    fn simulation(auth: SorobanAuthorizationEntry) -> SimulateTransactionResponse {
        let transaction_data = transaction_data(4_900);
        SimulateTransactionResponse {
            min_resource_fee: 4_900,
            results: vec![SimulateHostFunctionResultRaw {
                auth: vec![STANDARD.encode(authorization_entry_xdr(&auth).unwrap())],
                xdr: STANDARD.encode(ScVal::Void.to_xdr(Limits::none()).unwrap()),
            }],
            transaction_data: STANDARD.encode(transaction_data.to_xdr(Limits::none()).unwrap()),
            latest_ledger: 123_456,
            ..Default::default()
        }
    }

    fn source_auth() -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: ScAddress::Contract(ContractId(Hash([0; 32]))),
                    function_name: ScSymbol::try_from(b"transfer".to_vec()).unwrap(),
                    args: VecM::default(),
                }),
                sub_invocations: VecM::default(),
            },
        }
    }

    fn address_auth(address: ScAddress) -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::AddressV2(SorobanAddressCredentials {
                address,
                nonce: 42,
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: source_auth().root_invocation,
        }
    }

    fn candidate(request: &SorobanInvokeRequest) -> TransactionEnvelope {
        let record = WalletRecord {
            name: "main".to_owned(),
            address: ACCOUNT.to_owned(),
            wallet_type: "watch-only".to_owned(),
            network: TESTNET.to_owned(),
            secret: None,
            metadata: Default::default(),
        };
        build_candidate(&record, request, 7, 100).unwrap()
    }

    #[test]
    fn simulation_assembly_is_reviewed_after_auth_expiry_and_resource_fee() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let prepared = assemble_reviewed_transaction(
            "main".to_owned(),
            TESTNET,
            &request,
            candidate(&request),
            simulation(source_auth()),
        )
        .unwrap();

        assert_eq!(prepared.review.total_fee_stroops, 5_000);
        assert_eq!(prepared.review.resource_fee_stroops, 4_900);
        assert_eq!(prepared.review.inclusion_fee_stroops, 100);
        assert_eq!(prepared.review.authorizers, vec![ACCOUNT]);
        assert_eq!(prepared.review.credential_types, vec!["source-account"]);
        assert_eq!(prepared.review.authorization_expiration_ledger, None);
        assert_eq!(prepared.review.simulation_ledger, 123_456);
        prepared.assert_review_binding().unwrap();
    }

    #[test]
    fn detached_address_v2_expiration_is_part_of_reviewed_object() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let public = StrkeyPublicKey::from_string(ACCOUNT).unwrap();
        let auth = address_auth(ScAddress::Account(stellar_xdr::AccountId(
            PublicKey::PublicKeyTypeEd25519(Uint256(public.0)),
        )));
        let prepared = assemble_reviewed_transaction(
            "main".to_owned(),
            TESTNET,
            &request,
            candidate(&request),
            simulation(auth),
        )
        .unwrap();

        assert_eq!(prepared.review.authorizers, vec![ACCOUNT]);
        assert_eq!(prepared.review.credential_types, vec!["address-v2"]);
        assert_eq!(
            prepared.review.authorization_expiration_ledger,
            Some(123_556)
        );
        let entries = invoke_auth_entries(&prepared.envelope).unwrap();
        let SorobanCredentials::AddressV2(credentials) = &entries[0].credentials else {
            panic!("expected AddressV2 credentials");
        };
        assert_eq!(credentials.signature_expiration_ledger, 123_556);
    }

    #[test]
    fn detached_contract_authorizer_fails_closed_before_sdk_signing() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let auth = address_auth(ScAddress::Contract(ContractId(Hash([9; 32]))));
        let mut prepared = assemble_reviewed_transaction(
            "main".to_owned(),
            TESTNET,
            &request,
            candidate(&request),
            simulation(auth),
        )
        .unwrap();
        let root = std::env::temp_dir().join(format!(
            "fresnica-soroban-contract-authorizer-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();

        let error = authorize_prepared_soroban(&storage, &mut prepared, PASSCODE).unwrap_err();
        assert!(error.contains("only Classic G accounts"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detached_classic_authorization_uses_sdk_and_rebinds_transaction_hash() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let public = StrkeyPublicKey::from_string(ACCOUNT).unwrap();
        let auth = address_auth(ScAddress::Account(stellar_xdr::AccountId(
            PublicKey::PublicKeyTypeEd25519(Uint256(public.0)),
        )));
        let mut prepared = assemble_reviewed_transaction(
            "main".to_owned(),
            TESTNET,
            &request,
            candidate(&request),
            simulation(auth),
        )
        .unwrap();
        let reviewed_hash = prepared.signing_transaction_hash();
        let root = std::env::temp_dir().join(format!(
            "fresnica-soroban-direct-authorizer-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let storage = WalletStorage::new(&root).unwrap();
        let signer = import_secret_record("signer", TESTNET, SECRET, PASSCODE).unwrap();
        storage.save(&signer, false).unwrap();

        authorize_prepared_soroban(&storage, &mut prepared, PASSCODE).unwrap();

        assert_ne!(prepared.signing_transaction_hash(), reviewed_hash);
        prepared.assert_review_binding().unwrap();
        let entries = invoke_auth_entries(&prepared.envelope).unwrap();
        let SorobanCredentials::AddressV2(credentials) = &entries[0].credentials else {
            panic!("expected AddressV2 credentials");
        };
        assert!(!matches!(credentials.signature, ScVal::Void));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invoke_host_function_uses_medium_ledger_threshold() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let envelope = candidate(&request);
        let account = crate::LedgerAccountAuthorization {
            account_id: ACCOUNT.to_owned(),
            low_threshold: 1,
            medium_threshold: 2,
            high_threshold: 3,
            signers: Vec::new(),
        };
        let plan = plan_classic_ledger_authorization(&envelope, &[account]).unwrap();
        assert!(plan.requirements[0].uses.iter().any(|use_| {
            use_.threshold == AuthorizationThreshold::Medium
                && matches!(
                    use_.scope,
                    crate::AuthorizationScope::Operation {
                        kind: crate::ClassicOperationKind::InvokeHostFunction,
                        ..
                    }
                )
        }));
    }

    #[test]
    fn candidate_sequence_is_current_plus_one() {
        let request =
            SorobanInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "transfer", vec![]);
        let TransactionEnvelope::Tx(envelope) = candidate(&request) else {
            panic!("expected v1 envelope");
        };
        assert_eq!(envelope.tx.seq_num, SequenceNumber(8));
    }
}
