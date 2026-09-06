use std::collections::BTreeSet;

use zeroize::Zeroizing;

use crate::anchor_protocol::{
    exchange_anchor_sep10_challenge, prepare_anchor_sep10_challenge, sep10_authorization_plan,
    AnchorCapabilities,
};
use crate::ledger_authorization::{
    satisfied_ed25519_conditions, LedgerSignerCondition, LedgerSignerKind,
};
use crate::service::FresnicaClient;
use crate::signing_coordination::sign_needed_local_ed25519;
use crate::storage::WalletRecord;
use crate::transaction::{network_passphrase, parse_transaction_xdr};

impl FresnicaClient {
    pub fn authenticate_anchor_sep10<F>(
        &self,
        record: &WalletRecord,
        home_domain: &str,
        capabilities: &AnchorCapabilities,
        passcode_provider: F,
    ) -> Result<Zeroizing<String>, String>
    where
        F: FnOnce() -> Result<Zeroizing<String>, String>,
    {
        let network = self.network();
        if record.network != network {
            return Err(format!(
                "wallet \"{}\" is configured for {}; invoke with --network {}",
                record.name, record.network, record.network
            ));
        }

        let ledger_account = self.ledger_account(&record.address)?;
        let authorization = sep10_authorization_plan(ledger_account.as_ref(), &record.address)?;
        let challenge =
            prepare_anchor_sep10_challenge(network, &record.address, home_domain, capabilities)?;
        let mut envelope = parse_transaction_xdr(challenge.transaction_xdr())?;
        let mut satisfied =
            satisfied_ed25519_conditions(&authorization, &envelope, network_passphrase(network)?)?;
        satisfied.remove(&LedgerSignerCondition {
            kind: LedgerSignerKind::Ed25519PublicKey,
            key: challenge.server_signing_key().to_owned(),
        });
        let excluded = BTreeSet::from([challenge.server_signing_key().to_owned()]);
        let passcode = passcode_provider()?;
        sign_needed_local_ed25519(
            self.storage(),
            &authorization,
            &satisfied,
            &excluded,
            1,
            network,
            &mut envelope,
            passcode.as_str(),
        )?;
        exchange_anchor_sep10_challenge(network, &challenge, &authorization, &envelope)
    }
}
