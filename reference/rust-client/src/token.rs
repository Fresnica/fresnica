use sha2::{Digest, Sha256};
use stellar_strkey::Contract as StrkeyContract;
use stellar_xdr::{
    ContractIdPreimage, Hash, HashIdPreimage, HashIdPreimageContractId, Limits, WriteXdr,
};

use crate::asset::AssetId;
use crate::transaction::network_passphrase;

const XDR_DEPTH_LIMIT: u32 = 500;

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

    const TESTNET_NATIVE_SAC: &str = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
    const CONTRACT: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";
    const ISSUER: &str = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5";

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
}
