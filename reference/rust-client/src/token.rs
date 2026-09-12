use std::borrow::Cow;

use serde_json::Value;
use sha2::{Digest, Sha256};
use stellar_strkey::Contract as StrkeyContract;
use stellar_xdr::{
    ContractIdPreimage, Hash, HashIdPreimage, HashIdPreimageContractId, Limits, WriteXdr,
};

use crate::asset::AssetId;
use crate::contract::{
    ContractAddressNames, ContractCapabilities, ContractExecutableObservation,
    ContractInvokePreparation, ContractInvokeRequest, ContractReadResult, PreparedContractInvoke,
};
use crate::service::FresnicaClient;
use crate::transaction::network_passphrase;

const XDR_DEPTH_LIMIT: u32 = 500;
pub const STELLAR_ASSET_TOKEN_DECIMALS: u32 = 7;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedTokenSource {
    StellarAsset { asset: String },
    Contract,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedToken {
    pub contract_id: String,
    pub source: ResolvedTokenSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenAmount {
    pub raw: i128,
    pub decimals: u32,
    pub amount: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenBalanceRequest {
    pub token: ResolvedToken,
    pub owner: String,
    address_names: ContractAddressNames,
}

impl TokenBalanceRequest {
    pub fn new(token: ResolvedToken, owner: impl Into<String>) -> Self {
        Self {
            token,
            owner: owner.into(),
            address_names: ContractAddressNames::default(),
        }
    }

    pub fn references_argument_value(&self, candidate: &str) -> bool {
        same_argument_value(&self.owner, candidate)
    }

    pub fn add_address_name(&mut self, name: &str, address: &str) -> Result<(), String> {
        self.address_names.add(name, address)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenBalanceResult {
    pub token: ResolvedToken,
    pub owner: Value,
    pub amount: TokenAmount,
    pub contract: ContractReadResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenTransferRequest {
    pub token: ResolvedToken,
    pub wallet: Option<String>,
    pub amount: String,
    pub destination: String,
    address_names: ContractAddressNames,
}

impl TokenTransferRequest {
    pub fn new(
        token: ResolvedToken,
        amount: impl Into<String>,
        destination: impl Into<String>,
    ) -> Self {
        Self {
            token,
            wallet: None,
            amount: amount.into(),
            destination: destination.into(),
            address_names: ContractAddressNames::default(),
        }
    }

    pub fn references_argument_value(&self, candidate: &str) -> bool {
        same_argument_value(&self.destination, candidate)
    }

    pub fn add_address_name(&mut self, name: &str, address: &str) -> Result<(), String> {
        self.address_names.add(name, address)
    }
}

#[derive(Clone, Debug)]
pub struct PreparedTokenTransfer {
    pub token: ResolvedToken,
    pub wallet_name: String,
    pub wallet_address: String,
    pub destination: Value,
    pub amount_input: String,
    pub amount: TokenAmount,
    pub contract: PreparedContractInvoke,
}

pub fn resolve_token(reference: &str, network: &str) -> Result<ResolvedToken, String> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err("token reference cannot be empty".to_owned());
    }

    if let Ok(contract) = StrkeyContract::from_string(reference) {
        return Ok(ResolvedToken {
            contract_id: format!("{contract}"),
            source: ResolvedTokenSource::Contract,
        });
    }

    let asset_text = if reference.eq_ignore_ascii_case("native") {
        "XLM"
    } else {
        reference
    };
    let asset = AssetId::parse(asset_text).map_err(|error| {
        format!("token must be XLM, native, CODE:GISSUER, or a C... contract id: {error}")
    })?;
    let passphrase = network_passphrase(network)?;
    let contract_id = contract_id_from_asset(&asset, passphrase)?;
    Ok(ResolvedToken {
        contract_id,
        source: ResolvedTokenSource::StellarAsset {
            asset: asset.display(),
        },
    })
}

impl FresnicaClient {
    pub async fn token_balance(
        &self,
        request: TokenBalanceRequest,
    ) -> Result<TokenBalanceResult, String> {
        let mut balance_request = ContractInvokeRequest::new_positional(
            &request.token.contract_id,
            "balance",
            vec![request.owner],
        );
        balance_request.set_address_names(request.address_names);
        let balance = token_read(self, balance_request, "balance").await?;
        let raw = parse_i128_output(balance.output.as_ref(), "balance")?;

        let decimals = match request.token.source {
            ResolvedTokenSource::StellarAsset { .. } => STELLAR_ASSET_TOKEN_DECIMALS,
            ResolvedTokenSource::Contract => {
                let decimals = token_read(
                    self,
                    ContractInvokeRequest::new_positional(
                        &request.token.contract_id,
                        "decimals",
                        Vec::new(),
                    ),
                    "decimals",
                )
                .await?;
                ensure_same_executable(&balance.executable, &decimals.executable)?;
                parse_u32_output(decimals.output.as_ref(), "decimals")?
            }
        };
        let owner = balance
            .arguments
            .first()
            .map(|argument| argument.value.clone())
            .ok_or_else(|| "SEP-41 balance review is missing its owner argument".to_owned())?;
        Ok(TokenBalanceResult {
            token: request.token,
            owner,
            amount: TokenAmount {
                raw,
                decimals,
                amount: format_fixed_point(raw, decimals),
            },
            contract: balance,
        })
    }

    pub async fn prepare_token_transfer(
        &self,
        request: TokenTransferRequest,
    ) -> Result<PreparedTokenTransfer, String> {
        let wallet = self.resolve_wallet(request.wallet.as_deref())?;
        let decimals_observation = match request.token.source {
            ResolvedTokenSource::StellarAsset { .. } => None,
            ResolvedTokenSource::Contract => Some(
                token_read(
                    self,
                    ContractInvokeRequest::new_positional(
                        &request.token.contract_id,
                        "decimals",
                        Vec::new(),
                    ),
                    "decimals",
                )
                .await?,
            ),
        };
        let decimals = decimals_observation
            .as_ref()
            .map(|result| parse_u32_output(result.output.as_ref(), "decimals"))
            .transpose()?
            .unwrap_or(STELLAR_ASSET_TOKEN_DECIMALS);
        let raw = parse_token_amount(&request.amount, decimals)?;

        let mut invoke = ContractInvokeRequest::new_positional(
            &request.token.contract_id,
            "transfer",
            vec![wallet.address.clone(), request.destination, raw.to_string()],
        );
        invoke.wallet = Some(wallet.name.clone());
        invoke.set_address_names(request.address_names);
        let outcome = self.prepare_contract_invoke_outcome(invoke).await?;
        let ContractInvokePreparation::Transaction(prepared) = outcome else {
            return Err(
                "SEP-41 transfer unexpectedly simulated as read-only; refusing token transfer"
                    .to_owned(),
            );
        };
        ensure_current_sep41(&prepared.review.capabilities, &prepared.review.contract_id)?;
        if let Some(decimals) = decimals_observation.as_ref() {
            ensure_same_executable(&decimals.executable, &prepared.review.executable)?;
        }
        let source = prepared
            .review
            .arguments
            .first()
            .map(|argument| argument.value.clone())
            .ok_or_else(|| "SEP-41 transfer review is missing its source argument".to_owned())?;
        if source != Value::String(wallet.address.clone()) {
            return Err(
                "prepared SEP-41 transfer source does not match the selected Fresnica wallet"
                    .to_owned(),
            );
        }
        let destination = prepared
            .review
            .arguments
            .get(1)
            .map(|argument| argument.value.clone())
            .ok_or_else(|| {
                "SEP-41 transfer review is missing its destination argument".to_owned()
            })?;
        Ok(PreparedTokenTransfer {
            token: request.token,
            wallet_name: wallet.name,
            wallet_address: wallet.address,
            destination,
            amount_input: request.amount,
            amount: TokenAmount {
                raw,
                decimals,
                amount: format_fixed_point(raw, decimals),
            },
            contract: prepared,
        })
    }
}

async fn token_read(
    client: &FresnicaClient,
    request: ContractInvokeRequest,
    function: &str,
) -> Result<ContractReadResult, String> {
    let outcome = client.prepare_contract_invoke_outcome(request).await?;
    let ContractInvokePreparation::ReadOnly(result) = outcome else {
        return Err(format!(
            "SEP-41 {function} unexpectedly requires a write transaction; refusing token read"
        ));
    };
    ensure_current_sep41(&result.capabilities, &result.contract_id)?;
    Ok(result)
}

fn ensure_current_sep41(
    capabilities: &ContractCapabilities,
    contract_id: &str,
) -> Result<(), String> {
    if capabilities.sep41.current_interface_compatible {
        Ok(())
    } else {
        Err(format!(
            "contract {contract_id} is not compatible with the current SEP-41 interface"
        ))
    }
}

fn ensure_same_executable(
    first: &ContractExecutableObservation,
    second: &ContractExecutableObservation,
) -> Result<(), String> {
    if first == second {
        Ok(())
    } else {
        Err(format!(
            "token contract executable changed during the operation from {} to {}; retry after inspecting the contract",
            executable_label(first),
            executable_label(second)
        ))
    }
}

fn executable_label(observation: &ContractExecutableObservation) -> String {
    match observation.wasm_hash.as_deref() {
        Some(hash) => format!("{}:{hash}", observation.kind.as_str()),
        None => observation.kind.as_str().to_owned(),
    }
}

fn parse_i128_output(value: Option<&Value>, label: &str) -> Result<i128, String> {
    parse_integer_text(value, label)?
        .parse()
        .map_err(|_| format!("SEP-41 {label} returned a value outside i128 range"))
}

fn parse_u32_output(value: Option<&Value>, label: &str) -> Result<u32, String> {
    parse_integer_text(value, label)?
        .parse()
        .map_err(|_| format!("SEP-41 {label} returned a value outside u32 range"))
}

fn parse_integer_text<'a>(value: Option<&'a Value>, label: &str) -> Result<Cow<'a, str>, String> {
    match value {
        Some(Value::String(value)) => Ok(Cow::Borrowed(value)),
        Some(Value::Number(value)) => Ok(Cow::Owned(value.to_string())),
        Some(other) => Err(format!(
            "SEP-41 {label} returned a non-integer value: {other}"
        )),
        None => Err(format!("SEP-41 {label} returned no value")),
    }
}

fn parse_token_amount(value: &str, decimals: u32) -> Result<i128, String> {
    let value = value.trim();
    if value.is_empty() || value.starts_with('-') || value.starts_with('+') {
        return Err("token transfer amount must be a positive decimal number".to_owned());
    }
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default().trim_end_matches('0');
    if parts.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("token transfer amount must be a positive decimal number".to_owned());
    }
    if fraction.len() > decimals as usize {
        return Err(format!(
            "token transfer amount has more than {decimals} decimal places"
        ));
    }

    let mut significant = format!("{whole}{fraction}");
    let first_nonzero = significant.bytes().position(|byte| byte != b'0');
    let Some(first_nonzero) = first_nonzero else {
        return Err("token transfer amount must be greater than zero".to_owned());
    };
    significant.drain(..first_nonzero);
    let trailing_zeros = decimals as usize - fraction.len();
    if significant.len().saturating_add(trailing_zeros) > 39 {
        return Err("token transfer amount is outside the i128 range".to_owned());
    }
    significant.extend(std::iter::repeat_n('0', trailing_zeros));
    let raw: i128 = significant
        .parse()
        .map_err(|_| "token transfer amount is outside the i128 range".to_owned())?;
    if raw <= 0 {
        return Err("token transfer amount must be greater than zero".to_owned());
    }
    Ok(raw)
}

fn format_fixed_point(raw: i128, decimals: u32) -> Option<String> {
    let decimals = usize::try_from(decimals).ok()?;
    if decimals > 38 {
        return None;
    }
    let negative = raw < 0;
    let digits = raw.unsigned_abs().to_string();
    if decimals == 0 {
        return Some(if negative {
            format!("-{digits}")
        } else {
            digits
        });
    }
    let (whole, fraction) = if digits.len() <= decimals {
        (
            "0".to_owned(),
            format!("{}{}", "0".repeat(decimals - digits.len()), digits),
        )
    } else {
        let split = digits.len() - decimals;
        (digits[..split].to_owned(), digits[split..].to_owned())
    };
    let fraction = fraction.trim_end_matches('0');
    let value = if fraction.is_empty() {
        whole
    } else {
        format!("{whole}.{fraction}")
    };
    Some(if negative { format!("-{value}") } else { value })
}

fn same_argument_value(value: &str, candidate: &str) -> bool {
    value.trim().trim_matches('"').to_ascii_lowercase()
        == candidate.trim().trim_matches('"').to_ascii_lowercase()
}

fn contract_id_from_asset(asset: &AssetId, network_passphrase: &str) -> Result<String, String> {
    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage: ContractIdPreimage::Asset(asset.to_xdr()),
    });
    let xdr = preimage
        .to_xdr(Limits::depth(XDR_DEPTH_LIMIT))
        .map_err(|error| format!("unable to encode Stellar Asset Contract id preimage: {error}"))?;
    Ok(format!("{}", StrkeyContract(Sha256::digest(xdr).into())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ContractExecutableKind, ContractSep41Evidence};

    const TESTNET_NATIVE_SAC: &str = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";
    const ISSUER: &str = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5";
    const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn testnet_native_resolves_to_known_sac_vector() {
        let xlm = resolve_token("XLM", "testnet").unwrap();
        let native = resolve_token("native", "testnet").unwrap();
        assert_eq!(xlm, native);
        assert_eq!(xlm.contract_id, TESTNET_NATIVE_SAC);
        assert_eq!(
            xlm.source,
            ResolvedTokenSource::StellarAsset {
                asset: "XLM".to_owned()
            }
        );
    }

    #[test]
    fn issued_asset_resolution_is_network_scoped_and_preserves_asset_identity() {
        let reference = format!("USDC:{ISSUER}");
        let testnet = resolve_token(&reference, "testnet").unwrap();
        let mainnet = resolve_token(&reference, "mainnet").unwrap();
        assert_ne!(testnet.contract_id, mainnet.contract_id);
        assert_eq!(
            testnet.source,
            ResolvedTokenSource::StellarAsset {
                asset: reference.clone()
            }
        );
    }

    #[test]
    fn contract_reference_is_preserved_without_asset_reinterpretation() {
        let resolved = resolve_token(CONTRACT, "testnet").unwrap();
        assert_eq!(resolved.contract_id, CONTRACT);
        assert_eq!(resolved.source, ResolvedTokenSource::Contract);
    }

    #[test]
    fn invalid_reference_fails_with_supported_shapes() {
        let error = resolve_token("not-a-token", "testnet").unwrap_err();
        assert!(error.contains("XLM, native, CODE:GISSUER, or a C... contract id"));
    }

    #[test]
    fn token_requests_share_contract_address_name_ambiguity_rules() {
        let token = resolve_token(TESTNET_NATIVE_SAC, "testnet").unwrap();
        let mut balance = TokenBalanceRequest::new(token.clone(), "alice");
        assert!(balance.references_argument_value("ALICE"));
        balance.add_address_name("alice", ACCOUNT).unwrap();
        assert!(balance.add_address_name("alice", CONTRACT).is_err());

        let mut transfer = TokenTransferRequest::new(token, "1", "alice");
        assert!(transfer.references_argument_value("Alice"));
        transfer.add_address_name("alice", ACCOUNT).unwrap();
        assert!(transfer.add_address_name("alice", CONTRACT).is_err());
    }

    #[test]
    fn token_amount_parser_is_exact_and_rejects_rounding_or_overflow() {
        assert_eq!(parse_token_amount("1", 7).unwrap(), 10_000_000);
        assert_eq!(parse_token_amount("1.25", 7).unwrap(), 12_500_000);
        assert_eq!(parse_token_amount("0.0000001", 7).unwrap(), 1);
        assert_eq!(parse_token_amount("1.2300000", 7).unwrap(), 12_300_000);
        assert!(parse_token_amount("0", 7).is_err());
        assert!(parse_token_amount("-1", 7).is_err());
        assert!(parse_token_amount("1e2", 7).is_err());
        assert!(parse_token_amount("0.00000001", 7).is_err());
        assert!(parse_token_amount("999999999999999999999999999999999999999", 7).is_err());
    }

    #[test]
    fn fixed_point_format_is_exact_and_bounded() {
        assert_eq!(
            format_fixed_point(12_345_678, 7).as_deref(),
            Some("1.2345678")
        );
        assert_eq!(format_fixed_point(10_000_000, 7).as_deref(), Some("1"));
        assert_eq!(format_fixed_point(1, 7).as_deref(), Some("0.0000001"));
        assert_eq!(format_fixed_point(-1, 7).as_deref(), Some("-0.0000001"));
        assert_eq!(format_fixed_point(1, 39), None);
    }

    #[test]
    fn executable_identity_must_stay_stable_across_multi_read_operation() {
        let first = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("11".repeat(32)),
        };
        assert!(ensure_same_executable(&first, &first).is_ok());
        let changed = ContractExecutableObservation {
            kind: ContractExecutableKind::Wasm,
            wasm_hash: Some("22".repeat(32)),
        };
        assert!(ensure_same_executable(&first, &changed)
            .unwrap_err()
            .contains("changed during the operation"));
    }

    #[test]
    fn current_sep41_evidence_is_required() {
        let capabilities = ContractCapabilities {
            sep41: ContractSep41Evidence {
                native_sac: false,
                sep47_declared: true,
                current_interface_compatible: false,
            },
        };
        assert!(ensure_current_sep41(&capabilities, CONTRACT).is_err());
    }
}
