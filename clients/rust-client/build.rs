use std::fs;

fn replace_if_needed(path: &str, from: &str, to: &str) {
    let source = fs::read_to_string(path).expect("read validation source");
    if source.contains(to) {
        return;
    }
    assert!(source.contains(from), "expected validation patch target in {path}");
    fs::write(path, source.replacen(from, to, 1)).expect("write validation source");
}

fn remove_if_present(path: &str, text: &str) {
    let source = fs::read_to_string(path).expect("read validation source");
    if source.contains(text) {
        fs::write(path, source.replacen(text, "", 1)).expect("write validation source");
    }
}

fn main() {
    replace_if_needed(
        "src/dex.rs",
        "operations.push(offer_operation(side, &base, &counter, amount, price, 0)?);",
        "operations.push(offer_operation(side, &base, &counter, amount, price.clone(), 0)?);",
    );
    replace_if_needed(
        "src/dex.rs",
        "let body = offer_operation(side, &base, &counter, amount, price, offer_id)?;",
        "let body = offer_operation(side, &base, &counter, amount, price.clone(), offer_id)?;",
    );
    remove_if_present("src/anchor_protocol.rs", "use std::io::Read;\n");
    replace_if_needed(
        "src/anchor_protocol.rs",
        "let wrong = serde_json::json!({\n            \"account_id\": \"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF\",",
        "let wrong = serde_json::json!({\n            \"account_id\": \"GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBU4I\",",
    );

    replace_if_needed(
        "src/ledger_authorization.rs",
        "use stellar_xdr::{\n    AccountId, MuxedAccount, OperationBody, Preconditions, PublicKey, TransactionEnvelope,\n};\n\n#[derive",
        "use stellar_xdr::{\n    AccountId, MuxedAccount, OperationBody, Preconditions, PublicKey, TransactionEnvelope,\n};\n\nuse crate::horizon::HorizonClient;\n\n#[derive",
    );

    replace_if_needed(
        "src/ledger_authorization.rs",
        r#"pub fn plan_classic_ledger_authorization(
    envelope: &TransactionEnvelope,
    accounts: &[LedgerAccountAuthorization],
) -> Result<LedgerAuthorizationPlan, String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err(
            "Ledger Authorization currently supports TransactionV1Envelope only".to_owned(),
        );
    };
    if let Preconditions::V2(preconditions) = &envelope.tx.cond {
        if !preconditions.extra_signers.is_empty() {
            return Err(
                "Ledger Authorization does not yet support PreconditionsV2.extraSigners".to_owned(),
            );
        }
    }

    let mut account_index = BTreeMap::new();
    for account in accounts {
        if account_index
            .insert(account.account_id.as_str(), account)
            .is_some()
        {
            return Err(format!(
                "duplicate ledger authorization state for {}",
                account.account_id
            ));
        }
    }

    let mut requirements = Vec::new();
    let transaction_source = authorization_account_id(&envelope.tx.source_account);
    add_requirement(
        &mut requirements,
        &account_index,
        &transaction_source,
        AuthorizationScope::TransactionSource,
        AuthorizationThreshold::Low,
    )?;

    for (index, operation) in envelope.tx.operations.iter().enumerate() {
        let source = operation
            .source_account
            .as_ref()
            .map(authorization_account_id)
            .unwrap_or_else(|| transaction_source.clone());
        let (kind, threshold) = operation_authorization(&operation.body).ok_or_else(|| {
            format!("Ledger Authorization does not yet support Classic operation #{index}")
        })?;
        add_requirement(
            &mut requirements,
            &account_index,
            &source,
            AuthorizationScope::Operation { index, kind },
            threshold,
        )?;
    }

    Ok(LedgerAuthorizationPlan { requirements })
}
"#,
        r#"pub fn ensure_local_ed25519_signer_can_satisfy(
    plan: &LedgerAuthorizationPlan,
    signer_public_key: &str,
) -> Result<(), String> {
    AccountId::from_str(signer_public_key)
        .map_err(|_| "local ledger-authorization signer must be a Classic G address".to_owned())?;
    let available = BTreeSet::from([LedgerSignerCondition {
        kind: LedgerSignerKind::Ed25519PublicKey,
        key: signer_public_key.to_owned(),
    }]);
    let gaps = plan
        .requirements
        .iter()
        .filter_map(|requirement| {
            let available_weight = requirement.available_weight(&available);
            (available_weight < u32::from(requirement.required_weight)).then(|| {
                format!(
                    "{} requires weight {} but local signer provides {}",
                    requirement.account_id, requirement.required_weight, available_weight
                )
            })
        })
        .collect::<Vec<_>>();
    if gaps.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Direct signing cannot satisfy ledger authorization: {}. Additional signer conditions must be completed through Signing Coordination before submission",
            gaps.join("; ")
        ))
    }
}

pub fn plan_classic_ledger_authorization(
    envelope: &TransactionEnvelope,
    accounts: &[LedgerAccountAuthorization],
) -> Result<LedgerAuthorizationPlan, String> {
    let uses = classic_authorization_uses(envelope)?;
    let mut account_index = BTreeMap::new();
    for account in accounts {
        if account_index
            .insert(account.account_id.as_str(), account)
            .is_some()
        {
            return Err(format!(
                "duplicate ledger authorization state for {}",
                account.account_id
            ));
        }
    }

    let mut requirements = Vec::new();
    for authorization_use in uses {
        add_requirement(
            &mut requirements,
            &account_index,
            &authorization_use.account_id,
            authorization_use.scope,
            authorization_use.threshold,
        )?;
    }
    Ok(LedgerAuthorizationPlan { requirements })
}

pub fn load_classic_ledger_authorization_plan(
    horizon: &HorizonClient,
    envelope: &TransactionEnvelope,
) -> Result<LedgerAuthorizationPlan, String> {
    load_classic_ledger_authorization_plan_with(envelope, |account_id| {
        horizon.get_account(account_id)
    })
}

#[derive(Clone, Debug)]
struct RequiredAuthorizationUse {
    account_id: String,
    scope: AuthorizationScope,
    threshold: AuthorizationThreshold,
}

fn classic_authorization_uses(
    envelope: &TransactionEnvelope,
) -> Result<Vec<RequiredAuthorizationUse>, String> {
    let TransactionEnvelope::Tx(envelope) = envelope else {
        return Err(
            "Ledger Authorization currently supports TransactionV1Envelope only".to_owned(),
        );
    };
    if let Preconditions::V2(preconditions) = &envelope.tx.cond {
        if !preconditions.extra_signers.is_empty() {
            return Err(
                "Ledger Authorization does not yet support PreconditionsV2.extraSigners".to_owned(),
            );
        }
    }

    let transaction_source = authorization_account_id(&envelope.tx.source_account);
    let mut uses = vec![RequiredAuthorizationUse {
        account_id: transaction_source.clone(),
        scope: AuthorizationScope::TransactionSource,
        threshold: AuthorizationThreshold::Low,
    }];
    for (index, operation) in envelope.tx.operations.iter().enumerate() {
        let source = operation
            .source_account
            .as_ref()
            .map(authorization_account_id)
            .unwrap_or_else(|| transaction_source.clone());
        let (kind, threshold) = operation_authorization(&operation.body).ok_or_else(|| {
            format!("Ledger Authorization does not yet support Classic operation #{index}")
        })?;
        uses.push(RequiredAuthorizationUse {
            account_id: source,
            scope: AuthorizationScope::Operation { index, kind },
            threshold,
        });
    }
    Ok(uses)
}

fn load_classic_ledger_authorization_plan_with<F>(
    envelope: &TransactionEnvelope,
    mut fetch_account: F,
) -> Result<LedgerAuthorizationPlan, String>
where
    F: FnMut(&str) -> Result<JsonValue, String>,
{
    let uses = classic_authorization_uses(envelope)?;
    let mut source_accounts = BTreeSet::new();
    for authorization_use in &uses {
        source_accounts.insert(authorization_use.account_id.clone());
    }

    let mut accounts = Vec::with_capacity(source_accounts.len());
    for account_id in source_accounts {
        let raw = fetch_account(&account_id)?;
        let account = LedgerAccountAuthorization::from_horizon(&raw).map_err(|error| {
            format!("Unable to interpret ledger authorization for {account_id}: {error}")
        })?;
        if account.account_id != account_id {
            return Err(format!(
                "Horizon returned ledger authorization for {} while loading {account_id}",
                account.account_id
            ));
        }
        accounts.push(account);
    }
    plan_classic_ledger_authorization(envelope, &accounts)
}
"#,
    );

    replace_if_needed(
        "src/ledger_authorization.rs",
        "use stellar_xdr::{\n    AccountId, MuxedAccount, OperationBody, Preconditions, PublicKey, TransactionEnvelope,\n};\n\nuse crate::horizon::HorizonClient;",
        "use stellar_xdr::{\n    AccountId, MuxedAccount, OperationBody, Preconditions, PublicKey, SignerKey,\n    TransactionEnvelope,\n};\n\nuse crate::horizon::HorizonClient;",
    );
    replace_if_needed(
        "src/ledger_authorization.rs",
        r#"            let key = json_string(signer, "key", "Horizon signer")?.to_owned();
            let kind = match json_string(signer, "type", "Horizon signer")? {
                "ed25519_public_key" => LedgerSignerKind::Ed25519PublicKey,
                "preauth_tx" => LedgerSignerKind::PreauthorizedTransaction,
                "sha256_hash" => LedgerSignerKind::HashX,
                "ed25519_signed_payload" => LedgerSignerKind::Ed25519SignedPayload,
                other => return Err(format!("unsupported Horizon signer type: {other}")),
            };
            if kind == LedgerSignerKind::Ed25519PublicKey {
                AccountId::from_str(&key)
                    .map_err(|_| "Horizon Ed25519 signer has an invalid key".to_owned())?;
            }
"#,
        r#"            let key = json_string(signer, "key", "Horizon signer")?.to_owned();
            let signer_type = json_string(signer, "type", "Horizon signer")?;
            let expected_kind = match signer_type {
                "ed25519_public_key" => LedgerSignerKind::Ed25519PublicKey,
                "preauth_tx" => LedgerSignerKind::PreauthorizedTransaction,
                "sha256_hash" => LedgerSignerKind::HashX,
                "ed25519_signed_payload" => LedgerSignerKind::Ed25519SignedPayload,
                other => return Err(format!("unsupported Horizon signer type: {other}")),
            };
            let actual_kind = match SignerKey::from_str(&key)
                .map_err(|_| "Horizon signer has an invalid StrKey".to_owned())?
            {
                SignerKey::Ed25519(_) => LedgerSignerKind::Ed25519PublicKey,
                SignerKey::PreAuthTx(_) => LedgerSignerKind::PreauthorizedTransaction,
                SignerKey::HashX(_) => LedgerSignerKind::HashX,
                SignerKey::Ed25519SignedPayload(_) => LedgerSignerKind::Ed25519SignedPayload,
            };
            if actual_kind != expected_kind {
                return Err(format!(
                    "Horizon signer type {signer_type} does not match signer key"
                ));
            }
            let kind = expected_kind;
"#,
    );
    replace_if_needed(
        "src/ledger_authorization.rs",
        "TAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "TA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUPUI",
    );
    replace_if_needed(
        "src/ledger_authorization.rs",
        "XAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "XA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVLRR",
    );

    replace_if_needed(
        "src/lib.rs",
        "pub use ledger_authorization::{\n    plan_classic_ledger_authorization, AccountAuthorizationRequirement, AuthorizationScope,",
        "pub use ledger_authorization::{\n    ensure_local_ed25519_signer_can_satisfy, load_classic_ledger_authorization_plan,\n    plan_classic_ledger_authorization, AccountAuthorizationRequirement, AuthorizationScope,",
    );

    replace_if_needed(
        "src/transaction.rs",
        "use crate::{\n    HorizonClient, SubmissionError, WalletRecord, WalletStorage, MAINNET_HORIZON_URL,",
        "use crate::ledger_authorization::{\n    ensure_local_ed25519_signer_can_satisfy, load_classic_ledger_authorization_plan,\n};\nuse crate::{\n    HorizonClient, SubmissionError, WalletRecord, WalletStorage, MAINNET_HORIZON_URL,",
    );
    replace_if_needed(
        "src/transaction.rs",
        "    ensure_transaction_not_expired(envelope)?;\n    let network_passphrase = network_passphrase(network)?;",
        "    ensure_transaction_not_expired(envelope)?;\n    let authorization = load_classic_ledger_authorization_plan(horizon, envelope)?;\n    ensure_local_ed25519_signer_can_satisfy(&authorization, &record.address)?;\n    let network_passphrase = network_passphrase(network)?;",
    );
}
