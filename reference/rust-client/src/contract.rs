use std::collections::BTreeMap;

use serde_json::Value;
use soroban_spec_tools::{sanitize, Spec};
use stellar_rpc_client::SimulateTransactionResponse;
use stellar_xdr::{
    ContractEvent, ContractEventType, DiagnosticEvent, ScSpecEntry, ScSpecFunctionV0,
    ScSpecTypeDef, ScVal,
};

use crate::horizon_gateway::HorizonGateway;
use crate::rpc_gateway::RpcGateway;
use crate::signing_coordination::ExternalEd25519SigningProvider;
use crate::soroban::{
    authorize_prepared_soroban, authorize_prepared_soroban_with_system_auth,
    prepare_soroban_invoke, sign_prepared_soroban, sign_prepared_soroban_with_providers,
    simulate_soroban_invoke, submit_prepared_soroban, validate_soroban_simulation,
    PreparedSorobanTransaction, SorobanInvokeRequest, SorobanReview,
};
use crate::storage::WalletStorage;
use crate::system_auth::SystemAuthUnlockProvider;
use crate::transaction::TransactionSubmission;

pub const DEFAULT_CONTRACT_AUTHORIZATION_LIFETIME_LEDGERS: u32 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractParameterType {
    pub name: String,
    pub example: Option<String>,
}

impl ContractParameterType {
    fn from_spec(spec: &Spec, type_def: &ScSpecTypeDef) -> Self {
        Self {
            name: contract_type_name(type_def),
            example: spec.example(0, type_def),
        }
    }
}

fn contract_type_name(type_def: &ScSpecTypeDef) -> String {
    match type_def {
        ScSpecTypeDef::MuxedAddress => "muxed_address".to_owned(),
        ScSpecTypeDef::Option(inner) => {
            format!("option<{}>", contract_type_name(&inner.value_type))
        }
        ScSpecTypeDef::Result(inner) => format!(
            "result<{},{}>",
            contract_type_name(&inner.ok_type),
            contract_type_name(&inner.error_type)
        ),
        ScSpecTypeDef::Vec(inner) => format!("vec<{}>", contract_type_name(&inner.element_type)),
        ScSpecTypeDef::Map(inner) => format!(
            "map<{},{}>",
            contract_type_name(&inner.key_type),
            contract_type_name(&inner.value_type)
        ),
        ScSpecTypeDef::Tuple(inner) => format!(
            "({})",
            inner
                .value_types
                .iter()
                .map(contract_type_name)
                .collect::<Vec<_>>()
                .join(",")
        ),
        ScSpecTypeDef::BytesN(inner) => format!("bytes[{}]", inner.n),
        ScSpecTypeDef::Udt(inner) => sanitize(&inner.name.to_utf8_string_lossy()),
        other => other.name().to_ascii_lowercase(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractParameter {
    pub name: String,
    pub doc: String,
    pub value_type: ContractParameterType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractFunction {
    pub name: String,
    pub doc: String,
    pub inputs: Vec<ContractParameter>,
    pub outputs: Vec<ContractParameterType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractExecutableKind {
    StellarAsset,
    Wasm,
    ExternalRef,
}

impl ContractExecutableKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StellarAsset => "stellar_asset",
            Self::Wasm => "wasm",
            Self::ExternalRef => "external_ref",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractExecutableObservation {
    pub kind: ContractExecutableKind,
    pub wasm_hash: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractInterface {
    pub contract_id: String,
    pub executable: ContractExecutableObservation,
    pub functions: Vec<ContractFunction>,
}

impl ContractInterface {
    pub(crate) fn from_spec(
        contract_id: &str,
        executable: ContractExecutableObservation,
        entries: &[ScSpecEntry],
    ) -> Self {
        let spec = Spec::new(entries);
        let functions = entries
            .iter()
            .filter_map(|entry| match entry {
                ScSpecEntry::FunctionV0(function) => Some(contract_function(&spec, function)),
                _ => None,
            })
            .collect();
        Self {
            contract_id: contract_id.to_owned(),
            executable,
            functions,
        }
    }

    pub fn function(&self, name: &str) -> Option<&ContractFunction> {
        self.functions.iter().find(|function| function.name == name)
    }
}

fn contract_function(spec: &Spec, function: &ScSpecFunctionV0) -> ContractFunction {
    ContractFunction {
        name: sanitize(&function.name.to_utf8_string_lossy()),
        doc: function.doc.to_utf8_string_lossy(),
        inputs: function
            .inputs
            .iter()
            .map(|input| ContractParameter {
                name: sanitize(&input.name.to_utf8_string_lossy()),
                doc: input.doc.to_utf8_string_lossy(),
                value_type: ContractParameterType::from_spec(spec, &input.type_),
            })
            .collect(),
        outputs: function
            .outputs
            .iter()
            .map(|output| ContractParameterType::from_spec(spec, output))
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractArgumentInput {
    pub name: String,
    pub value: String,
}

impl ContractArgumentInput {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractArgumentReview {
    pub name: String,
    pub value_type: String,
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractInvokeRequest {
    pub wallet: Option<String>,
    pub contract_id: String,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentInput>,
    pub inclusion_fee_stroops: Option<u32>,
    pub authorization_lifetime_ledgers: u32,
    address_names: BTreeMap<String, String>,
}

impl ContractInvokeRequest {
    pub fn new(
        contract_id: impl Into<String>,
        function_name: impl Into<String>,
        arguments: Vec<ContractArgumentInput>,
    ) -> Self {
        Self {
            wallet: None,
            contract_id: contract_id.into(),
            function_name: function_name.into(),
            arguments,
            inclusion_fee_stroops: None,
            authorization_lifetime_ledgers: DEFAULT_CONTRACT_AUTHORIZATION_LIFETIME_LEDGERS,
            address_names: BTreeMap::new(),
        }
    }

    pub fn add_address_name(&mut self, name: &str, address: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("contract address name cannot be empty".to_owned());
        }
        let address = address.trim();
        if address.is_empty() {
            return Err(format!(
                "contract address name {name:?} cannot resolve to an empty address"
            ));
        }
        let key = address_name_key(name);
        if let Some(existing) = self.address_names.get(&key) {
            if existing == address {
                return Ok(());
            }
            return Err(format!(
                "contract address name {name:?} is ambiguous: {existing} or {address}"
            ));
        }
        self.address_names.insert(key, address.to_owned());
        Ok(())
    }

    fn resolve(
        &self,
        spec_entries: &[ScSpecEntry],
    ) -> Result<(SorobanInvokeRequest, Vec<ContractArgumentReview>), String> {
        let spec = Spec::new(spec_entries);
        let function = find_function(&spec, &self.function_name)?;
        let function_name = function.name.to_utf8_string_lossy();
        let mut supplied = BTreeMap::new();
        for argument in &self.arguments {
            if supplied
                .insert(argument.name.clone(), argument.value.clone())
                .is_some()
            {
                return Err(format!(
                    "contract argument --{} was provided more than once",
                    argument.name
                ));
            }
        }

        let mut scvals = Vec::with_capacity(function.inputs.len());
        let mut review_arguments = Vec::with_capacity(function.inputs.len());
        for input in &function.inputs {
            let name = sanitize(&input.name.to_utf8_string_lossy());
            let value_type = contract_type_name(&input.type_);
            let parsed = match remove_argument(&mut supplied, &name) {
                Some(value) => {
                    parse_argument(&spec, &name, &value, &input.type_, &self.address_names)?
                }
                None if matches!(input.type_, ScSpecTypeDef::Option(_)) => ScVal::Void,
                None => {
                    return Err(format!(
                        "missing contract argument --{name} (expected {value_type})"
                    ));
                }
            };
            let normalized = spec.xdr_to_json(&parsed, &input.type_).map_err(|error| {
                format!("unable to normalize contract argument --{name} ({value_type}): {error}")
            })?;
            scvals.push(parsed);
            review_arguments.push(ContractArgumentReview {
                name,
                value_type,
                value: normalized,
            });
        }

        if let Some((name, _)) = supplied.first_key_value() {
            return Err(format!(
                "unknown contract argument --{name} for function {}",
                self.function_name
            ));
        }

        let mut request =
            SorobanInvokeRequest::new(self.contract_id.clone(), function_name, scvals);
        request.wallet = self.wallet.clone();
        request.inclusion_fee_stroops = self.inclusion_fee_stroops;
        request.authorization_lifetime_ledgers = self.authorization_lifetime_ledgers;
        Ok((request, review_arguments))
    }
}

fn remove_argument(arguments: &mut BTreeMap<String, String>, spec_name: &str) -> Option<String> {
    arguments.remove(spec_name).or_else(|| {
        let kebab = spec_name.replace('_', "-");
        if kebab == spec_name {
            None
        } else {
            arguments.remove(&kebab)
        }
    })
}

fn find_function<'a>(spec: &'a Spec, function_name: &str) -> Result<&'a ScSpecFunctionV0, String> {
    if let Ok(function) = spec.find_function(function_name) {
        return Ok(function);
    }

    let functions = spec
        .find_functions()
        .map_err(|error| format!("unable to read contract functions: {error}"))?;
    let mut available = Vec::new();
    for function in functions {
        let name = sanitize(&function.name.to_utf8_string_lossy());
        if name == function_name {
            return Ok(function);
        }
        available.push(name);
    }
    if available.is_empty() {
        Err("contract spec does not expose any functions".to_owned())
    } else {
        Err(format!(
            "contract function {function_name:?} was not found; available functions: {}",
            available.join(", ")
        ))
    }
}

fn parse_argument(
    spec: &Spec,
    name: &str,
    value: &str,
    type_def: &ScSpecTypeDef,
    address_names: &BTreeMap<String, String>,
) -> Result<ScVal, String> {
    match spec.from_string(value, type_def) {
        Ok(parsed) => return Ok(parsed),
        Err(direct_error) => {
            if matches!(
                type_def,
                ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
            ) {
                let key = address_name_key(value.trim().trim_matches('"'));
                if let Some(address) = address_names.get(&key) {
                    return spec.from_string(address, type_def).map_err(|error| {
                        format!(
                            "contract address name {value:?} resolved to {address}, but the resolved address is invalid for --{name}: {error}"
                        )
                    });
                }
            }
            return Err(contract_argument_parse_error(
                spec,
                name,
                type_def,
                direct_error,
            ));
        }
    }
}

fn address_name_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn contract_argument_parse_error(
    spec: &Spec,
    name: &str,
    type_def: &ScSpecTypeDef,
    error: soroban_spec_tools::Error,
) -> String {
    let value_type = contract_type_name(type_def);
    let example = spec
        .example(0, type_def)
        .map(|example| format!("; example: {example}"))
        .unwrap_or_default();
    format!("invalid value for contract argument --{name}; expected {value_type}{example}: {error}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractInvokeReview {
    pub wallet_name: String,
    pub fee_payer: String,
    pub operation_source: String,
    pub contract_id: String,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentReview>,
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

impl ContractInvokeReview {
    fn from_soroban(review: &SorobanReview, arguments: Vec<ContractArgumentReview>) -> Self {
        Self {
            wallet_name: review.wallet_name.clone(),
            fee_payer: review.fee_payer.clone(),
            operation_source: review.operation_source.clone(),
            contract_id: review.contract_id.clone(),
            function_name: review.function_name.clone(),
            arguments,
            authorizers: review.authorizers.clone(),
            credential_types: review.credential_types.clone(),
            auth_entry_count: review.auth_entry_count,
            total_fee_stroops: review.total_fee_stroops,
            resource_fee_stroops: review.resource_fee_stroops,
            inclusion_fee_stroops: review.inclusion_fee_stroops,
            min_resource_fee_stroops: review.min_resource_fee_stroops,
            simulation_ledger: review.simulation_ledger,
            authorization_expiration_ledger: review.authorization_expiration_ledger,
            network: review.network.clone(),
            transaction_hash: review.transaction_hash.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractReadResult {
    pub contract_id: String,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentReview>,
    pub output: Option<Value>,
    pub simulation_ledger: u32,
    pub network: String,
}

#[derive(Clone, Debug)]
pub enum ContractInvokePreparation {
    ReadOnly(ContractReadResult),
    Transaction(PreparedContractInvoke),
}

#[derive(Clone, Debug)]
pub struct PreparedContractInvoke {
    pub review: ContractInvokeReview,
    prepared: PreparedSorobanTransaction,
}

impl PreparedContractInvoke {
    pub fn signing_transaction_hash_hex(&self) -> String {
        self.prepared.signing_transaction_hash_hex()
    }
}

pub(crate) async fn contract_interface(
    rpc: &RpcGateway,
    contract_id: &str,
) -> Result<ContractInterface, String> {
    let snapshot = rpc.contract_spec_snapshot(contract_id).await?;
    Ok(ContractInterface::from_spec(
        contract_id,
        snapshot.executable,
        &snapshot.entries,
    ))
}

pub(crate) async fn prepare_contract_invoke(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    request: ContractInvokeRequest,
) -> Result<PreparedContractInvoke, String> {
    let spec_entries = rpc.contract_spec_entries(&request.contract_id).await?;
    let (low_level_request, arguments) = request.resolve(&spec_entries)?;
    let prepared = prepare_soroban_invoke(storage, rpc, low_level_request).await?;
    let review = ContractInvokeReview::from_soroban(&prepared.review, arguments);
    Ok(PreparedContractInvoke { review, prepared })
}

pub(crate) async fn prepare_contract_invoke_outcome(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    request: ContractInvokeRequest,
) -> Result<ContractInvokePreparation, String> {
    let spec_entries = rpc.contract_spec_entries(&request.contract_id).await?;
    let (low_level_request, arguments) = request.resolve(&spec_entries)?;
    let simulation = simulate_soroban_invoke(rpc, &low_level_request).await?;
    validate_soroban_simulation(&simulation)?;

    if simulation_requires_send(&simulation)? {
        let prepared = prepare_soroban_invoke(storage, rpc, low_level_request).await?;
        let review = ContractInvokeReview::from_soroban(&prepared.review, arguments);
        return Ok(ContractInvokePreparation::Transaction(
            PreparedContractInvoke { review, prepared },
        ));
    }

    let output = decode_simulation_output(&spec_entries, &request.function_name, &simulation)?;
    Ok(ContractInvokePreparation::ReadOnly(ContractReadResult {
        contract_id: request.contract_id,
        function_name: request.function_name,
        arguments,
        output,
        simulation_ledger: simulation.latest_ledger,
        network: rpc.network().to_owned(),
    }))
}

fn simulation_requires_send(simulation: &SimulateTransactionResponse) -> Result<bool, String> {
    let transaction_data = simulation
        .transaction_data()
        .map_err(|error| format!("Stellar RPC returned invalid transaction data: {error}"))?;
    let has_write = !transaction_data.resources.footprint.read_write.is_empty();
    let has_published_event = simulation
        .events()
        .map_err(|error| format!("Stellar RPC returned invalid simulation events: {error}"))?
        .iter()
        .any(
            |DiagnosticEvent {
                 event: ContractEvent { type_, .. },
                 ..
             }| matches!(type_, ContractEventType::Contract),
        );
    let has_auth = simulation
        .results()
        .map_err(|error| format!("Stellar RPC returned invalid simulation result: {error}"))?
        .iter()
        .any(|result| !result.auth.is_empty());
    Ok(has_write || has_published_event || has_auth)
}

fn decode_simulation_output(
    spec_entries: &[ScSpecEntry],
    function_name: &str,
    simulation: &SimulateTransactionResponse,
) -> Result<Option<Value>, String> {
    let spec = Spec::new(spec_entries);
    let function = find_function(&spec, function_name)?;
    let Some(output_type) = function.outputs.first() else {
        return Ok(None);
    };
    let results = simulation
        .results()
        .map_err(|error| format!("Stellar RPC returned invalid simulation result: {error}"))?;
    let result = results
        .first()
        .ok_or_else(|| "Soroban simulation did not return a contract result".to_owned())?;
    spec.xdr_to_json(&result.xdr, output_type)
        .map(Some)
        .map_err(|error| format!("unable to decode contract return value: {error}"))
}

pub(crate) fn authorize_contract_invoke(
    storage: &WalletStorage,
    prepared: &mut PreparedContractInvoke,
    passcode: &str,
) -> Result<(), String> {
    authorize_prepared_soroban(storage, &mut prepared.prepared, passcode)
}

pub(crate) fn authorize_contract_invoke_with_system_auth(
    storage: &WalletStorage,
    prepared: &mut PreparedContractInvoke,
    passcode: Option<&str>,
    system_auth_providers: &[SystemAuthUnlockProvider],
) -> Result<(), String> {
    authorize_prepared_soroban_with_system_auth(
        storage,
        &mut prepared.prepared,
        passcode,
        system_auth_providers,
    )
}

pub(crate) fn sign_contract_invoke(
    storage: &WalletStorage,
    prepared: &mut PreparedContractInvoke,
    horizon: &HorizonGateway,
    passcode: &str,
) -> Result<(), String> {
    sign_prepared_soroban(storage, &mut prepared.prepared, horizon, passcode)
}

pub(crate) fn sign_contract_invoke_with_providers(
    storage: &WalletStorage,
    prepared: &mut PreparedContractInvoke,
    horizon: &HorizonGateway,
    passcode: Option<&str>,
    system_auth_providers: &[SystemAuthUnlockProvider],
    external_providers: &[ExternalEd25519SigningProvider],
) -> Result<(), String> {
    sign_prepared_soroban_with_providers(
        storage,
        &mut prepared.prepared,
        horizon,
        passcode,
        system_auth_providers,
        external_providers,
    )
}

pub(crate) async fn submit_contract_invoke(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    prepared: &PreparedContractInvoke,
) -> Result<TransactionSubmission, String> {
    submit_prepared_soroban(storage, rpc, &prepared.prepared).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use stellar_strkey::Contract as StrkeyContract;
    use stellar_xdr::{
        ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeBytesN, ScSpecTypeOption, ScSpecTypeVec,
        ScSymbol, StringM, VecM,
    };

    use super::*;

    const ACCOUNT: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const OTHER_ACCOUNT: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";

    fn function_entry(name: &str, inputs: &[(&str, ScSpecTypeDef)]) -> ScSpecEntry {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::try_from("test function").unwrap(),
            name: ScSymbol::try_from(name).unwrap(),
            inputs: VecM::try_from(
                inputs
                    .iter()
                    .map(|(name, type_)| ScSpecFunctionInputV0 {
                        doc: StringM::try_from(format!("{name} doc")).unwrap(),
                        name: StringM::try_from(*name).unwrap(),
                        type_: type_.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            outputs: VecM::default(),
        })
    }

    fn vec_of(type_: ScSpecTypeDef) -> ScSpecTypeDef {
        ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
            element_type: Box::new(type_),
        }))
    }

    fn option_of(type_: ScSpecTypeDef) -> ScSpecTypeDef {
        ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(type_),
        }))
    }

    #[test]
    fn contract_interface_preserves_names_docs_types_and_official_examples() {
        let entries = vec![function_entry(
            "batch",
            &[
                ("values", vec_of(ScSpecTypeDef::U32)),
                ("memo", ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 4 })),
            ],
        )];

        let interface = ContractInterface::from_spec(
            "CCONTRACT",
            ContractExecutableObservation {
                kind: ContractExecutableKind::Wasm,
                wasm_hash: Some("ab".repeat(32)),
            },
            &entries,
        );
        let function = interface.function("batch").unwrap();
        assert_eq!(interface.executable.kind, ContractExecutableKind::Wasm);
        assert_eq!(
            interface.executable.wasm_hash.as_deref(),
            Some("abababababababababababababababababababababababababababababababab")
        );
        assert_eq!(function.doc, "test function");
        assert_eq!(function.inputs[0].name, "values");
        assert_eq!(function.inputs[0].value_type.name, "vec<u32>");
        assert!(function.inputs[0].value_type.example.is_some());
        assert_eq!(function.inputs[1].value_type.name, "bytes[4]");
    }

    #[test]
    fn named_arguments_resolve_in_contract_spec_order() {
        let entries = vec![function_entry(
            "transfer",
            &[
                ("from", ScSpecTypeDef::Address),
                ("amount", ScSpecTypeDef::I128),
            ],
        )];
        let request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "transfer",
            vec![
                ContractArgumentInput::new("amount", "10000000"),
                ContractArgumentInput::new("from", ACCOUNT),
            ],
        );

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Address(_)));
        assert!(matches!(low_level.args[1], ScVal::I128(_)));
        assert_eq!(review[0].name, "from");
        assert_eq!(review[0].value, json!(ACCOUNT));
        assert_eq!(review[1].name, "amount");
        assert_eq!(review[1].value, json!("10000000"));
    }

    #[test]
    fn address_names_resolve_after_raw_address_parsing() {
        let entries = vec![function_entry("balance", &[("id", ScSpecTypeDef::Address)])];
        let mut named = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![ContractArgumentInput::new("id", "Alice")],
        );
        named.add_address_name("alice", ACCOUNT).unwrap();
        let (_, review) = named.resolve(&entries).unwrap();
        assert_eq!(review[0].value, json!(ACCOUNT));

        let mut raw = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![ContractArgumentInput::new("id", ACCOUNT)],
        );
        raw.add_address_name(ACCOUNT, OTHER_ACCOUNT).unwrap();
        let (_, review) = raw.resolve(&entries).unwrap();
        assert_eq!(review[0].value, json!(ACCOUNT));
    }

    #[test]
    fn conflicting_address_names_fail_before_contract_parsing() {
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "balance", vec![]);
        request.add_address_name("Alice", ACCOUNT).unwrap();
        request.add_address_name("alice", ACCOUNT).unwrap();
        let error = request
            .add_address_name("ALICE", OTHER_ACCOUNT)
            .unwrap_err();
        assert!(error.contains("ambiguous"));
        assert!(error.contains(ACCOUNT));
        assert!(error.contains(OTHER_ACCOUNT));
    }

    #[test]
    fn kebab_argument_alias_matches_underscore_spec_name() {
        let entries = vec![function_entry(
            "set_value",
            &[("new_value", ScSpecTypeDef::U32)],
        )];
        let request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "set_value",
            vec![ContractArgumentInput::new("new-value", "7")],
        );
        let (_, review) = request.resolve(&entries).unwrap();
        assert_eq!(review[0].name, "new_value");
        assert_eq!(review[0].value, json!(7));
    }

    #[test]
    fn complex_values_are_parsed_by_official_spec_tools() {
        let entries = vec![function_entry(
            "batch",
            &[
                ("values", vec_of(ScSpecTypeDef::U32)),
                ("memo", ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 4 })),
                ("counter", ScSpecTypeDef::U256),
            ],
        )];
        let request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "batch",
            vec![
                ContractArgumentInput::new("values", "[1,2,3]"),
                ContractArgumentInput::new("memo", "deadbeef"),
                ContractArgumentInput::new("counter", "42"),
            ],
        );

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Vec(Some(_))));
        assert!(matches!(low_level.args[1], ScVal::Bytes(_)));
        assert!(matches!(low_level.args[2], ScVal::U256(_)));
        assert_eq!(review[0].value, json!([1, 2, 3]));
        assert_eq!(review[1].value, json!("deadbeef"));
        assert_eq!(review[2].value, json!("42"));
    }

    #[test]
    fn optional_argument_can_be_omitted_like_stellar_cli() {
        let entries = vec![function_entry(
            "maybe",
            &[("value", option_of(ScSpecTypeDef::U32))],
        )];
        let request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "maybe", vec![]);

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Void));
        assert_eq!(review[0].value, Value::Null);
        assert_eq!(review[0].value_type, "option<u32>");
    }

    #[test]
    fn duplicate_and_unknown_named_arguments_fail_before_rpc_submission() {
        let entries = vec![function_entry("balance", &[("id", ScSpecTypeDef::Address)])];
        let duplicate = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![
                ContractArgumentInput::new("id", ACCOUNT),
                ContractArgumentInput::new("id", ACCOUNT),
            ],
        );
        assert!(duplicate
            .resolve(&entries)
            .unwrap_err()
            .contains("more than once"));

        let unknown = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![
                ContractArgumentInput::new("id", ACCOUNT),
                ContractArgumentInput::new("extra", "1"),
            ],
        );
        assert!(unknown
            .resolve(&entries)
            .unwrap_err()
            .contains("unknown contract argument"));
    }

    #[test]
    fn invalid_value_uses_official_parser_error_without_xdr_leaking_to_products() {
        let entries = vec![function_entry("balance", &[("id", ScSpecTypeDef::Address)])];
        let request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![ContractArgumentInput::new("id", "not-an-address")],
        );
        let error = request.resolve(&entries).unwrap_err();
        assert!(error.contains("invalid value for contract argument --id"));
        assert!(error.contains("expected address"));
    }
}
