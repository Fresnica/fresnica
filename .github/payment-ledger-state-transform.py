from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    assert old in text, f"missing expected text in {path}"
    file.write_text(text.replace(old, new, 1))


replace_once(
    "reference/rust-client/src/lib.rs",
    "mod history_state;\npub mod horizon_gateway;",
    "mod history_state;\npub mod horizon_gateway;\nmod ledger_account_state;",
)

service = Path("reference/rust-client/src/service.rs")
text = service.read_text()
text = text.replace(
    "use crate::history_state::HistoryOperation;\n",
    "use crate::history_state::HistoryOperation;\nuse crate::ledger_account_state::LedgerAccountState;\n",
    1,
)
old = '''    pub fn ledger_account(&self, address: &str) -> Result<Option<Value>, String> {
        self.gateway.get_account_optional(address)
    }
'''
new = '''    pub(crate) fn ledger_account_state(&self, address: &str) -> Result<LedgerAccountState, String> {
        let raw = self.gateway.get_account(address)?;
        LedgerAccountState::from_horizon(&raw)
    }

    pub(crate) fn ledger_account_state_optional(
        &self,
        address: &str,
    ) -> Result<Option<LedgerAccountState>, String> {
        match self.gateway.get_account_optional(address)? {
            Some(raw) => LedgerAccountState::from_horizon(&raw).map(Some),
            None => Ok(None),
        }
    }
'''
assert old in text
service.write_text(text.replace(old, new, 1))

transaction = Path("reference/rust-client/src/transaction.rs")
text = transaction.read_text()
text = text.replace(
    "use crate::ledger_authorization::{\n",
    "use crate::account_state::AccountState;\nuse crate::ledger_authorization::{\n",
    1,
)
marker = '''pub(crate) fn prepared_classic_authorization_snapshot(
    storage: &WalletStorage,
    network: &str,
    envelope: &TransactionEnvelope,
    source_account: &Value,
) -> Result<LedgerAuthorizationSnapshot, String> {
    let account = LedgerAccountAuthorization::from_horizon(source_account).map_err(|error| {
        format!("Unable to interpret prepared transaction authorization: {error}")
    })?;
    let plan = plan_classic_ledger_authorization(envelope, &[account])?;
    review_ledger_authorization(storage, &plan, network, envelope)
}
'''
addition = marker + '''

pub(crate) fn prepared_classic_authorization_snapshot_from_state(
    storage: &WalletStorage,
    network: &str,
    envelope: &TransactionEnvelope,
    source_account: &AccountState,
) -> Result<LedgerAuthorizationSnapshot, String> {
    let plan = plan_classic_ledger_authorization(envelope, &[source_account.authorization()])?;
    review_ledger_authorization(storage, &plan, network, envelope)
}
'''
assert marker in text
transaction.write_text(text.replace(marker, addition, 1))

payment = Path("reference/rust-client/src/payment.rs")
text = payment.read_text()
old_imports = '''use crate::asset::AssetId;
use crate::transaction::prepared_classic_authorization_snapshot;
use crate::{
    account_sequence, balance_stroops, build_single_operation_envelope_with_memo, format_stroops,
    minimum_balance_stroops, parse_positive_stroops, resolve_destination, resolve_write_wallet,
    sign_and_submit, FresnicaClient, LedgerAuthorizationSnapshot, LedgerParameters,
    TransactionSubmission, WalletRecord,
};
'''
new_imports = '''use crate::asset::AssetId;
use crate::ledger_account_state::{
    LedgerAccountState, LedgerBalanceState, LedgerTrustlineAuthorization,
};
use crate::transaction::prepared_classic_authorization_snapshot_from_state;
use crate::{
    build_single_operation_envelope_with_memo, format_stroops, parse_positive_stroops,
    resolve_destination, resolve_write_wallet, sign_and_submit, FresnicaClient,
    LedgerAuthorizationSnapshot, LedgerParameters, TransactionSubmission, WalletRecord,
};
'''
assert old_imports in text
text = text.replace(old_imports, new_imports, 1)
old_prepare = '''        let account = self.gateway().get_account(&current.address)?;
        let destination_exists = self.gateway().account_exists(destination_address)?;
        if !destination_exists && !asset.is_native() {
            return Err(
                "Destination account does not exist. Only XLM can create a new Stellar account; issued assets require an existing account and trustline."
                    .to_owned(),
            );
        }
        let destination_account = if destination_exists {
            Some(self.gateway().get_account(destination_address)?)
        } else {
            None
        };
        let ledger = self.gateway().get_ledger_parameters()?;
        validate_transfer(&account, &current.address, &asset, amount, ledger)?;
        if let Some(destination_account) = destination_account.as_ref() {
            validate_destination_receive(destination_account, destination_address, &asset, amount)?;
            if matches!(&memo, PaymentMemo::None) && account_requires_memo(destination_account)? {
                return Err(format!(
                    "Destination {destination_address} requires a transaction memo (SEP-29). Add a memo and try again."
                ));
            }
        }
'''
new_prepare = '''        let account = self.ledger_account_state(&current.address)?;
        let destination_account = self.ledger_account_state_optional(destination_address)?;
        let destination_exists = destination_account.is_some();
        if !destination_exists && !asset.is_native() {
            return Err(
                "Destination account does not exist. Only XLM can create a new Stellar account; issued assets require an existing account and trustline."
                    .to_owned(),
            );
        }
        let ledger = self.gateway().get_ledger_parameters()?;
        validate_transfer(&account, &current.address, &asset, amount, ledger)?;
        if let Some(destination_account) = destination_account.as_ref() {
            validate_destination_receive(destination_account, destination_address, &asset, amount)?;
            if matches!(&memo, PaymentMemo::None) && destination_account.memo_required {
                return Err(format!(
                    "Destination {destination_address} requires a transaction memo (SEP-29). Add a memo and try again."
                ));
            }
        }
'''
assert old_prepare in text
text = text.replace(old_prepare, new_prepare, 1)
text = text.replace("            account_sequence(&account)?,\n", "            account.account.sequence,\n", 1)
old_auth = '''        let ledger_authorization = prepared_classic_authorization_snapshot(
            self.storage(),
            self.network(),
            &envelope,
            &account,
        )?;
'''
new_auth = '''        let ledger_authorization = prepared_classic_authorization_snapshot_from_state(
            self.storage(),
            self.network(),
            &envelope,
            &account.account,
        )?;
'''
assert old_auth in text
text = text.replace(old_auth, new_auth, 1)
start = text.index("fn validate_transfer(\n")
end = text.index("fn account_id_to_muxed", start)
typed_helpers = r'''fn validate_transfer(
    account: &LedgerAccountState,
    account_id: &str,
    asset: &AssetId,
    requested: i64,
    ledger: LedgerParameters,
) -> Result<(), String> {
    if !asset.is_native() && asset.issuer_is(account_id) {
        ensure_payment_fee_capacity(account, ledger)?;
        return Ok(());
    }

    let available = match account.balance(asset) {
        Some(balance) if asset.is_native() => balance
            .available_after_selling_liabilities()
            .saturating_sub(account.minimum_balance_stroops(ledger.base_reserve_in_stroops)?)
            .saturating_sub(i64::from(ledger.base_fee_in_stroops))
            .max(0),
        Some(balance) => {
            ensure_payment_trustline_authorized(balance, asset)?;
            ensure_payment_fee_capacity(account, ledger)?;
            balance.available_after_selling_liabilities()
        }
        None => 0,
    };
    if requested > available {
        return Err(format!(
            "Insufficient {} balance: requested {}, available {}",
            asset.display(),
            format_stroops(requested),
            format_stroops(available)
        ));
    }
    Ok(())
}

fn validate_destination_receive(
    account: &LedgerAccountState,
    account_id: &str,
    asset: &AssetId,
    requested: i64,
) -> Result<(), String> {
    if !asset.is_native() && asset.issuer_is(account_id) {
        return Ok(());
    }
    let balance = account
        .balance(asset)
        .ok_or_else(|| format!("Destination has no trustline for {}", asset.display()))?;

    let capacity = if asset.is_native() {
        i64::MAX.saturating_sub(balance.committed_for_receiving()?)
    } else {
        ensure_payment_trustline_authorized(balance, asset)?;
        let limit = balance
            .limit
            .ok_or_else(|| format!("Trustline limit is unavailable for {}", asset.display()))?;
        limit
            .saturating_sub(balance.committed_for_receiving()?)
            .max(0)
    };
    if requested > capacity {
        return Err(format!(
            "Insufficient receiving capacity for {} at destination: need {}, available {}",
            asset.display(),
            format_stroops(requested),
            format_stroops(capacity)
        ));
    }
    Ok(())
}

fn ensure_payment_fee_capacity(
    account: &LedgerAccountState,
    ledger: LedgerParameters,
) -> Result<(), String> {
    let native = account
        .balance(&AssetId::native())
        .ok_or_else(|| "No XLM balance is available to pay the transaction fee".to_owned())?;
    let free = native
        .available_after_selling_liabilities()
        .saturating_sub(account.minimum_balance_stroops(ledger.base_reserve_in_stroops)?)
        .max(0);
    let fee = i64::from(ledger.base_fee_in_stroops);
    if free < fee {
        return Err(format!(
            "Insufficient XLM for transaction fee: need {}, available {}",
            format_stroops(fee),
            format_stroops(free)
        ));
    }
    Ok(())
}

fn ensure_payment_trustline_authorized(
    balance: &LedgerBalanceState,
    asset: &AssetId,
) -> Result<(), String> {
    match balance.authorization {
        Some(LedgerTrustlineAuthorization::Full) => Ok(()),
        Some(
            LedgerTrustlineAuthorization::MaintainLiabilities
            | LedgerTrustlineAuthorization::Unauthorized,
        ) => Err(format!(
            "Trustline for {} is not fully authorized for payment",
            asset.display()
        )),
        None => Err(format!(
            "Trustline authorization state is unavailable for {}",
            asset.display()
        )),
    }
}

'''
text = text[:start] + typed_helpers + text[end:]
old_test_helper = '''    fn account(native_balance: &str, subentries: i64) -> Value {
        serde_json::json!({
            "sequence": "7",
            "subentry_count": subentries,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [{
                "asset_type": "native",
                "balance": native_balance,
                "selling_liabilities": "0.0000000",
                "buying_liabilities": "0.0000000"
            }]
        })
    }
'''
new_test_helper = '''    fn account_state(account_id: &str, subentries: u32) -> crate::AccountState {
        crate::AccountState {
            account_id: account_id.to_owned(),
            sequence: 7,
            subentry_count: subentries,
            num_sponsoring: 0,
            num_sponsored: 0,
            home_domain: None,
            thresholds: crate::AccountThresholds {
                low: 0,
                medium: 0,
                high: 0,
            },
            signers: Vec::new(),
        }
    }

    fn native_balance(value: &str) -> LedgerBalanceState {
        LedgerBalanceState {
            asset: crate::BalanceAsset::Native,
            balance: crate::parse_stroops(value, false).unwrap(),
            selling_liabilities: 0,
            buying_liabilities: 0,
            limit: None,
            authorization: None,
            clawback_enabled: None,
        }
    }

    fn account(native_balance_value: &str, subentries: u32) -> LedgerAccountState {
        LedgerAccountState {
            account: account_state("GSOURCE", subentries),
            balances: vec![native_balance(native_balance_value)],
            flags: Default::default(),
            memo_required: false,
        }
    }
'''
assert old_test_helper in text
text = text.replace(old_test_helper, new_test_helper, 1)
old_issuer = '''        let account = serde_json::json!({
            "subentry_count": 0,
            "num_sponsoring": 0,
            "num_sponsored": 0,
            "balances": [{
                "asset_type": "native",
                "balance": "10",
                "selling_liabilities": "0",
                "buying_liabilities": "0"
            }]
        });
'''
new_issuer = '''        let account = LedgerAccountState {
            account: account_state(source, 0),
            balances: vec![native_balance("10")],
            flags: Default::default(),
            memo_required: false,
        };
'''
assert old_issuer in text
text = text.replace(old_issuer, new_issuer, 1)
old_destination = '''        let destination = serde_json::json!({
            "balances": [{
                "asset_type": "credit_alphanum4",
                "asset_code": "USD",
                "asset_issuer": DESTINATION,
                "balance": "9.5",
                "buying_liabilities": "0.25",
                "selling_liabilities": "0",
                "limit": "10",
                "is_authorized": true
            }]
        });
'''
new_destination = '''        let destination = LedgerAccountState {
            account: account_state("GHOLDER", 0),
            balances: vec![LedgerBalanceState {
                asset: crate::BalanceAsset::Issued {
                    code: "USD".to_owned(),
                    issuer: DESTINATION.to_owned(),
                },
                balance: crate::parse_stroops("9.5", false).unwrap(),
                selling_liabilities: 0,
                buying_liabilities: crate::parse_stroops("0.25", false).unwrap(),
                limit: Some(crate::parse_stroops("10", false).unwrap()),
                authorization: Some(LedgerTrustlineAuthorization::Full),
                clawback_enabled: Some(false),
            }],
            flags: Default::default(),
            memo_required: false,
        };
'''
assert old_destination in text
text = text.replace(old_destination, new_destination, 1)
old_unauthorized = '''        let mut unauthorized = destination;
        unauthorized["balances"][0]["is_authorized"] = Value::Bool(false);
        assert!(validate_destination_receive(&unauthorized, holder, &asset, 1,).is_err());
'''
new_unauthorized = '''        let mut unauthorized = destination;
        unauthorized.balances[0].authorization = Some(LedgerTrustlineAuthorization::Unauthorized);
        assert!(validate_destination_receive(&unauthorized, holder, &asset, 1,).is_err());
'''
assert old_unauthorized in text
text = text.replace(old_unauthorized, new_unauthorized, 1)
old_native_destination = '''        let destination = serde_json::json!({
            "balances": [{
                "asset_type": "native",
                "balance": "922337203685.4775800",
                "buying_liabilities": "0",
                "selling_liabilities": "0"
            }]
        });
'''
new_native_destination = '''        let destination = LedgerAccountState {
            account: account_state(DESTINATION, 0),
            balances: vec![native_balance("922337203685.4775800")],
            flags: Default::default(),
            memo_required: false,
        };
'''
assert old_native_destination in text
text = text.replace(old_native_destination, new_native_destination, 1)
old_sep29 = '''    #[test]
    fn sep29_memo_required_decodes_horizon_account_data() {
        let account = serde_json::json!({
            "data": {"config.memo_required": "MQ=="}
        });
        assert!(account_requires_memo(&account).unwrap());
        let unset = serde_json::json!({"data": {}});
        assert!(!account_requires_memo(&unset).unwrap());
    }

'''
assert old_sep29 in text
text = text.replace(old_sep29, "", 1)
payment.write_text(text)

replace_once(
    "docs/capabilities/payment.md",
    "The current Rust reference implementation is `reference/rust-client::payment` and is consumed by both CLI and TUI.",
    "The current Rust reference implementation is `reference/rust-client::payment` and is consumed by both CLI and TUI. Its write preflight consumes a crate-private provider-neutral ledger account state; Horizon JSON is normalized before Payment balance, reserve, authorization, receiving-capacity and SEP-29 memo semantics are evaluated.",
)
