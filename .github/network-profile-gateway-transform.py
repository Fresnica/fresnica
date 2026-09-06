from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if old not in text:
        raise SystemExit(f"expected source fragment not found in {path}")
    file.write_text(text.replace(old, new, 1))


replace_once(
    "reference/rust-client/src/transaction.rs",
    """use crate::{
    HorizonGateway, SubmissionError, WalletRecord, WalletStorage, MAINNET_HORIZON_URL,
    TESTNET_HORIZON_URL,
};
""",
    """use crate::{HorizonGateway, SubmissionError, WalletRecord, WalletStorage};
""",
)

replace_once(
    "reference/rust-client/src/transaction.rs",
    """pub fn network_gateway(network: &str) -> Result<HorizonGateway, String> {
    Ok(HorizonGateway::new(match network {
        \"mainnet\" => MAINNET_HORIZON_URL,
        \"testnet\" => TESTNET_HORIZON_URL,
        other => return Err(format!(\"unknown network: {other}\")),
    }))
}

""",
    "",
)

replace_once(
    "reference/rust-client/src/lib.rs",
    """    minimum_balance_stroops, network_gateway, network_passphrase, parse_positive_stroops,
""",
    """    minimum_balance_stroops, network_passphrase, parse_positive_stroops,
""",
)
