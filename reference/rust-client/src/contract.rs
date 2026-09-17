use std::collections::BTreeMap;

use serde_json::Value;
use soroban_spec_tools::{sanitize, Spec};
use stellar_rpc_client::SimulateTransactionResponse;
use stellar_xdr::{
    ContractEvent, ContractEventType, DiagnosticEvent, Limits, ReadXdr, ScMetaEntry, ScMetaV0,
    ScSpecEntry, ScSpecFunctionV0, ScSpecTypeDef, ScVal, SorobanTransactionDataExt, WriteXdr,
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
pub const SEP41_INTERFACE_VERSION: &str = "0.5.1";
pub const CONTRACT_ABI_SCHEMA: &str = "fresnica-soroban-abi-v1";
const CONTRACT_ARGUMENT_XDR_DEPTH_LIMIT: u32 = 500;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractAbiType {
    Primitive(String),
    Option(Box<ContractAbiType>),
    Result {
        ok: Box<ContractAbiType>,
        error: Box<ContractAbiType>,
    },
    Vec(Box<ContractAbiType>),
    Map {
        key: Box<ContractAbiType>,
        value: Box<ContractAbiType>,
    },
    Tuple(Vec<ContractAbiType>),
    BytesN(u32),
    Udt(String),
}

impl ContractAbiType {
    fn from_spec(type_def: &ScSpecTypeDef) -> Self {
        match type_def {
            ScSpecTypeDef::Option(inner) => {
                Self::Option(Box::new(Self::from_spec(&inner.value_type)))
            }
            ScSpecTypeDef::Result(inner) => Self::Result {
                ok: Box::new(Self::from_spec(&inner.ok_type)),
                error: Box::new(Self::from_spec(&inner.error_type)),
            },
            ScSpecTypeDef::Vec(inner) => Self::Vec(Box::new(Self::from_spec(&inner.element_type))),
            ScSpecTypeDef::Map(inner) => Self::Map {
                key: Box::new(Self::from_spec(&inner.key_type)),
                value: Box::new(Self::from_spec(&inner.value_type)),
            },
            ScSpecTypeDef::Tuple(inner) => {
                Self::Tuple(inner.value_types.iter().map(Self::from_spec).collect())
            }
            ScSpecTypeDef::BytesN(inner) => Self::BytesN(inner.n),
            ScSpecTypeDef::Udt(inner) => Self::Udt(inner.name.to_utf8_string_lossy()),
            other => Self::Primitive(contract_type_name(other)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractParameterType {
    pub name: String,
    pub example: Option<String>,
    pub abi: ContractAbiType,
}

impl ContractParameterType {
    fn from_spec(spec: &Spec, type_def: &ScSpecTypeDef) -> Self {
        Self {
            name: contract_type_name(type_def),
            example: spec.example(0, type_def),
            abi: ContractAbiType::from_spec(type_def),
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractAbiField {
    pub name: String,
    pub doc: String,
    pub value_type: ContractAbiType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractAbiUnionCasePayload {
    Void,
    Tuple(Vec<ContractAbiType>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractAbiUnionCase {
    pub name: String,
    pub doc: String,
    pub payload: ContractAbiUnionCasePayload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractAbiEnumCase {
    pub name: String,
    pub doc: String,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractUserType {
    Struct {
        name: String,
        doc: String,
        lib: String,
        fields: Vec<ContractAbiField>,
    },
    Union {
        name: String,
        doc: String,
        lib: String,
        cases: Vec<ContractAbiUnionCase>,
    },
    Enum {
        name: String,
        doc: String,
        lib: String,
        cases: Vec<ContractAbiEnumCase>,
    },
    ErrorEnum {
        name: String,
        doc: String,
        lib: String,
        cases: Vec<ContractAbiEnumCase>,
    },
}

fn contract_user_type(entry: &ScSpecEntry) -> Option<ContractUserType> {
    match entry {
        ScSpecEntry::UdtStructV0(value) => Some(ContractUserType::Struct {
            name: value.name.to_utf8_string_lossy(),
            doc: value.doc.to_utf8_string_lossy(),
            lib: value.lib.to_utf8_string_lossy(),
            fields: value
                .fields
                .iter()
                .map(|field| ContractAbiField {
                    name: field.name.to_utf8_string_lossy(),
                    doc: field.doc.to_utf8_string_lossy(),
                    value_type: ContractAbiType::from_spec(&field.type_),
                })
                .collect(),
        }),
        ScSpecEntry::UdtUnionV0(value) => Some(ContractUserType::Union {
            name: value.name.to_utf8_string_lossy(),
            doc: value.doc.to_utf8_string_lossy(),
            lib: value.lib.to_utf8_string_lossy(),
            cases: value
                .cases
                .iter()
                .map(|case| match case {
                    stellar_xdr::ScSpecUdtUnionCaseV0::VoidV0(case) => ContractAbiUnionCase {
                        name: case.name.to_utf8_string_lossy(),
                        doc: case.doc.to_utf8_string_lossy(),
                        payload: ContractAbiUnionCasePayload::Void,
                    },
                    stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) => ContractAbiUnionCase {
                        name: case.name.to_utf8_string_lossy(),
                        doc: case.doc.to_utf8_string_lossy(),
                        payload: ContractAbiUnionCasePayload::Tuple(
                            case.type_.iter().map(ContractAbiType::from_spec).collect(),
                        ),
                    },
                })
                .collect(),
        }),
        ScSpecEntry::UdtEnumV0(value) => Some(ContractUserType::Enum {
            name: value.name.to_utf8_string_lossy(),
            doc: value.doc.to_utf8_string_lossy(),
            lib: value.lib.to_utf8_string_lossy(),
            cases: value
                .cases
                .iter()
                .map(|case| ContractAbiEnumCase {
                    name: case.name.to_utf8_string_lossy(),
                    doc: case.doc.to_utf8_string_lossy(),
                    value: case.value,
                })
                .collect(),
        }),
        ScSpecEntry::UdtErrorEnumV0(value) => Some(ContractUserType::ErrorEnum {
            name: value.name.to_utf8_string_lossy(),
            doc: value.doc.to_utf8_string_lossy(),
            lib: value.lib.to_utf8_string_lossy(),
            cases: value
                .cases
                .iter()
                .map(|case| ContractAbiEnumCase {
                    name: case.name.to_utf8_string_lossy(),
                    doc: case.doc.to_utf8_string_lossy(),
                    value: case.value,
                })
                .collect(),
        }),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractMetadataEntry {
    pub key: String,
    pub value: String,
}

impl ContractMetadataEntry {
    pub(crate) fn from_xdr(entry: &ScMetaEntry) -> Self {
        match entry {
            ScMetaEntry::ScMetaV0(ScMetaV0 { key, val }) => Self {
                key: key.to_utf8_string_lossy(),
                value: val.to_utf8_string_lossy(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractSep41Evidence {
    pub native_sac: bool,
    pub sep47_declared: bool,
    pub current_interface_compatible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractCapabilities {
    pub sep41: ContractSep41Evidence,
}

impl ContractCapabilities {
    fn from_spec(
        executable: &ContractExecutableObservation,
        metadata: &[ContractMetadataEntry],
        entries: &[ScSpecEntry],
    ) -> Self {
        Self {
            sep41: ContractSep41Evidence {
                native_sac: executable.kind == ContractExecutableKind::StellarAsset,
                sep47_declared: metadata_declares_sep(metadata, "41"),
                current_interface_compatible: sep41_interface_compatible(entries),
            },
        }
    }
}

fn metadata_declares_sep(metadata: &[ContractMetadataEntry], sep: &str) -> bool {
    metadata
        .iter()
        .filter(|entry| entry.key == "sep")
        .flat_map(|entry| entry.value.split(','))
        .map(str::trim)
        .any(|value| value == sep)
}

fn sep41_interface_compatible(entries: &[ScSpecEntry]) -> bool {
    function_signature_matches(entries, "allowance", &["address", "address"], &["i128"])
        && function_signature_matches(
            entries,
            "approve",
            &["address", "address", "i128", "u32"],
            &[],
        )
        && function_signature_matches(entries, "balance", &["address"], &["i128"])
        && function_signature_matches(
            entries,
            "transfer",
            &["address", "muxed_address", "i128"],
            &[],
        )
        && function_signature_matches(
            entries,
            "transfer_from",
            &["address", "address", "address", "i128"],
            &[],
        )
        && function_signature_matches(entries, "burn", &["address", "i128"], &[])
        && function_signature_matches(entries, "burn_from", &["address", "address", "i128"], &[])
        && function_signature_matches(entries, "decimals", &[], &["u32"])
        && function_signature_matches(entries, "name", &[], &["string"])
        && function_signature_matches(entries, "symbol", &[], &["string"])
}

fn function_signature_matches(
    entries: &[ScSpecEntry],
    name: &str,
    inputs: &[&str],
    outputs: &[&str],
) -> bool {
    entries
        .iter()
        .find_map(|entry| match entry {
            ScSpecEntry::FunctionV0(function) if function.name.to_utf8_string_lossy() == name => {
                Some(function)
            }
            _ => None,
        })
        .is_some_and(|function| {
            function.inputs.len() == inputs.len()
                && function.outputs.len() == outputs.len()
                && function
                    .inputs
                    .iter()
                    .zip(inputs)
                    .all(|(input, expected)| contract_type_name(&input.type_) == *expected)
                && function
                    .outputs
                    .iter()
                    .zip(outputs)
                    .all(|(output, expected)| contract_type_name(output) == *expected)
        })
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
    pub metadata: Vec<ContractMetadataEntry>,
    pub capabilities: ContractCapabilities,
    pub functions: Vec<ContractFunction>,
    pub user_types: Vec<ContractUserType>,
}

impl ContractInterface {
    pub(crate) fn from_spec(
        contract_id: &str,
        executable: ContractExecutableObservation,
        metadata: Vec<ContractMetadataEntry>,
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
        let user_types = entries.iter().filter_map(contract_user_type).collect();
        let capabilities = ContractCapabilities::from_spec(&executable, &metadata, entries);
        Self {
            contract_id: contract_id.to_owned(),
            executable,
            metadata,
            capabilities,
            functions,
            user_types,
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
    pub scval_xdr: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContractAddressNames {
    names: BTreeMap<String, String>,
}

impl ContractAddressNames {
    pub fn add(&mut self, name: &str, address: &str) -> Result<(), String> {
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
        if let Some(existing) = self.names.get(&key) {
            if existing == address {
                return Ok(());
            }
            return Err(format!(
                "contract address name {name:?} is ambiguous: {existing} or {address}"
            ));
        }
        self.names.insert(key, address.to_owned());
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.names.get(&address_name_key(name)).map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractInvokeRequest {
    pub wallet: Option<String>,
    pub contract_id: String,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentInput>,
    positional_arguments: Option<Vec<String>>,
    json_arguments: Vec<(String, Value)>,
    scval_xdr_arguments: Vec<ContractArgumentInput>,
    pub inclusion_fee_stroops: Option<u32>,
    pub authorization_lifetime_ledgers: u32,
    address_names: ContractAddressNames,
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
            positional_arguments: None,
            json_arguments: Vec::new(),
            scval_xdr_arguments: Vec::new(),
            inclusion_fee_stroops: None,
            authorization_lifetime_ledgers: DEFAULT_CONTRACT_AUTHORIZATION_LIFETIME_LEDGERS,
            address_names: ContractAddressNames::default(),
        }
    }

    pub fn new_positional(
        contract_id: impl Into<String>,
        function_name: impl Into<String>,
        arguments: Vec<String>,
    ) -> Self {
        Self {
            wallet: None,
            contract_id: contract_id.into(),
            function_name: function_name.into(),
            arguments: Vec::new(),
            positional_arguments: Some(arguments),
            json_arguments: Vec::new(),
            scval_xdr_arguments: Vec::new(),
            inclusion_fee_stroops: None,
            authorization_lifetime_ledgers: DEFAULT_CONTRACT_AUTHORIZATION_LIFETIME_LEDGERS,
            address_names: ContractAddressNames::default(),
        }
    }

    pub fn references_argument_value(&self, candidate: &str) -> bool {
        let candidate = candidate.trim().trim_matches('"').to_ascii_lowercase();
        self.arguments
            .iter()
            .map(|argument| argument.value.as_str())
            .chain(
                self.positional_arguments
                    .iter()
                    .flatten()
                    .map(String::as_str),
            )
            .any(|value| value.trim().trim_matches('"').to_ascii_lowercase() == candidate)
    }

    pub fn add_address_name(&mut self, name: &str, address: &str) -> Result<(), String> {
        self.address_names.add(name, address)
    }

    pub fn add_json_argument(&mut self, name: impl Into<String>, value: Value) {
        self.json_arguments.push((name.into(), value));
    }

    pub fn add_scval_xdr_argument(
        &mut self,
        name: impl Into<String>,
        xdr_base64: impl Into<String>,
    ) {
        self.scval_xdr_arguments
            .push(ContractArgumentInput::new(name, xdr_base64));
    }

    pub(crate) fn set_address_names(&mut self, address_names: ContractAddressNames) {
        self.address_names = address_names;
    }

    fn resolve(
        &self,
        spec_entries: &[ScSpecEntry],
    ) -> Result<(SorobanInvokeRequest, Vec<ContractArgumentReview>), String> {
        let spec = Spec::new(spec_entries);
        let function = find_function(&spec, &self.function_name)?;
        if self.positional_arguments.is_some()
            && (!self.json_arguments.is_empty() || !self.scval_xdr_arguments.is_empty())
        {
            return Err(
                "JSON and pre-encoded ScVal XDR arguments require named contract invocation"
                    .to_owned(),
            );
        }
        let function_name = function.name.to_utf8_string_lossy();
        let (scvals, review_arguments) = match &self.positional_arguments {
            Some(arguments) => {
                if arguments.len() > function.inputs.len() {
                    return Err(format!(
                        "contract function {} accepts {} arguments but {} were provided",
                        self.function_name,
                        function.inputs.len(),
                        arguments.len()
                    ));
                }
                let mut scvals = Vec::with_capacity(function.inputs.len());
                let mut review_arguments = Vec::with_capacity(function.inputs.len());
                for (index, input) in function.inputs.iter().enumerate() {
                    let name = sanitize(&input.name.to_utf8_string_lossy());
                    let value_type = contract_type_name(&input.type_);
                    let parsed = match arguments.get(index) {
                        Some(value) => {
                            parse_argument(&spec, &name, value, &input.type_, &self.address_names)?
                        }
                        None if matches!(input.type_, ScSpecTypeDef::Option(_)) => ScVal::Void,
                        None => {
                            return Err(format!(
                                "missing positional contract argument {} ({name}: {value_type})",
                                index + 1
                            ));
                        }
                    };
                    let review =
                        contract_argument_review(&spec, name, value_type, &input.type_, &parsed)?;
                    scvals.push(parsed);
                    review_arguments.push(review);
                }
                (scvals, review_arguments)
            }
            None => {
                let mut supplied = BTreeMap::new();
                for argument in &self.arguments {
                    if supplied
                        .insert(
                            argument.name.clone(),
                            ContractArgumentSource::SpecValue(argument.value.clone()),
                        )
                        .is_some()
                    {
                        return Err(format!(
                            "contract argument --{} was provided more than once",
                            argument.name
                        ));
                    }
                }
                for (name, value) in &self.json_arguments {
                    if supplied
                        .insert(name.clone(), ContractArgumentSource::Json(value.clone()))
                        .is_some()
                    {
                        return Err(format!(
                            "contract argument --{name} was provided more than once"
                        ));
                    }
                }
                for argument in &self.scval_xdr_arguments {
                    if supplied
                        .insert(
                            argument.name.clone(),
                            ContractArgumentSource::ScValXdrBase64(argument.value.clone()),
                        )
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
                        Some(ContractArgumentSource::SpecValue(value)) => {
                            parse_argument(&spec, &name, &value, &input.type_, &self.address_names)?
                        }
                        Some(ContractArgumentSource::Json(value)) => {
                            parse_json_argument(&spec, &name, &value, &input.type_)?
                        }
                        Some(ContractArgumentSource::ScValXdrBase64(value)) => {
                            parse_scval_xdr_argument(&name, &value)?
                        }
                        None if matches!(input.type_, ScSpecTypeDef::Option(_)) => ScVal::Void,
                        None => {
                            return Err(format!(
                                "missing contract argument --{name} (expected {value_type})"
                            ));
                        }
                    };
                    let review =
                        contract_argument_review(&spec, name, value_type, &input.type_, &parsed)?;
                    scvals.push(parsed);
                    review_arguments.push(review);
                }

                if let Some((name, _)) = supplied.first_key_value() {
                    return Err(format!(
                        "unknown contract argument --{name} for function {}",
                        self.function_name
                    ));
                }
                (scvals, review_arguments)
            }
        };

        let mut request =
            SorobanInvokeRequest::new(self.contract_id.clone(), function_name, scvals);
        request.wallet = self.wallet.clone();
        request.inclusion_fee_stroops = self.inclusion_fee_stroops;
        request.authorization_lifetime_ledgers = self.authorization_lifetime_ledgers;
        Ok((request, review_arguments))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ContractArgumentSource {
    SpecValue(String),
    Json(Value),
    ScValXdrBase64(String),
}

fn ensure_contract_type_normalization_safe(
    spec: &Spec,
    type_def: &ScSpecTypeDef,
    visited_udts: &mut Vec<String>,
) -> Result<(), String> {
    match type_def {
        ScSpecTypeDef::Option(inner) => {
            ensure_contract_type_normalization_safe(spec, &inner.value_type, visited_udts)
        }
        ScSpecTypeDef::Result(inner) => {
            ensure_contract_type_normalization_safe(spec, &inner.ok_type, visited_udts)
        }
        ScSpecTypeDef::Vec(inner) => {
            ensure_contract_type_normalization_safe(spec, &inner.element_type, visited_udts)
        }
        ScSpecTypeDef::Map(inner) => {
            ensure_contract_type_normalization_safe(spec, &inner.key_type, visited_udts)?;
            ensure_contract_type_normalization_safe(spec, &inner.value_type, visited_udts)
        }
        ScSpecTypeDef::Tuple(inner) => {
            for value_type in &inner.value_types {
                ensure_contract_type_normalization_safe(spec, value_type, visited_udts)?;
            }
            Ok(())
        }
        ScSpecTypeDef::Error => Err(
            "Contract Spec Error values cannot be safely normalized as ordinary contract values"
                .to_owned(),
        ),
        ScSpecTypeDef::Udt(inner) => {
            let type_name = inner.name.to_utf8_string_lossy();
            if visited_udts.iter().any(|visited| visited == &type_name) {
                return Ok(());
            }
            visited_udts.push(type_name.clone());
            let result = match spec.find(&type_name).map_err(|error| {
                format!("unable to resolve user-defined type {type_name}: {error}")
            })? {
                ScSpecEntry::UdtStructV0(struct_) => {
                    let tuple_struct = struct_
                        .fields
                        .first()
                        .is_some_and(|field| field.name.to_utf8_string_lossy() == "0");
                    if !tuple_struct {
                        let names = struct_
                            .fields
                            .iter()
                            .map(|field| field.name.to_utf8_string_lossy())
                            .collect::<Vec<_>>();
                        if names.windows(2).any(|pair| pair[0] >= pair[1]) {
                            return Err(format!(
                                "named struct {type_name} does not use canonical field order required for safe Contract-Spec normalization"
                            ));
                        }
                    }
                    for field in &struct_.fields {
                        ensure_contract_type_normalization_safe(spec, &field.type_, visited_udts)?;
                    }
                    Ok(())
                }
                ScSpecEntry::UdtUnionV0(union) => {
                    for case in &union.cases {
                        if let stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) = case {
                            for value_type in &case.type_ {
                                ensure_contract_type_normalization_safe(
                                    spec,
                                    value_type,
                                    visited_udts,
                                )?;
                            }
                        }
                    }
                    Ok(())
                }
                ScSpecEntry::UdtEnumV0(_) => Ok(()),
                ScSpecEntry::UdtErrorEnumV0(_) => Err(format!(
                    "error enum {type_name} cannot be safely normalized as an ordinary contract value"
                )),
                _ => Err(format!(
                    "Contract Spec entry {type_name} is not a user-defined value type"
                )),
            };
            visited_udts.pop();
            result
        }
        _ => Ok(()),
    }
}

fn validate_scval_against_type(
    spec: &Spec,
    value: &ScVal,
    type_def: &ScSpecTypeDef,
    depth: u32,
) -> Result<(), String> {
    if depth > CONTRACT_ARGUMENT_XDR_DEPTH_LIMIT {
        return Err("contract value exceeds the supported nesting depth".to_owned());
    }
    let next = depth + 1;
    let mismatch = || {
        Err(format!(
            "ScVal does not match Contract Spec type {}",
            contract_type_name(type_def)
        ))
    };
    match type_def {
        ScSpecTypeDef::Val => Ok(()),
        ScSpecTypeDef::Bool if matches!(value, ScVal::Bool(_)) => Ok(()),
        ScSpecTypeDef::Void if matches!(value, ScVal::Void) => Ok(()),
        ScSpecTypeDef::Error if matches!(value, ScVal::Error(_)) => Ok(()),
        ScSpecTypeDef::U32 if matches!(value, ScVal::U32(_)) => Ok(()),
        ScSpecTypeDef::I32 if matches!(value, ScVal::I32(_)) => Ok(()),
        ScSpecTypeDef::U64 if matches!(value, ScVal::U64(_)) => Ok(()),
        ScSpecTypeDef::I64 if matches!(value, ScVal::I64(_)) => Ok(()),
        ScSpecTypeDef::Timepoint if matches!(value, ScVal::Timepoint(_)) => Ok(()),
        ScSpecTypeDef::Duration if matches!(value, ScVal::Duration(_)) => Ok(()),
        ScSpecTypeDef::U128 if matches!(value, ScVal::U128(_)) => Ok(()),
        ScSpecTypeDef::I128 if matches!(value, ScVal::I128(_)) => Ok(()),
        ScSpecTypeDef::U256 if matches!(value, ScVal::U256(_)) => Ok(()),
        ScSpecTypeDef::I256 if matches!(value, ScVal::I256(_)) => Ok(()),
        ScSpecTypeDef::Bytes if matches!(value, ScVal::Bytes(_)) => Ok(()),
        ScSpecTypeDef::BytesN(expected) => match value {
            ScVal::Bytes(bytes) if bytes.as_slice().len() == expected.n as usize => Ok(()),
            _ => mismatch(),
        },
        ScSpecTypeDef::String if matches!(value, ScVal::String(_)) => Ok(()),
        ScSpecTypeDef::Symbol if matches!(value, ScVal::Symbol(_)) => Ok(()),
        ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
            if matches!(value, ScVal::Address(_)) =>
        {
            Ok(())
        }
        ScSpecTypeDef::Option(inner) => {
            if matches!(value, ScVal::Void) {
                Ok(())
            } else {
                validate_scval_against_type(spec, value, &inner.value_type, next)
            }
        }
        ScSpecTypeDef::Result(inner) => {
            if matches!(value, ScVal::Error(_)) {
                return Err(
                    "Result error values cannot be normalized as successful contract values"
                        .to_owned(),
                );
            }
            validate_scval_against_type(spec, value, &inner.ok_type, next)
        }
        ScSpecTypeDef::Vec(inner) => match value {
            ScVal::Vec(Some(values)) => {
                for value in values.iter() {
                    validate_scval_against_type(spec, value, &inner.element_type, next)?;
                }
                Ok(())
            }
            _ => mismatch(),
        },
        ScSpecTypeDef::Map(inner) => match value {
            ScVal::Map(Some(values)) => {
                for entry in values.iter() {
                    validate_scval_against_type(spec, &entry.key, &inner.key_type, next)?;
                    validate_scval_against_type(spec, &entry.val, &inner.value_type, next)?;
                }
                Ok(())
            }
            _ => mismatch(),
        },
        ScSpecTypeDef::Tuple(inner) => match value {
            ScVal::Vec(Some(values)) if values.len() == inner.value_types.len() => {
                for (value, type_def) in values.iter().zip(inner.value_types.iter()) {
                    validate_scval_against_type(spec, value, type_def, next)?;
                }
                Ok(())
            }
            _ => mismatch(),
        },
        ScSpecTypeDef::Udt(inner) => {
            let type_name = inner.name.to_utf8_string_lossy();
            match spec.find(&type_name).map_err(|error| {
                format!("unable to resolve user-defined type {type_name}: {error}")
            })? {
                ScSpecEntry::UdtStructV0(struct_) => {
                    let tuple_struct = struct_
                        .fields
                        .first()
                        .is_some_and(|field| field.name.to_utf8_string_lossy() == "0");
                    if tuple_struct {
                        let ScVal::Vec(Some(values)) = value else {
                            return mismatch();
                        };
                        if values.len() != struct_.fields.len() {
                            return Err(format!(
                                "tuple struct {type_name} requires {} values but ScVal contains {}",
                                struct_.fields.len(),
                                values.len()
                            ));
                        }
                        for (value, field) in values.iter().zip(struct_.fields.iter()) {
                            validate_scval_against_type(spec, value, &field.type_, next)?;
                        }
                        return Ok(());
                    }

                    let ScVal::Map(Some(values)) = value else {
                        return mismatch();
                    };
                    if values.len() != struct_.fields.len() {
                        return Err(format!(
                            "struct {type_name} requires {} fields but ScVal contains {}",
                            struct_.fields.len(),
                            values.len()
                        ));
                    }
                    for entry in values.iter() {
                        let ScVal::Symbol(key) = &entry.key else {
                            return Err(format!(
                                "struct {type_name} contains a non-symbol field key"
                            ));
                        };
                        let key = key.to_utf8_string_lossy();
                        let field = struct_
                            .fields
                            .iter()
                            .find(|field| field.name.to_utf8_string_lossy() == key)
                            .ok_or_else(|| {
                                format!("unknown field {key:?} for struct {type_name}")
                            })?;
                        validate_scval_against_type(spec, &entry.val, &field.type_, next)?;
                    }
                    Ok(())
                }
                ScSpecEntry::UdtUnionV0(union) => {
                    let ScVal::Vec(Some(values)) = value else {
                        return mismatch();
                    };
                    let Some(ScVal::Symbol(case_name)) = values.first() else {
                        return Err(format!(
                            "union {type_name} requires a symbol case discriminator"
                        ));
                    };
                    let case_name = case_name.to_utf8_string_lossy();
                    let case = union
                        .cases
                        .iter()
                        .find(|case| match case {
                            stellar_xdr::ScSpecUdtUnionCaseV0::VoidV0(case) => {
                                case.name.to_utf8_string_lossy() == case_name
                            }
                            stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) => {
                                case.name.to_utf8_string_lossy() == case_name
                            }
                        })
                        .ok_or_else(|| {
                            format!("unknown case {case_name:?} for union {type_name}")
                        })?;
                    match case {
                        stellar_xdr::ScSpecUdtUnionCaseV0::VoidV0(_) => {
                            if values.len() == 1 {
                                Ok(())
                            } else {
                                Err(format!(
                                    "void case {case_name:?} for union {type_name} contains an unexpected payload"
                                ))
                            }
                        }
                        stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) => {
                            if values.len() != case.type_.len() + 1 {
                                return Err(format!(
                                    "case {case_name:?} for union {type_name} requires {} payload values but ScVal contains {}",
                                    case.type_.len(),
                                    values.len().saturating_sub(1)
                                ));
                            }
                            for (value, type_def) in values.iter().skip(1).zip(case.type_.iter()) {
                                validate_scval_against_type(spec, value, type_def, next)?;
                            }
                            Ok(())
                        }
                    }
                }
                ScSpecEntry::UdtEnumV0(enum_) => match value {
                    ScVal::U32(value) if enum_.cases.iter().any(|case| case.value == *value) => {
                        Ok(())
                    }
                    _ => mismatch(),
                },
                ScSpecEntry::UdtErrorEnumV0(enum_) => match value {
                    ScVal::Error(stellar_xdr::ScError::Contract(value))
                        if enum_.cases.iter().any(|case| case.value == *value) =>
                    {
                        Ok(())
                    }
                    _ => mismatch(),
                },
                _ => Err(format!(
                    "Contract Spec entry {type_name} is not a user-defined value type"
                )),
            }
        }
        _ => mismatch(),
    }
}

fn contract_argument_review(
    spec: &Spec,
    name: String,
    value_type: String,
    type_def: &ScSpecTypeDef,
    parsed: &ScVal,
) -> Result<ContractArgumentReview, String> {
    validate_scval_against_type(spec, parsed, type_def, 0)
        .map_err(|detail| format!("invalid contract argument --{name} ({value_type}): {detail}"))?;
    ensure_contract_type_normalization_safe(spec, type_def, &mut Vec::new()).map_err(|detail| {
        format!("unable to normalize contract argument --{name} ({value_type}): {detail}")
    })?;
    let value = spec.xdr_to_json(parsed, type_def).map_err(|error| {
        format!("unable to normalize contract argument --{name} ({value_type}): {error}")
    })?;
    let scval_xdr = parsed
        .to_xdr_base64(Limits::depth(CONTRACT_ARGUMENT_XDR_DEPTH_LIMIT))
        .map_err(|error| {
            format!("unable to encode contract argument --{name} as ScVal XDR: {error}")
        })?;
    Ok(ContractArgumentReview {
        name,
        value_type,
        value,
        scval_xdr,
    })
}

fn remove_argument(
    arguments: &mut BTreeMap<String, ContractArgumentSource>,
    spec_name: &str,
) -> Option<ContractArgumentSource> {
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
    address_names: &ContractAddressNames,
) -> Result<ScVal, String> {
    match spec.from_string(value, type_def) {
        Ok(parsed) => return Ok(parsed),
        Err(direct_error) => {
            if matches!(
                type_def,
                ScSpecTypeDef::Address | ScSpecTypeDef::MuxedAddress
            ) {
                if let Some(address) = address_names.get(value.trim().trim_matches('"')) {
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

fn parse_json_argument(
    spec: &Spec,
    name: &str,
    value: &Value,
    type_def: &ScSpecTypeDef,
) -> Result<ScVal, String> {
    let prepared = prepare_json_argument_value(spec, value, type_def).map_err(|detail| {
        format!(
            "invalid value for contract argument --{name}; expected {}: {detail}",
            contract_type_name(type_def)
        )
    })?;
    spec.from_json(&prepared, type_def)
        .map_err(|error| contract_argument_parse_error(spec, name, type_def, error))
}

fn prepare_json_argument_value(
    spec: &Spec,
    value: &Value,
    type_def: &ScSpecTypeDef,
) -> Result<Value, String> {
    match type_def {
        ScSpecTypeDef::Void if value.is_null() => Ok(Value::String("void".to_owned())),
        ScSpecTypeDef::Timepoint | ScSpecTypeDef::Duration if value.is_number() => {
            let key = if matches!(type_def, ScSpecTypeDef::Timepoint) {
                "timepoint"
            } else {
                "duration"
            };
            let mut tagged = serde_json::Map::new();
            tagged.insert(key.to_owned(), Value::String(value.to_string()));
            Ok(Value::Object(tagged))
        }
        ScSpecTypeDef::Error => Err(
            "Contract Spec Error values are runtime error metadata and have no stable composed input form"
                .to_owned(),
        ),
        ScSpecTypeDef::Result(_) => Err(
            "Contract Spec Result values have no stable composed input form in the pinned official parser"
                .to_owned(),
        ),
        ScSpecTypeDef::Option(_inner) if value.is_null() => Ok(Value::Null),
        ScSpecTypeDef::Option(inner) => {
            prepare_json_argument_value(spec, value, &inner.value_type)
        }
        ScSpecTypeDef::Vec(inner) => match value {
            Value::Array(values) => values
                .iter()
                .map(|value| prepare_json_argument_value(spec, value, &inner.element_type))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            _ => Ok(value.clone()),
        },
        ScSpecTypeDef::Map(inner) => match value {
            Value::Object(values) => {
                let mut prepared = serde_json::Map::new();
                for (key, value) in values {
                    let key_value = serde_json::from_str(key)
                        .unwrap_or_else(|_| Value::String(key.clone()));
                    let prepared_key =
                        prepare_json_argument_value(spec, &key_value, &inner.key_type)?;
                    let prepared_key = serde_json::to_string(&prepared_key)
                        .map_err(|error| format!("unable to encode map key {key:?}: {error}"))?;
                    let prepared_key = if spec.from_string(&prepared_key, &inner.key_type).is_ok() {
                        prepared_key
                    } else {
                        key.clone()
                    };
                    prepared.insert(
                        prepared_key,
                        prepare_json_argument_value(spec, value, &inner.value_type)?,
                    );
                }
                Ok(Value::Object(prepared))
            }
            _ => Ok(value.clone()),
        },
        ScSpecTypeDef::Tuple(inner) => match value {
            Value::Array(values) => {
                if values.len() != inner.value_types.len() {
                    return Err(format!(
                        "tuple requires {} values but {} were provided",
                        inner.value_types.len(),
                        values.len()
                    ));
                }
                values
                    .iter()
                    .zip(inner.value_types.iter())
                    .map(|(value, type_def)| prepare_json_argument_value(spec, value, type_def))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            }
            _ => Ok(value.clone()),
        },
        ScSpecTypeDef::Udt(inner) => {
            let type_name = inner.name.to_utf8_string_lossy();
            match spec
                .find(&type_name)
                .map_err(|error| format!("unable to resolve user-defined type {type_name}: {error}"))?
            {
                ScSpecEntry::UdtStructV0(struct_) => {
                    prepare_json_struct_value(spec, value, struct_)
                }
                ScSpecEntry::UdtUnionV0(union) => prepare_json_union_value(spec, value, union),
                ScSpecEntry::UdtEnumV0(enum_) => {
                    validate_json_enum_shape(value, enum_)?;
                    Ok(value.clone())
                }
                ScSpecEntry::UdtErrorEnumV0(_) => Err(format!(
                    "error enum {type_name} is contract error metadata and cannot be supplied as a function argument"
                )),
                _ => Err(format!(
                    "Contract Spec entry {type_name} is not a user-defined value type"
                )),
            }
        }
        _ => Ok(value.clone()),
    }
}

fn prepare_json_struct_value(
    spec: &Spec,
    value: &Value,
    struct_: &stellar_xdr::ScSpecUdtStructV0,
) -> Result<Value, String> {
    let type_name = struct_.name.to_utf8_string_lossy();
    let tuple_struct = struct_
        .fields
        .iter()
        .any(|field| field.name.to_utf8_string_lossy() == "0");
    if tuple_struct {
        return match value {
            Value::Array(values) => {
                if values.len() != struct_.fields.len() {
                    return Err(format!(
                        "tuple struct {type_name} requires {} values but {} were provided",
                        struct_.fields.len(),
                        values.len()
                    ));
                }
                values
                    .iter()
                    .zip(struct_.fields.iter())
                    .map(|(value, field)| prepare_json_argument_value(spec, value, &field.type_))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            }
            Value::Object(values) => {
                if values.len() != struct_.fields.len() {
                    return Err(format!(
                        "tuple struct {type_name} requires {} values but {} were provided",
                        struct_.fields.len(),
                        values.len()
                    ));
                }
                let mut prepared = serde_json::Map::new();
                for field in &struct_.fields {
                    let field_name = field.name.to_utf8_string_lossy();
                    let field_value = values.get(&field_name).ok_or_else(|| {
                        format!("missing field {field_name:?} for tuple struct {type_name}")
                    })?;
                    prepared.insert(
                        field_name,
                        prepare_json_argument_value(spec, field_value, &field.type_)?,
                    );
                }
                Ok(Value::Object(prepared))
            }
            _ => Err(format!(
                "tuple struct {type_name} requires a JSON array or numeric-key object"
            )),
        };
    }

    let Value::Object(values) = value else {
        return Err(format!("struct {type_name} requires a JSON object"));
    };
    for key in values.keys() {
        if !struct_
            .fields
            .iter()
            .any(|field| field.name.to_utf8_string_lossy() == *key)
        {
            return Err(format!("unknown field {key:?} for struct {type_name}"));
        }
    }
    let mut prepared = serde_json::Map::new();
    for field in &struct_.fields {
        let field_name = field.name.to_utf8_string_lossy();
        let field_value = values
            .get(&field_name)
            .ok_or_else(|| format!("missing field {field_name:?} for struct {type_name}"))?;
        prepared.insert(
            field_name,
            prepare_json_argument_value(spec, field_value, &field.type_)?,
        );
    }
    Ok(Value::Object(prepared))
}

fn prepare_json_union_value(
    spec: &Spec,
    value: &Value,
    union: &stellar_xdr::ScSpecUdtUnionV0,
) -> Result<Value, String> {
    let type_name = union.name.to_utf8_string_lossy();
    let (case_name, payload) = match value {
        Value::String(case_name) => (case_name.as_str(), None),
        Value::Object(values) if values.len() == 1 => {
            let (case_name, payload) = values.iter().next().expect("single union case");
            (case_name.as_str(), Some(payload))
        }
        _ => {
            return Err(format!(
                "union {type_name} requires a void-case string or a single-key case object"
            ))
        }
    };
    let case = union
        .cases
        .iter()
        .find(|case| match case {
            stellar_xdr::ScSpecUdtUnionCaseV0::VoidV0(case) => {
                case.name.to_utf8_string_lossy() == case_name
            }
            stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) => {
                case.name.to_utf8_string_lossy() == case_name
            }
        })
        .ok_or_else(|| format!("unknown case {case_name:?} for union {type_name}"))?;

    match case {
        stellar_xdr::ScSpecUdtUnionCaseV0::VoidV0(_) => {
            if payload.is_some() {
                return Err(format!(
                    "void case {case_name:?} for union {type_name} must be a JSON string"
                ));
            }
            Ok(Value::String(case_name.to_owned()))
        }
        stellar_xdr::ScSpecUdtUnionCaseV0::TupleV0(case) => {
            let payload = payload.ok_or_else(|| {
                format!("case {case_name:?} for union {type_name} requires a payload")
            })?;
            let prepared_payload = if case.type_.len() == 1 {
                prepare_json_argument_value(spec, payload, &case.type_[0])?
            } else {
                let Value::Array(values) = payload else {
                    return Err(format!(
                        "case {case_name:?} for union {type_name} requires a JSON array with {} values",
                        case.type_.len()
                    ));
                };
                if values.len() != case.type_.len() {
                    return Err(format!(
                        "case {case_name:?} for union {type_name} requires {} values but {} were provided",
                        case.type_.len(),
                        values.len()
                    ));
                }
                Value::Array(
                    values
                        .iter()
                        .zip(case.type_.iter())
                        .map(|(value, type_def)| prepare_json_argument_value(spec, value, type_def))
                        .collect::<Result<Vec<_>, _>>()?,
                )
            };
            Ok(Value::Object(
                [(case_name.to_owned(), prepared_payload)]
                    .into_iter()
                    .collect(),
            ))
        }
    }
}

fn validate_json_enum_shape(
    value: &Value,
    enum_: &stellar_xdr::ScSpecUdtEnumV0,
) -> Result<(), String> {
    let type_name = enum_.name.to_utf8_string_lossy();
    let value = value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| format!("enum {type_name} requires its numeric u32 case value"))?;
    if enum_.cases.iter().any(|case| case.value == value) {
        Ok(())
    } else {
        Err(format!("unknown numeric case {value} for enum {type_name}"))
    }
}

fn parse_scval_xdr_argument(name: &str, value: &str) -> Result<ScVal, String> {
    ScVal::from_xdr_base64(
        value.trim(),
        Limits::depth(CONTRACT_ARGUMENT_XDR_DEPTH_LIMIT),
    )
    .map_err(|error| format!("invalid ScVal XDR for contract argument --{name}: {error}"))
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
    pub executable: ContractExecutableObservation,
    pub metadata: Vec<ContractMetadataEntry>,
    pub capabilities: ContractCapabilities,
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
    fn from_soroban(
        review: &SorobanReview,
        executable: ContractExecutableObservation,
        metadata: Vec<ContractMetadataEntry>,
        capabilities: ContractCapabilities,
        arguments: Vec<ContractArgumentReview>,
    ) -> Self {
        Self {
            wallet_name: review.wallet_name.clone(),
            fee_payer: review.fee_payer.clone(),
            operation_source: review.operation_source.clone(),
            contract_id: review.contract_id.clone(),
            executable,
            metadata,
            capabilities,
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
    pub executable: ContractExecutableObservation,
    pub metadata: Vec<ContractMetadataEntry>,
    pub capabilities: ContractCapabilities,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentReview>,
    pub output: Option<Value>,
    pub output_xdr: Option<String>,
    pub simulation_ledger: u32,
    pub network: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractSimulationEffects {
    pub read_write_entry_count: usize,
    pub archived_entry_count: usize,
    pub published_event_count: usize,
    pub authorization_entry_count: usize,
    pub restore_required: bool,
}

impl ContractSimulationEffects {
    pub fn requires_send(&self) -> bool {
        self.restore_required
            || self.read_write_entry_count > 0
            || self.archived_entry_count > 0
            || self.published_event_count > 0
            || self.authorization_entry_count > 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContractSimulationResult {
    pub contract_id: String,
    pub executable: ContractExecutableObservation,
    pub metadata: Vec<ContractMetadataEntry>,
    pub capabilities: ContractCapabilities,
    pub function_name: String,
    pub arguments: Vec<ContractArgumentReview>,
    pub output: Option<Value>,
    pub output_xdr: Option<String>,
    pub simulation_ledger: u32,
    pub network: String,
    pub effects: ContractSimulationEffects,
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
        snapshot.metadata,
        &snapshot.entries,
    ))
}

fn ensure_contract_executable_unchanged(
    before: &ContractExecutableObservation,
    after: &ContractExecutableObservation,
) -> Result<(), String> {
    if before == after {
        return Ok(());
    }
    Err(format!(
        "contract executable changed while preparing the invocation: {} -> {}; inspect the deployed contract and prepare the call again",
        executable_identity(before),
        executable_identity(after)
    ))
}

fn executable_identity(observation: &ContractExecutableObservation) -> String {
    match &observation.wasm_hash {
        Some(hash) => format!("{}:{hash}", observation.kind.as_str()),
        None => observation.kind.as_str().to_owned(),
    }
}

pub(crate) async fn prepare_contract_invoke(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    request: ContractInvokeRequest,
) -> Result<PreparedContractInvoke, String> {
    let snapshot = rpc.contract_spec_snapshot(&request.contract_id).await?;
    let capabilities = ContractCapabilities::from_spec(
        &snapshot.executable,
        &snapshot.metadata,
        &snapshot.entries,
    );
    let (low_level_request, arguments) = request.resolve(&snapshot.entries)?;
    let prepared = prepare_soroban_invoke(storage, rpc, low_level_request).await?;
    let executable = rpc
        .contract_executable_observation(&request.contract_id)
        .await?;
    ensure_contract_executable_unchanged(&snapshot.executable, &executable)?;
    let review = ContractInvokeReview::from_soroban(
        &prepared.review,
        executable,
        snapshot.metadata,
        capabilities,
        arguments,
    );
    Ok(PreparedContractInvoke { review, prepared })
}

pub(crate) async fn simulate_contract_invoke(
    rpc: &RpcGateway,
    request: ContractInvokeRequest,
) -> Result<ContractSimulationResult, String> {
    let snapshot = rpc.contract_spec_snapshot(&request.contract_id).await?;
    let capabilities = ContractCapabilities::from_spec(
        &snapshot.executable,
        &snapshot.metadata,
        &snapshot.entries,
    );
    let (low_level_request, arguments) = request.resolve(&snapshot.entries)?;
    let simulation = simulate_soroban_invoke(rpc, &low_level_request).await?;
    validate_contract_simulation_preview(&simulation)?;
    let (output, output_xdr) =
        decode_simulation_output(&snapshot.entries, &request.function_name, &simulation)?;
    let effects = simulation_effects(&simulation)?;
    Ok(ContractSimulationResult {
        contract_id: request.contract_id,
        executable: snapshot.executable,
        metadata: snapshot.metadata,
        capabilities,
        function_name: request.function_name,
        arguments,
        output,
        output_xdr,
        simulation_ledger: simulation.latest_ledger,
        network: rpc.network().to_owned(),
        effects,
    })
}

pub(crate) async fn prepare_contract_invoke_outcome(
    storage: &WalletStorage,
    rpc: &RpcGateway,
    request: ContractInvokeRequest,
) -> Result<ContractInvokePreparation, String> {
    let snapshot = rpc.contract_spec_snapshot(&request.contract_id).await?;
    let capabilities = ContractCapabilities::from_spec(
        &snapshot.executable,
        &snapshot.metadata,
        &snapshot.entries,
    );
    let (low_level_request, arguments) = request.resolve(&snapshot.entries)?;
    let simulation = simulate_soroban_invoke(rpc, &low_level_request).await?;
    validate_soroban_simulation(&simulation)?;

    if simulation_requires_send(&simulation)? {
        let prepared = prepare_soroban_invoke(storage, rpc, low_level_request).await?;
        let executable = rpc
            .contract_executable_observation(&request.contract_id)
            .await?;
        ensure_contract_executable_unchanged(&snapshot.executable, &executable)?;
        let review = ContractInvokeReview::from_soroban(
            &prepared.review,
            executable,
            snapshot.metadata,
            capabilities,
            arguments,
        );
        return Ok(ContractInvokePreparation::Transaction(
            PreparedContractInvoke { review, prepared },
        ));
    }

    let (output, output_xdr) =
        decode_simulation_output(&snapshot.entries, &request.function_name, &simulation)?;
    Ok(ContractInvokePreparation::ReadOnly(ContractReadResult {
        contract_id: request.contract_id,
        executable: snapshot.executable,
        metadata: snapshot.metadata,
        capabilities,
        function_name: request.function_name,
        arguments,
        output,
        output_xdr,
        simulation_ledger: simulation.latest_ledger,
        network: rpc.network().to_owned(),
    }))
}

fn validate_contract_simulation_preview(
    simulation: &SimulateTransactionResponse,
) -> Result<(), String> {
    if let Some(error) = simulation.error.as_deref() {
        return Err(format!("Soroban transaction simulation failed: {error}"));
    }
    let results = simulation
        .results()
        .map_err(|error| format!("Stellar RPC returned invalid simulation result: {error}"))?;
    if results.len() != 1 {
        return Err(format!(
            "Soroban simulation returned {} host-function results; expected one",
            results.len()
        ));
    }
    Ok(())
}

fn simulation_effects(
    simulation: &SimulateTransactionResponse,
) -> Result<ContractSimulationEffects, String> {
    let transaction_data = simulation
        .transaction_data()
        .map_err(|error| format!("Stellar RPC returned invalid transaction data: {error}"))?;
    let archived_entry_count = match &transaction_data.ext {
        SorobanTransactionDataExt::V0 => 0,
        SorobanTransactionDataExt::V1(resources) => resources.archived_soroban_entries.len(),
    };
    let published_event_count = simulation
        .events()
        .map_err(|error| format!("Stellar RPC returned invalid simulation events: {error}"))?
        .iter()
        .filter(
            |DiagnosticEvent {
                 event: ContractEvent { type_, .. },
                 ..
             }| matches!(type_, ContractEventType::Contract),
        )
        .count();
    let authorization_entry_count = simulation
        .results()
        .map_err(|error| format!("Stellar RPC returned invalid simulation result: {error}"))?
        .iter()
        .map(|result| result.auth.len())
        .sum();
    Ok(ContractSimulationEffects {
        read_write_entry_count: transaction_data.resources.footprint.read_write.len(),
        archived_entry_count,
        published_event_count,
        authorization_entry_count,
        restore_required: simulation.restore_preamble.is_some(),
    })
}

fn simulation_requires_send(simulation: &SimulateTransactionResponse) -> Result<bool, String> {
    Ok(simulation_effects(simulation)?.requires_send())
}

fn decode_simulation_output(
    spec_entries: &[ScSpecEntry],
    function_name: &str,
    simulation: &SimulateTransactionResponse,
) -> Result<(Option<Value>, Option<String>), String> {
    let spec = Spec::new(spec_entries);
    let function = find_function(&spec, function_name)?;
    let Some(output_type) = function.outputs.first() else {
        return Ok((None, None));
    };
    let results = simulation
        .results()
        .map_err(|error| format!("Stellar RPC returned invalid simulation result: {error}"))?;
    let result = results
        .first()
        .ok_or_else(|| "Soroban simulation did not return a contract result".to_owned())?;
    validate_scval_against_type(&spec, &result.xdr, output_type, 0).map_err(|detail| {
        format!("contract return value does not match its Contract Spec: {detail}")
    })?;
    ensure_contract_type_normalization_safe(&spec, output_type, &mut Vec::new())
        .map_err(|detail| format!("unable to normalize contract return value: {detail}"))?;
    let value = spec
        .xdr_to_json(&result.xdr, output_type)
        .map_err(|error| format!("unable to decode contract return value: {error}"))?;
    let scval_xdr = result
        .xdr
        .to_xdr_base64(Limits::depth(CONTRACT_ARGUMENT_XDR_DEPTH_LIMIT))
        .map_err(|error| format!("unable to encode contract return value as ScVal XDR: {error}"))?;
    Ok((Some(value), Some(scval_xdr)))
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
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::json;
    use stellar_rpc_client::{RestorePreamble, SimulateHostFunctionResultRaw};
    use stellar_strkey::Contract as StrkeyContract;
    use stellar_xdr::{
        ContractDataDurability, ContractId, Hash, InvokeContractArgs, LedgerFootprint, LedgerKey,
        LedgerKeyContractData, ScAddress, ScSpecFunctionInputV0, ScSpecFunctionV0,
        ScSpecTypeBytesN, ScSpecTypeMap, ScSpecTypeOption, ScSpecTypeResult, ScSpecTypeTuple,
        ScSpecTypeUdt, ScSpecTypeVec, ScSpecUdtEnumCaseV0, ScSpecUdtEnumV0,
        ScSpecUdtErrorEnumCaseV0, ScSpecUdtErrorEnumV0, ScSpecUdtStructFieldV0, ScSpecUdtStructV0,
        ScSpecUdtUnionCaseTupleV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionCaseVoidV0,
        ScSpecUdtUnionV0, ScSymbol, SorobanAuthorizationEntry, SorobanAuthorizedFunction,
        SorobanAuthorizedInvocation, SorobanCredentials, SorobanResources, SorobanResourcesExtV0,
        SorobanTransactionData, SorobanTransactionDataExt, StringM, VecM, WriteXdr,
    };

    use super::*;

    const ACCOUNT: &str = "GDLVVGABQKYQVN6VJP7NHSLEA45A5YLS6PNKMIZFV4BBU2HXA5IRVHUR";
    const OTHER_ACCOUNT: &str = "GAXUGZINCMWFE5WPBMF4H75RYIH522TEGLZHGI7QXRDNGLEUFZJ4RWNY";

    fn simulation_response(read_write: Vec<LedgerKey>) -> SimulateTransactionResponse {
        let transaction_data = SorobanTransactionData {
            resources: SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::try_from(read_write).unwrap(),
                },
                instructions: 1,
                disk_read_bytes: 0,
                write_bytes: 0,
            },
            resource_fee: 0,
            ext: SorobanTransactionDataExt::V0,
        };
        SimulateTransactionResponse {
            results: vec![SimulateHostFunctionResultRaw {
                auth: Vec::new(),
                xdr: STANDARD.encode(ScVal::Void.to_xdr(Limits::none()).unwrap()),
            }],
            transaction_data: STANDARD.encode(transaction_data.to_xdr(Limits::none()).unwrap()),
            latest_ledger: 123_456,
            ..Default::default()
        }
    }

    fn persistent_contract_data_key() -> LedgerKey {
        LedgerKey::ContractData(LedgerKeyContractData {
            contract: ScAddress::Contract(ContractId(Hash([0; 32]))),
            key: ScVal::U32(7),
            durability: ContractDataDurability::Persistent,
        })
    }

    fn source_authorization_entry() -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials: SorobanCredentials::SourceAccount,
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: ScAddress::Contract(ContractId(Hash([0; 32]))),
                    function_name: ScSymbol::try_from("read").unwrap(),
                    args: VecM::default(),
                }),
                sub_invocations: VecM::default(),
            },
        }
    }

    fn function_entry(name: &str, inputs: &[(&str, ScSpecTypeDef)]) -> ScSpecEntry {
        function_entry_with_outputs(name, inputs, &[])
    }

    fn function_entry_with_outputs(
        name: &str,
        inputs: &[(&str, ScSpecTypeDef)],
        outputs: &[ScSpecTypeDef],
    ) -> ScSpecEntry {
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
            outputs: VecM::try_from(outputs.to_vec()).unwrap(),
        })
    }

    fn sep41_entries() -> Vec<ScSpecEntry> {
        vec![
            function_entry_with_outputs(
                "allowance",
                &[
                    ("from", ScSpecTypeDef::Address),
                    ("spender", ScSpecTypeDef::Address),
                ],
                &[ScSpecTypeDef::I128],
            ),
            function_entry(
                "approve",
                &[
                    ("from", ScSpecTypeDef::Address),
                    ("spender", ScSpecTypeDef::Address),
                    ("amount", ScSpecTypeDef::I128),
                    ("live_until_ledger", ScSpecTypeDef::U32),
                ],
            ),
            function_entry_with_outputs(
                "balance",
                &[("id", ScSpecTypeDef::Address)],
                &[ScSpecTypeDef::I128],
            ),
            function_entry(
                "transfer",
                &[
                    ("from", ScSpecTypeDef::Address),
                    ("to", ScSpecTypeDef::MuxedAddress),
                    ("amount", ScSpecTypeDef::I128),
                ],
            ),
            function_entry(
                "transfer_from",
                &[
                    ("spender", ScSpecTypeDef::Address),
                    ("from", ScSpecTypeDef::Address),
                    ("to", ScSpecTypeDef::Address),
                    ("amount", ScSpecTypeDef::I128),
                ],
            ),
            function_entry(
                "burn",
                &[
                    ("from", ScSpecTypeDef::Address),
                    ("amount", ScSpecTypeDef::I128),
                ],
            ),
            function_entry(
                "burn_from",
                &[
                    ("spender", ScSpecTypeDef::Address),
                    ("from", ScSpecTypeDef::Address),
                    ("amount", ScSpecTypeDef::I128),
                ],
            ),
            function_entry_with_outputs("decimals", &[], &[ScSpecTypeDef::U32]),
            function_entry_with_outputs("name", &[], &[ScSpecTypeDef::String]),
            function_entry_with_outputs("symbol", &[], &[ScSpecTypeDef::String]),
        ]
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

    fn map_of(key: ScSpecTypeDef, value: ScSpecTypeDef) -> ScSpecTypeDef {
        ScSpecTypeDef::Map(Box::new(ScSpecTypeMap {
            key_type: Box::new(key),
            value_type: Box::new(value),
        }))
    }

    fn result_of(ok: ScSpecTypeDef, error: ScSpecTypeDef) -> ScSpecTypeDef {
        ScSpecTypeDef::Result(Box::new(ScSpecTypeResult {
            ok_type: Box::new(ok),
            error_type: Box::new(error),
        }))
    }

    fn udt(name: &str) -> ScSpecTypeDef {
        ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: StringM::try_from(name).unwrap(),
        })
    }

    fn tuple_of(types: Vec<ScSpecTypeDef>) -> ScSpecTypeDef {
        ScSpecTypeDef::Tuple(Box::new(ScSpecTypeTuple {
            value_types: VecM::try_from(types).unwrap(),
        }))
    }

    fn aqua_swaps_chain_type() -> ScSpecTypeDef {
        vec_of(tuple_of(vec![
            vec_of(ScSpecTypeDef::Address),
            ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 32 }),
            ScSpecTypeDef::Address,
        ]))
    }

    fn composer_boundary_entries() -> Vec<ScSpecEntry> {
        vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("Route").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("destination").unwrap(),
                        type_: ScSpecTypeDef::Address,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("hops").unwrap(),
                        type_: vec_of(ScSpecTypeDef::Address),
                    },
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("Action").unwrap(),
                cases: VecM::try_from(vec![
                    ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("None").unwrap(),
                    }),
                    ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("Transfer").unwrap(),
                        type_: VecM::try_from(vec![ScSpecTypeDef::Address, udt("Route")]).unwrap(),
                    }),
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("Mode").unwrap(),
                cases: VecM::try_from(vec![
                    ScSpecUdtEnumCaseV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("Exact").unwrap(),
                        value: 1,
                    },
                    ScSpecUdtEnumCaseV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("Flexible").unwrap(),
                        value: 2,
                    },
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("RouteError").unwrap(),
                cases: VecM::try_from(vec![ScSpecUdtErrorEnumCaseV0 {
                    doc: StringM::default(),
                    name: StringM::try_from("BadRoute").unwrap(),
                    value: 7,
                }])
                .unwrap(),
            }),
            function_entry(
                "compose_all",
                &[
                    ("action", udt("Action")),
                    ("mode", udt("Mode")),
                    ("routes", map_of(ScSpecTypeDef::Address, udt("Route"))),
                ],
            ),
            function_entry("accept_error", &[("error", udt("RouteError"))]),
        ]
    }

    const AQUA_TESTNET_SWAP_CHAIN_XDR: &str = "AAAAEAAAAAEAAAAEAAAAEAAAAAEAAAADAAAAEAAAAAEAAAACAAAAEgAAAAEzHHTSKEtx/P4wChB3BsQmO9OQVZTMFsEGS0FSLd2VfwAAABIAAAAB15KLcsJwPM/q9+uf9O9NUEpVqLl5/JtFDqLIQrTRzmEAAAANAAAAIEkTYzg4CRHdqPOcwUH82/sAEUn5qf3jDH+moJjmqW/WAAAAEgAAAAHXkotywnA8z+r365/0701QSlWouXn8m0UOoshCtNHOYQAAABAAAAABAAAAAwAAABAAAAABAAAAAgAAABIAAAABUEXNXsBymnaP1a0CUFhS308Cjc6DDlrFIgm6SEg7LwEAAAASAAAAAdeSi3LCcDzP6vfrn/TvTVBKVai5efybRQ6iyEK00c5hAAAADQAAACCy4C/PymyW+K1cvYTneEp3ezbZyWokWUAsT0WEYqq38AAAABIAAAABUEXNXsBymnaP1a0CUFhS308Cjc6DDlrFIgm6SEg7LwEAAAAQAAAAAQAAAAMAAAAQAAAAAQAAAAIAAAASAAAAAVBFzV7Acpp2j9WtAlBYUt9PAo3Ogw5axSIJukhIOy8BAAAAEgAAAAHbWFucFs4F4bWHJODfHSWxM1cXv5LyScuAwBSjpRdXOAAAAA0AAAAgmsepzeI6wq2hEQXuqkLkPC6oMyygqo9B9Y1xYCdNcY4AAAASAAAAAdtYW5wWzgXhtYck4N8dJbEzVxe/kvJJy4DAFKOlF1c4AAAAEAAAAAEAAAADAAAAEAAAAAEAAAACAAAAEgAAAAFX5Q9LKxYKKKWzW3s65W/2YF1kByzExXDq/+GzE2bVaAAAABIAAAAB21hbnBbOBeG1hyTg3x0lsTNXF7+S8knLgMAUo6UXVzgAAAANAAAAIJrHqc3iOsKtoREF7qpC5DwuqDMsoKqPQfWNcWAnTXGOAAAAEgAAAAFX5Q9LKxYKKKWzW3s65W/2YF1kByzExXDq/+GzE2bVaA==";

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
            vec![ContractMetadataEntry {
                key: "binver".to_owned(),
                value: "2.3.7".to_owned(),
            }],
            &entries,
        );
        let function = interface.function("batch").unwrap();
        assert_eq!(interface.executable.kind, ContractExecutableKind::Wasm);
        assert_eq!(interface.metadata[0].key, "binver");
        assert_eq!(interface.metadata[0].value, "2.3.7");
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
    fn contract_interface_preserves_recursive_types_and_all_udt_definitions() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::try_from("route definition").unwrap(),
                lib: StringM::try_from("routing").unwrap(),
                name: StringM::try_from("Route").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::try_from("destination").unwrap(),
                        name: StringM::try_from("destination").unwrap(),
                        type_: ScSpecTypeDef::Address,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::try_from("intermediate hops").unwrap(),
                        name: StringM::try_from("hops").unwrap(),
                        type_: vec_of(ScSpecTypeDef::Address),
                    },
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 {
                doc: StringM::try_from("action choice").unwrap(),
                lib: StringM::try_from("routing").unwrap(),
                name: StringM::try_from("Action").unwrap(),
                cases: VecM::try_from(vec![
                    ScSpecUdtUnionCaseV0::VoidV0(ScSpecUdtUnionCaseVoidV0 {
                        doc: StringM::try_from("do nothing").unwrap(),
                        name: StringM::try_from("None").unwrap(),
                    }),
                    ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                        doc: StringM::try_from("route transfer").unwrap(),
                        name: StringM::try_from("Transfer").unwrap(),
                        type_: VecM::try_from(vec![ScSpecTypeDef::Address, udt("Route")]).unwrap(),
                    }),
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 {
                doc: StringM::try_from("execution mode").unwrap(),
                lib: StringM::try_from("routing").unwrap(),
                name: StringM::try_from("Mode").unwrap(),
                cases: VecM::try_from(vec![
                    ScSpecUdtEnumCaseV0 {
                        doc: StringM::try_from("exact").unwrap(),
                        name: StringM::try_from("Exact").unwrap(),
                        value: 1,
                    },
                    ScSpecUdtEnumCaseV0 {
                        doc: StringM::try_from("flexible").unwrap(),
                        name: StringM::try_from("Flexible").unwrap(),
                        value: 2,
                    },
                ])
                .unwrap(),
            }),
            ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 {
                doc: StringM::try_from("routing errors").unwrap(),
                lib: StringM::try_from("routing").unwrap(),
                name: StringM::try_from("RouteError").unwrap(),
                cases: VecM::try_from(vec![ScSpecUdtErrorEnumCaseV0 {
                    doc: StringM::try_from("bad route").unwrap(),
                    name: StringM::try_from("BadRoute").unwrap(),
                    value: 7,
                }])
                .unwrap(),
            }),
            function_entry_with_outputs(
                "compose",
                &[(
                    "routes",
                    option_of(map_of(ScSpecTypeDef::Address, vec_of(udt("Route")))),
                )],
                &[result_of(udt("Mode"), udt("RouteError"))],
            ),
        ];

        let interface = ContractInterface::from_spec(
            "CCONTRACT",
            ContractExecutableObservation {
                kind: ContractExecutableKind::Wasm,
                wasm_hash: None,
            },
            vec![],
            &entries,
        );
        let function = interface.function("compose").unwrap();
        assert_eq!(
            function.inputs[0].value_type.abi,
            ContractAbiType::Option(Box::new(ContractAbiType::Map {
                key: Box::new(ContractAbiType::Primitive("address".to_owned())),
                value: Box::new(ContractAbiType::Vec(Box::new(ContractAbiType::Udt(
                    "Route".to_owned()
                )))),
            }))
        );
        assert_eq!(
            function.outputs[0].abi,
            ContractAbiType::Result {
                ok: Box::new(ContractAbiType::Udt("Mode".to_owned())),
                error: Box::new(ContractAbiType::Udt("RouteError".to_owned())),
            }
        );
        assert_eq!(interface.user_types.len(), 4);
        assert!(matches!(
            &interface.user_types[0],
            ContractUserType::Struct { name, lib, fields, .. }
                if name == "Route"
                    && lib == "routing"
                    && fields.len() == 2
                    && fields[1].value_type
                        == ContractAbiType::Vec(Box::new(ContractAbiType::Primitive(
                            "address".to_owned()
                        )))
        ));
        assert!(matches!(
            &interface.user_types[1],
            ContractUserType::Union { name, cases, .. }
                if name == "Action"
                    && matches!(cases[0].payload, ContractAbiUnionCasePayload::Void)
                    && matches!(
                        &cases[1].payload,
                        ContractAbiUnionCasePayload::Tuple(values)
                            if values == &vec![
                                ContractAbiType::Primitive("address".to_owned()),
                                ContractAbiType::Udt("Route".to_owned()),
                            ]
                    )
        ));
        assert!(matches!(
            &interface.user_types[2],
            ContractUserType::Enum { name, cases, .. }
                if name == "Mode" && cases[1].name == "Flexible" && cases[1].value == 2
        ));
        assert!(matches!(
            &interface.user_types[3],
            ContractUserType::ErrorEnum { name, cases, .. }
                if name == "RouteError" && cases[0].name == "BadRoute" && cases[0].value == 7
        ));
    }

    #[test]
    fn invoke_review_preserves_executable_observation() {
        let executable = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("cd".repeat(32)),
        };
        let review = SorobanReview {
            wallet_name: "wallet".to_owned(),
            fee_payer: ACCOUNT.to_owned(),
            operation_source: ACCOUNT.to_owned(),
            contract_id: "CCONTRACT".to_owned(),
            function_name: "balance".to_owned(),
            argument_count: 0,
            authorizers: Vec::new(),
            credential_types: Vec::new(),
            auth_entry_count: 0,
            total_fee_stroops: 100,
            resource_fee_stroops: 0,
            inclusion_fee_stroops: 100,
            min_resource_fee_stroops: 0,
            simulation_ledger: 123,
            authorization_expiration_ledger: None,
            network: "testnet".to_owned(),
            transaction_hash: "deadbeef".to_owned(),
        };
        let metadata = vec![ContractMetadataEntry {
            key: "sep".to_owned(),
            value: "41".to_owned(),
        }];
        let capabilities =
            ContractCapabilities::from_spec(&executable, &metadata, &sep41_entries());
        let result = ContractInvokeReview::from_soroban(
            &review,
            executable.clone(),
            metadata.clone(),
            capabilities.clone(),
            Vec::new(),
        );
        assert_eq!(result.executable, executable);
        assert_eq!(result.metadata, metadata);
        assert_eq!(result.capabilities, capabilities);
    }

    #[test]
    fn sep41_capability_keeps_declaration_and_interface_evidence_separate() {
        let executable = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("ab".repeat(32)),
        };
        let metadata = vec![
            ContractMetadataEntry {
                key: "sep".to_owned(),
                value: "40".to_owned(),
            },
            ContractMetadataEntry {
                key: "sep".to_owned(),
                value: "47, 41".to_owned(),
            },
        ];
        let compatible = ContractCapabilities::from_spec(&executable, &metadata, &sep41_entries());
        assert!(!compatible.sep41.native_sac);
        assert!(compatible.sep41.sep47_declared);
        assert!(compatible.sep41.current_interface_compatible);

        let declared_only = ContractCapabilities::from_spec(&executable, &metadata, &[]);
        assert!(declared_only.sep41.sep47_declared);
        assert!(!declared_only.sep41.current_interface_compatible);

        let shape_only = ContractCapabilities::from_spec(&executable, &[], &sep41_entries());
        assert!(!shape_only.sep41.sep47_declared);
        assert!(shape_only.sep41.current_interface_compatible);
    }

    #[test]
    fn sep47_requires_the_canonical_sep_41_identifier() {
        let metadata = vec![ContractMetadataEntry {
            key: "sep".to_owned(),
            value: "041,410,41x".to_owned(),
        }];
        assert!(!metadata_declares_sep(&metadata, "41"));
        assert!(metadata_declares_sep(
            &[ContractMetadataEntry {
                key: "sep".to_owned(),
                value: "40,41".to_owned(),
            }],
            "41"
        ));
    }

    #[test]
    fn current_stellar_asset_spec_is_sep41_interface_compatible() {
        let entries = soroban_spec::read::parse_raw(stellar_asset_spec::xdr()).unwrap();
        let executable = ContractExecutableObservation {
            kind: ContractExecutableKind::StellarAsset,
            wasm_hash: None,
        };
        let capabilities = ContractCapabilities::from_spec(&executable, &[], &entries);
        assert!(capabilities.sep41.native_sac);
        assert!(!capabilities.sep41.sep47_declared);
        assert!(capabilities.sep41.current_interface_compatible);
    }

    #[test]
    fn contract_metadata_preserves_wasm_key_value_entries() {
        let entry = ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "home_domain".try_into().unwrap(),
            val: "example.org".try_into().unwrap(),
        });
        assert_eq!(
            ContractMetadataEntry::from_xdr(&entry),
            ContractMetadataEntry {
                key: "home_domain".to_owned(),
                value: "example.org".to_owned(),
            }
        );
    }

    #[test]
    fn simulation_effects_keep_pure_read_non_sending() {
        let simulation = simulation_response(Vec::new());
        let effects = simulation_effects(&simulation).unwrap();
        assert_eq!(
            effects,
            ContractSimulationEffects {
                read_write_entry_count: 0,
                archived_entry_count: 0,
                published_event_count: 0,
                authorization_entry_count: 0,
                restore_required: false,
            }
        );
        assert!(!effects.requires_send());
    }

    #[test]
    fn simulation_effects_expose_restore_preamble() {
        let mut simulation = simulation_response(vec![persistent_contract_data_key()]);
        simulation.restore_preamble = Some(RestorePreamble {
            transaction_data: simulation.transaction_data.clone(),
            min_resource_fee: 100,
        });

        validate_contract_simulation_preview(&simulation).unwrap();
        let effects = simulation_effects(&simulation).unwrap();
        assert_eq!(effects.read_write_entry_count, 1);
        assert_eq!(effects.archived_entry_count, 0);
        assert!(effects.restore_required);
        assert!(effects.requires_send());
    }

    #[test]
    fn simulation_effects_expose_archived_entries_without_restore_preamble() {
        let mut simulation = simulation_response(vec![persistent_contract_data_key()]);
        let mut transaction_data = simulation.transaction_data().unwrap();
        transaction_data.ext = SorobanTransactionDataExt::V1(SorobanResourcesExtV0 {
            archived_soroban_entries: VecM::try_from(vec![0]).unwrap(),
        });
        simulation.transaction_data =
            STANDARD.encode(transaction_data.to_xdr(Limits::none()).unwrap());

        let effects = simulation_effects(&simulation).unwrap();
        assert_eq!(effects.read_write_entry_count, 1);
        assert_eq!(effects.archived_entry_count, 1);
        assert!(!effects.restore_required);
        assert!(effects.requires_send());
    }

    #[test]
    fn simulation_effects_count_published_contract_events() {
        let mut simulation = simulation_response(Vec::new());
        let event = DiagnosticEvent {
            in_successful_contract_call: true,
            event: ContractEvent {
                type_: ContractEventType::Contract,
                ..Default::default()
            },
        };
        simulation.events = vec![STANDARD.encode(event.to_xdr(Limits::none()).unwrap())];

        let effects = simulation_effects(&simulation).unwrap();
        assert_eq!(effects.published_event_count, 1);
        assert!(effects.requires_send());
    }

    #[test]
    fn simulation_effects_count_authorization_entries() {
        let mut simulation = simulation_response(Vec::new());
        simulation.results[0].auth =
            vec![STANDARD.encode(source_authorization_entry().to_xdr(Limits::none()).unwrap())];

        let effects = simulation_effects(&simulation).unwrap();
        assert_eq!(effects.authorization_entry_count, 1);
        assert!(effects.requires_send());
    }

    #[test]
    fn executable_change_during_write_preparation_fails_closed() {
        let before = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("11".repeat(32)),
        };
        let after = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("22".repeat(32)),
        };
        assert!(ensure_contract_executable_unchanged(&before, &before).is_ok());
        let error = ensure_contract_executable_unchanged(&before, &after).unwrap_err();
        assert!(error.contains("changed while preparing"));
        assert!(error.contains(&"11".repeat(32)));
        assert!(error.contains(&"22".repeat(32)));
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
    fn positional_arguments_follow_spec_order_not_parameter_names() {
        let entries = vec![function_entry(
            "transfer",
            &[
                ("source_account", ScSpecTypeDef::Address),
                ("quantity", ScSpecTypeDef::I128),
            ],
        )];
        let request = ContractInvokeRequest::new_positional(
            format!("{}", StrkeyContract([0; 32])),
            "transfer",
            vec![ACCOUNT.to_owned(), "100".to_owned()],
        );
        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Address(_)));
        assert!(matches!(low_level.args[1], ScVal::I128(_)));
        assert_eq!(review[0].name, "source_account");
        assert_eq!(review[1].name, "quantity");
        assert_eq!(review[1].value, json!("100"));
    }

    #[test]
    fn positional_arguments_support_address_names_and_optional_tail() {
        let entries = vec![function_entry(
            "lookup",
            &[
                ("who", ScSpecTypeDef::Address),
                ("memo", option_of(ScSpecTypeDef::String)),
            ],
        )];
        let mut request = ContractInvokeRequest::new_positional(
            format!("{}", StrkeyContract([0; 32])),
            "lookup",
            vec!["Alice".to_owned()],
        );
        request.add_address_name("alice", ACCOUNT).unwrap();
        assert!(request.references_argument_value("ALICE"));
        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Address(_)));
        assert!(matches!(low_level.args[1], ScVal::Void));
        assert_eq!(review[0].value, json!(ACCOUNT));
    }

    #[test]
    fn positional_argument_count_fails_closed() {
        let entries = vec![function_entry(
            "balance",
            &[("who", ScSpecTypeDef::Address)],
        )];
        let too_many = ContractInvokeRequest::new_positional(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            vec![ACCOUNT.to_owned(), OTHER_ACCOUNT.to_owned()],
        );
        assert!(too_many
            .resolve(&entries)
            .unwrap_err()
            .contains("accepts 1 arguments"));

        let missing = ContractInvokeRequest::new_positional(
            format!("{}", StrkeyContract([0; 32])),
            "balance",
            Vec::new(),
        );
        assert!(missing
            .resolve(&entries)
            .unwrap_err()
            .contains("missing positional contract argument 1"));
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
    fn structured_json_arguments_use_official_spec_composer_for_nested_udt_values() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::try_from("route definition").unwrap(),
                lib: StringM::try_from("routing").unwrap(),
                name: StringM::try_from("Route").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::try_from("destination").unwrap(),
                        name: StringM::try_from("destination").unwrap(),
                        type_: ScSpecTypeDef::Address,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::try_from("intermediate hops").unwrap(),
                        name: StringM::try_from("hops").unwrap(),
                        type_: vec_of(ScSpecTypeDef::Address),
                    },
                ])
                .unwrap(),
            }),
            function_entry(
                "compose",
                &[(
                    "routes",
                    option_of(map_of(ScSpecTypeDef::Address, vec_of(udt("Route")))),
                )],
            ),
        ];
        let value = json!({
            ACCOUNT: [{
                "destination": OTHER_ACCOUNT,
                "hops": [ACCOUNT]
            }]
        });
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "compose", vec![]);
        request.add_json_argument("routes", value.clone());

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Map(Some(_))));
        assert_eq!(review[0].name, "routes");
        assert_eq!(review[0].value, value);
    }

    #[test]
    fn structured_json_arguments_fail_closed_on_nested_abi_mismatch() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("Route").unwrap(),
                fields: VecM::try_from(vec![ScSpecUdtStructFieldV0 {
                    doc: StringM::default(),
                    name: StringM::try_from("hops").unwrap(),
                    type_: vec_of(ScSpecTypeDef::Address),
                }])
                .unwrap(),
            }),
            function_entry("compose", &[("route", udt("Route"))]),
        ];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "compose", vec![]);
        request.add_json_argument("route", json!({"hops": [7]}));

        let error = request.resolve(&entries).unwrap_err();
        assert!(
            error.contains("invalid value for contract argument --route"),
            "{error}"
        );
        assert!(error.contains("expected Route"), "{error}");
    }

    #[test]
    fn structured_json_arguments_share_duplicate_detection_with_existing_sources() {
        let entries = vec![function_entry("set", &[("value", ScSpecTypeDef::U32)])];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "set",
            vec![ContractArgumentInput::new("value", "7")],
        );
        request.add_json_argument("value", json!(8));

        assert!(request
            .resolve(&entries)
            .unwrap_err()
            .contains("provided more than once"));
    }

    #[test]
    fn structured_json_arguments_compose_union_enum_and_nested_map_udt_values() {
        let entries = composer_boundary_entries();
        let route = json!({"destination": OTHER_ACCOUNT, "hops": [ACCOUNT]});
        let action = json!({"Transfer": [ACCOUNT, route.clone()]});
        let routes = json!({ACCOUNT: route});
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "compose_all",
            vec![],
        );
        request.add_json_argument("action", action.clone());
        request.add_json_argument("mode", json!(2));
        request.add_json_argument("routes", routes.clone());

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Vec(Some(_))));
        assert!(matches!(low_level.args[1], ScVal::U32(2)));
        assert!(matches!(low_level.args[2], ScVal::Map(Some(_))));
        assert_eq!(review[0].value, action);
        assert_eq!(review[1].value, json!(2));
        assert_eq!(review[2].value, routes);

        let mut void_case = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "compose_all",
            vec![],
        );
        void_case.add_json_argument("action", json!("None"));
        void_case.add_json_argument("mode", json!(1));
        void_case.add_json_argument("routes", json!({}));
        let (_, review) = void_case.resolve(&entries).unwrap();
        assert_eq!(review[0].value, json!("None"));
    }

    #[test]
    fn structured_json_arguments_fail_closed_on_malformed_udt_shapes() {
        let entries = composer_boundary_entries();

        let mut malformed_union = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "compose_all",
            vec![],
        );
        malformed_union.add_json_argument(
            "action",
            json!({"Transfer": [ACCOUNT, {"destination": OTHER_ACCOUNT, "hops": []}], "None": null}),
        );
        malformed_union.add_json_argument("mode", json!(1));
        malformed_union.add_json_argument("routes", json!({}));
        let union_result = std::panic::catch_unwind(|| malformed_union.resolve(&entries));
        assert!(union_result.is_ok(), "malformed union must not panic");
        assert!(union_result.unwrap().unwrap_err().contains("Action"));

        let mut extra_struct_field = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "compose_all",
            vec![],
        );
        extra_struct_field.add_json_argument(
            "action",
            json!({"Transfer": [ACCOUNT, {"destination": OTHER_ACCOUNT, "hops": [], "typo": 7}]}),
        );
        extra_struct_field.add_json_argument("mode", json!(1));
        extra_struct_field.add_json_argument("routes", json!({}));
        assert!(extra_struct_field
            .resolve(&entries)
            .unwrap_err()
            .contains("typo"));

        let mut malformed_enum = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "compose_all",
            vec![],
        );
        malformed_enum.add_json_argument("action", json!("None"));
        malformed_enum.add_json_argument("mode", json!("Exact"));
        malformed_enum.add_json_argument("routes", json!({}));
        let enum_result = std::panic::catch_unwind(|| malformed_enum.resolve(&entries));
        assert!(enum_result.is_ok(), "malformed enum must not panic");
        assert!(enum_result.unwrap().unwrap_err().contains("Mode"));

        let mut error_enum = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "accept_error",
            vec![],
        );
        error_enum.add_json_argument("error", json!(7));
        let error_result = std::panic::catch_unwind(|| error_enum.resolve(&entries));
        assert!(error_result.is_ok(), "error enum input must not panic");
        assert!(error_result.unwrap().unwrap_err().contains("RouteError"));
    }

    #[test]
    fn typed_json_supported_static_types_roundtrip_through_semantic_review() {
        fn assert_roundtrip(type_def: ScSpecTypeDef, value: Value) {
            let entries = vec![function_entry("echo", &[("value", type_def)])];
            let mut first =
                ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "echo", vec![]);
            first.add_json_argument("value", value);
            let (first_low_level, first_review) = first.resolve(&entries).unwrap();

            let mut second =
                ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "echo", vec![]);
            second.add_json_argument("value", first_review[0].value.clone());
            let (second_low_level, second_review) = second.resolve(&entries).unwrap();

            assert_eq!(first_low_level.args, second_low_level.args);
            assert_eq!(first_review[0].value, second_review[0].value);
            assert_eq!(first_review[0].scval_xdr, second_review[0].scval_xdr);
        }

        assert_roundtrip(ScSpecTypeDef::Bool, json!(true));
        assert_roundtrip(ScSpecTypeDef::Void, Value::Null);
        assert_roundtrip(ScSpecTypeDef::U32, json!(7));
        assert_roundtrip(ScSpecTypeDef::I32, json!(-7));
        assert_roundtrip(ScSpecTypeDef::U64, json!(7u64));
        assert_roundtrip(ScSpecTypeDef::I64, json!(-7i64));
        assert_roundtrip(ScSpecTypeDef::Timepoint, json!(1_760_501_234u64));
        assert_roundtrip(ScSpecTypeDef::Duration, json!(1_234_567u64));
        assert_roundtrip(
            ScSpecTypeDef::U128,
            json!("340282366920938463463374607431768211455"),
        );
        assert_roundtrip(
            ScSpecTypeDef::I128,
            json!("-170141183460469231731687303715884105728"),
        );
        assert_roundtrip(ScSpecTypeDef::U256, json!("0xffff"));
        assert_roundtrip(ScSpecTypeDef::I256, json!("-65535"));
        assert_roundtrip(ScSpecTypeDef::Bytes, json!("00a0ff"));
        assert_roundtrip(ScSpecTypeDef::String, json!("hello world"));
        assert_roundtrip(ScSpecTypeDef::Symbol, json!("hello"));
        assert_roundtrip(ScSpecTypeDef::Address, json!(ACCOUNT));
        assert_roundtrip(ScSpecTypeDef::MuxedAddress, json!(ACCOUNT));
        assert_roundtrip(
            ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 4 }),
            json!("00a0ff01"),
        );
        assert_roundtrip(option_of(ScSpecTypeDef::U32), Value::Null);
        assert_roundtrip(option_of(ScSpecTypeDef::U32), json!(7));
        assert_roundtrip(vec_of(ScSpecTypeDef::U32), json!([1, 2, 3]));
        assert_roundtrip(
            map_of(ScSpecTypeDef::U32, ScSpecTypeDef::String),
            json!({"7": "seven"}),
        );
        assert_roundtrip(
            tuple_of(vec![ScSpecTypeDef::U32, ScSpecTypeDef::String]),
            json!([7, "seven"]),
        );
    }

    #[test]
    fn structured_json_void_uses_json_null() {
        let entries = vec![function_entry(
            "accept_void",
            &[("value", ScSpecTypeDef::Void)],
        )];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "accept_void",
            vec![],
        );
        request.add_json_argument("value", Value::Null);
        let (low_level, review) = request.resolve(&entries).unwrap();
        assert_eq!(low_level.args[0], ScVal::Void);
        assert_eq!(review[0].value, Value::Null);
    }

    #[test]
    fn structured_json_arguments_cover_time_types_and_special_map_keys() {
        let entries = vec![function_entry(
            "edge_values",
            &[
                ("when", ScSpecTypeDef::Timepoint),
                ("for", ScSpecTypeDef::Duration),
                ("u32_map", map_of(ScSpecTypeDef::U32, ScSpecTypeDef::String)),
                ("bool_map", map_of(ScSpecTypeDef::Bool, ScSpecTypeDef::U32)),
                (
                    "bytes_map",
                    map_of(
                        ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 2 }),
                        ScSpecTypeDef::U32,
                    ),
                ),
            ],
        )];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "edge_values",
            vec![],
        );
        request.add_json_argument("when", json!(1_760_501_234u64));
        request.add_json_argument("for", json!(1_234_567u64));
        request.add_json_argument("u32_map", json!({"7": "seven"}));
        request.add_json_argument("bool_map", json!({"true": 1}));
        request.add_json_argument("bytes_map", json!({"a0ff": 9}));

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Timepoint(_)));
        assert!(matches!(low_level.args[1], ScVal::Duration(_)));
        assert_eq!(review[0].value, json!(1_760_501_234u64));
        assert_eq!(review[1].value, json!(1_234_567u64));
        assert_eq!(review[2].value, json!({"7": "seven"}));
        assert_eq!(review[3].value, json!({"true": 1}));
        assert_eq!(review[4].value, json!({"a0ff": 9}));
    }

    #[test]
    fn structured_json_map_keys_cover_timepoint_and_nested_udt_adaptation() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("ScheduleKey").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("delay").unwrap(),
                        type_: ScSpecTypeDef::Duration,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("when").unwrap(),
                        type_: ScSpecTypeDef::Timepoint,
                    },
                ])
                .unwrap(),
            }),
            function_entry(
                "set_maps",
                &[
                    (
                        "times",
                        map_of(ScSpecTypeDef::Timepoint, ScSpecTypeDef::U32),
                    ),
                    ("schedules", map_of(udt("ScheduleKey"), ScSpecTypeDef::U32)),
                ],
            ),
        ];
        let mut schedule_map = serde_json::Map::new();
        schedule_map.insert(
            r#"{"delay":1234567,"when":1760501234}"#.to_owned(),
            json!(2),
        );
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "set_maps", vec![]);
        request.add_json_argument("times", json!({"1760501234": 1}));
        request.add_json_argument("schedules", Value::Object(schedule_map));

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Map(Some(_))));
        assert!(matches!(low_level.args[1], ScVal::Map(Some(_))));
        assert_eq!(review[0].value, json!({"1760501234": 1}));
        assert_eq!(review[1].value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn structured_json_val_accepts_explicit_scval_shape_and_normalizes_payload() {
        let entries = vec![function_entry("echo", &[("value", ScSpecTypeDef::Val)])];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "echo", vec![]);
        request.add_json_argument("value", json!({"u32": 7}));

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert_eq!(low_level.args[0], ScVal::U32(7));
        assert_eq!(review[0].value, json!(7));
    }

    #[test]
    fn structured_json_result_input_fails_closed_without_an_official_encoding() {
        let entries = vec![function_entry(
            "accept_result",
            &[("value", result_of(ScSpecTypeDef::U32, ScSpecTypeDef::U32))],
        )];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "accept_result",
            vec![],
        );
        request.add_json_argument("value", json!({"ok": 7}));

        let result = std::panic::catch_unwind(|| request.resolve(&entries));
        assert!(result.is_ok(), "Result input must not panic");
        let error = result.unwrap().unwrap_err();
        assert!(error.contains("result"), "{error}");
        assert!(error.contains("no stable composed input form"), "{error}");
    }

    #[test]
    fn scval_xdr_error_argument_fails_closed_before_official_normalizer_panic() {
        let entries = vec![function_entry(
            "accept_error",
            &[("value", ScSpecTypeDef::Error)],
        )];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "accept_error",
            vec![],
        );
        let error = ScVal::Error(stellar_xdr::ScError::Contract(7))
            .to_xdr_base64(Limits::none())
            .unwrap();
        request.add_scval_xdr_argument("value", error);
        let result = std::panic::catch_unwind(|| request.resolve(&entries));
        assert!(result.is_ok(), "Error argument must not panic");
        let error = result.unwrap().unwrap_err();
        assert!(error.contains("cannot be safely normalized"), "{error}");
    }

    #[test]
    fn contract_spec_named_struct_fields_must_follow_canonical_key_order() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("UnsafeOrder").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("zeta").unwrap(),
                        type_: ScSpecTypeDef::U32,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("alpha").unwrap(),
                        type_: ScSpecTypeDef::String,
                    },
                ])
                .unwrap(),
            }),
            function_entry("read", &[("value", udt("UnsafeOrder"))]),
        ];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "read", vec![]);
        request.add_json_argument("value", json!({"alpha": "x", "zeta": 7}));
        let result = std::panic::catch_unwind(|| request.resolve(&entries));
        assert!(result.is_ok(), "noncanonical Contract Spec must not panic");
        let error = result.unwrap().unwrap_err();
        assert!(error.contains("UnsafeOrder"), "{error}");
        assert!(error.contains("canonical field order"), "{error}");
    }

    #[test]
    fn structured_json_nested_time_and_void_values_are_adapted_before_official_encoding() {
        let entries = vec![
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name: StringM::try_from("Schedule").unwrap(),
                fields: VecM::try_from(vec![
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("delay").unwrap(),
                        type_: ScSpecTypeDef::Duration,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("marker").unwrap(),
                        type_: ScSpecTypeDef::Void,
                    },
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: StringM::try_from("when").unwrap(),
                        type_: ScSpecTypeDef::Timepoint,
                    },
                ])
                .unwrap(),
            }),
            function_entry("schedule", &[("value", udt("Schedule"))]),
        ];
        let value = json!({
            "when": 1_760_501_234u64,
            "delay": 1_234_567u64,
            "marker": null,
        });
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "schedule", vec![]);
        request.add_json_argument("value", value.clone());

        let (_, review) = request.resolve(&entries).unwrap();
        assert_eq!(review[0].value, value);
    }

    #[test]
    fn dynamic_val_review_keeps_exact_scval_identity() {
        let entries = vec![function_entry("echo", &[("value", ScSpecTypeDef::Val)])];

        let mut unsigned =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "echo", vec![]);
        unsigned.add_json_argument("value", json!({"u32": 7}));
        let (_, unsigned_review) = unsigned.resolve(&entries).unwrap();

        let mut signed =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "echo", vec![]);
        signed.add_json_argument("value", json!({"i32": 7}));
        let (_, signed_review) = signed.resolve(&entries).unwrap();

        assert_eq!(unsigned_review[0].value, json!(7));
        assert_eq!(signed_review[0].value, json!(7));
        assert_ne!(unsigned_review[0].scval_xdr, signed_review[0].scval_xdr);
        assert_eq!(
            ScVal::from_xdr_base64(&unsigned_review[0].scval_xdr, Limits::none()).unwrap(),
            ScVal::U32(7)
        );
        assert_eq!(
            ScVal::from_xdr_base64(&signed_review[0].scval_xdr, Limits::none()).unwrap(),
            ScVal::I32(7)
        );
    }

    #[test]
    fn dynamic_val_output_keeps_exact_scval_identity() {
        let entries = vec![function_entry_with_outputs(
            "echo",
            &[],
            &[ScSpecTypeDef::Val],
        )];
        let mut simulation = simulation_response(vec![]);
        simulation.results[0].xdr = STANDARD.encode(ScVal::I32(7).to_xdr(Limits::none()).unwrap());

        let (value, scval_xdr) = decode_simulation_output(&entries, "echo", &simulation).unwrap();
        assert_eq!(value, Some(json!(7)));
        assert_eq!(
            ScVal::from_xdr_base64(scval_xdr.unwrap(), Limits::none()).unwrap(),
            ScVal::I32(7)
        );
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
    fn scval_xdr_argument_accepts_real_aqua_swap_chain_under_spec_type() {
        let entries = vec![function_entry(
            "swap_chained",
            &[("swaps_chain", aqua_swaps_chain_type())],
        )];
        let mut request = ContractInvokeRequest::new(
            format!("{}", StrkeyContract([0; 32])),
            "swap_chained",
            vec![],
        );
        request.add_scval_xdr_argument("swaps_chain", AQUA_TESTNET_SWAP_CHAIN_XDR);

        let (low_level, review) = request.resolve(&entries).unwrap();
        assert!(matches!(low_level.args[0], ScVal::Vec(Some(_))));
        assert_eq!(review[0].name, "swaps_chain");
        assert_eq!(review[0].value.as_array().unwrap().len(), 4);
    }

    #[test]
    fn scval_xdr_scalar_type_mismatch_fails_closed_without_official_normalizer_panic() {
        let entries = vec![function_entry("set", &[("value", ScSpecTypeDef::U32)])];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "set", vec![]);
        request.add_scval_xdr_argument(
            "value",
            ScVal::I32(7).to_xdr_base64(Limits::none()).unwrap(),
        );
        let result = std::panic::catch_unwind(|| request.resolve(&entries));
        assert!(result.is_ok(), "mismatched ScVal must not panic");
        let error = result.unwrap().unwrap_err();
        assert!(
            error.contains("does not match Contract Spec type u32"),
            "{error}"
        );
    }

    #[test]
    fn scval_xdr_tuple_arity_is_validated_before_normalization() {
        let entries = vec![function_entry(
            "set",
            &[(
                "value",
                tuple_of(vec![ScSpecTypeDef::U32, ScSpecTypeDef::String]),
            )],
        )];
        let value = ScVal::Vec(Some(vec![ScVal::U32(7)].try_into().unwrap()));
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "set", vec![]);
        request.add_scval_xdr_argument("value", value.to_xdr_base64(Limits::none()).unwrap());
        let error = request.resolve(&entries).unwrap_err();
        assert!(
            error.contains("does not match Contract Spec type (u32,string)"),
            "{error}"
        );
    }

    #[test]
    fn scval_xdr_argument_rejects_invalid_base64() {
        let entries = vec![function_entry("set", &[("value", ScSpecTypeDef::U32)])];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "set", vec![]);
        request.add_scval_xdr_argument("value", "not-xdr");
        let error = request.resolve(&entries).unwrap_err();
        assert!(error.contains("invalid ScVal XDR for contract argument --value"));
    }

    #[test]
    fn scval_xdr_argument_must_match_contract_spec_type() {
        let entries = vec![function_entry("set", &[("value", ScSpecTypeDef::U32)])];
        let mut request =
            ContractInvokeRequest::new(format!("{}", StrkeyContract([0; 32])), "set", vec![]);
        request.add_scval_xdr_argument("value", AQUA_TESTNET_SWAP_CHAIN_XDR);
        let error = request.resolve(&entries).unwrap_err();
        assert!(
            error.contains("invalid contract argument --value (u32)"),
            "{error}"
        );
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
