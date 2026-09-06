use serde_json::Value as JsonValue;

/// Provider-neutral Classic asset identity used by the current Rust History read model.
///
/// History is still a Defined capability rather than a frozen cross-platform DTO. This type
/// intentionally carries only protocol-level asset identity and no provider or presentation data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryAsset {
    Native,
    Issued { code: String, issuer: String },
}

impl HistoryAsset {
    pub fn identity(&self) -> String {
        match self {
            Self::Native => "XLM".to_owned(),
            Self::Issued { code, issuer } => format!("{code}:{issuer}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryTrustAsset {
    Classic(HistoryAsset),
    LiquidityPool { liquidity_pool_id: String },
    Unknown,
}

/// Provider-neutral semantic fields for an account-scoped Stellar operation.
///
/// This is deliberately narrower than the richer Python `ActivityView`: transaction grouping,
/// cache policy, spam classification and presentation enrichment remain separate concerns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryOperationKind {
    Payment {
        from: Option<String>,
        to: Option<String>,
        amount: Option<String>,
        asset: Option<HistoryAsset>,
    },
    CreateAccount {
        funder: Option<String>,
        account: Option<String>,
        starting_balance: Option<String>,
    },
    ManageSellOffer {
        offer_id: Option<String>,
        amount: Option<String>,
        selling_asset: Option<HistoryAsset>,
        buying_asset: Option<HistoryAsset>,
        price: Option<String>,
    },
    CreatePassiveSellOffer {
        offer_id: Option<String>,
        amount: Option<String>,
        selling_asset: Option<HistoryAsset>,
        buying_asset: Option<HistoryAsset>,
        price: Option<String>,
    },
    ManageBuyOffer {
        offer_id: Option<String>,
        amount: Option<String>,
        selling_asset: Option<HistoryAsset>,
        buying_asset: Option<HistoryAsset>,
        price: Option<String>,
    },
    ChangeTrust {
        asset: HistoryTrustAsset,
        limit: Option<String>,
    },
    InvokeHostFunction,
    LiquidityPoolDeposit,
    LiquidityPoolWithdraw,
    AccountMerge {
        into: Option<String>,
    },
    ManageData {
        name: Option<String>,
    },
    SetOptions,
    BumpSequence {
        bump_to: Option<String>,
    },
    Other {
        operation_type: String,
    },
}

impl HistoryOperationKind {
    pub fn operation_type(&self) -> &str {
        match self {
            Self::Payment { .. } => "payment",
            Self::CreateAccount { .. } => "create_account",
            Self::ManageSellOffer { .. } => "manage_sell_offer",
            Self::CreatePassiveSellOffer { .. } => "create_passive_sell_offer",
            Self::ManageBuyOffer { .. } => "manage_buy_offer",
            Self::ChangeTrust { .. } => "change_trust",
            Self::InvokeHostFunction => "invoke_host_function",
            Self::LiquidityPoolDeposit => "liquidity_pool_deposit",
            Self::LiquidityPoolWithdraw => "liquidity_pool_withdraw",
            Self::AccountMerge { .. } => "account_merge",
            Self::ManageData { .. } => "manage_data",
            Self::SetOptions => "set_options",
            Self::BumpSequence { .. } => "bump_sequence",
            Self::Other { operation_type } => operation_type,
        }
    }
}

/// Account-scoped operation identity plus provider-neutral semantic fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryOperation {
    pub operation_id: Option<String>,
    pub paging_token: Option<String>,
    pub transaction_hash: Option<String>,
    pub created_at: Option<String>,
    pub source_account: Option<String>,
    pub kind: HistoryOperationKind,
}

impl HistoryOperation {
    pub(crate) fn from_horizon(value: &JsonValue) -> Self {
        let operation_type = text(value, "type").unwrap_or("unknown");
        let kind = match operation_type {
            "payment" => HistoryOperationKind::Payment {
                from: text_owned(value, "from").or_else(|| text_owned(value, "source_account")),
                to: text_owned(value, "to"),
                amount: scalar_owned(value, "amount"),
                asset: history_asset(value, ""),
            },
            "create_account" => HistoryOperationKind::CreateAccount {
                funder: text_owned(value, "funder").or_else(|| text_owned(value, "source_account")),
                account: text_owned(value, "account"),
                starting_balance: scalar_owned(value, "starting_balance"),
            },
            "manage_sell_offer" => HistoryOperationKind::ManageSellOffer {
                offer_id: scalar_owned(value, "offer_id"),
                amount: scalar_owned(value, "amount"),
                selling_asset: history_asset(value, "selling_"),
                buying_asset: history_asset(value, "buying_"),
                price: scalar_owned(value, "price"),
            },
            "create_passive_sell_offer" => HistoryOperationKind::CreatePassiveSellOffer {
                offer_id: scalar_owned(value, "offer_id"),
                amount: scalar_owned(value, "amount"),
                selling_asset: history_asset(value, "selling_"),
                buying_asset: history_asset(value, "buying_"),
                price: scalar_owned(value, "price"),
            },
            "manage_buy_offer" => HistoryOperationKind::ManageBuyOffer {
                offer_id: scalar_owned(value, "offer_id"),
                amount: scalar_owned(value, "amount"),
                selling_asset: history_asset(value, "selling_"),
                buying_asset: history_asset(value, "buying_"),
                price: scalar_owned(value, "price"),
            },
            "change_trust" => HistoryOperationKind::ChangeTrust {
                asset: history_trust_asset(value),
                limit: scalar_owned(value, "limit"),
            },
            "invoke_host_function" => HistoryOperationKind::InvokeHostFunction,
            "liquidity_pool_deposit" => HistoryOperationKind::LiquidityPoolDeposit,
            "liquidity_pool_withdraw" => HistoryOperationKind::LiquidityPoolWithdraw,
            "account_merge" => HistoryOperationKind::AccountMerge {
                into: text_owned(value, "into").or_else(|| text_owned(value, "account")),
            },
            "manage_data" => HistoryOperationKind::ManageData {
                name: text_owned(value, "name"),
            },
            "set_options" => HistoryOperationKind::SetOptions,
            "bump_sequence" => HistoryOperationKind::BumpSequence {
                bump_to: scalar_owned(value, "bump_to"),
            },
            other => HistoryOperationKind::Other {
                operation_type: other.to_owned(),
            },
        };

        Self {
            operation_id: scalar_owned(value, "id"),
            paging_token: scalar_owned(value, "paging_token"),
            transaction_hash: text_owned(value, "transaction_hash"),
            created_at: text_owned(value, "created_at"),
            source_account: text_owned(value, "source_account"),
            kind,
        }
    }

    pub fn operation_type(&self) -> &str {
        self.kind.operation_type()
    }
}

fn history_asset(value: &JsonValue, prefix: &str) -> Option<HistoryAsset> {
    match text(value, &format!("{prefix}asset_type")) {
        Some("native") => Some(HistoryAsset::Native),
        Some("credit_alphanum4") | Some("credit_alphanum12") => {
            let code = text_owned(value, &format!("{prefix}asset_code"))?;
            let issuer = text_owned(value, &format!("{prefix}asset_issuer"))?;
            Some(HistoryAsset::Issued { code, issuer })
        }
        _ => None,
    }
}

fn history_trust_asset(value: &JsonValue) -> HistoryTrustAsset {
    if text(value, "asset_type") == Some("liquidity_pool_shares") {
        return text_owned(value, "liquidity_pool_id")
            .map(|liquidity_pool_id| HistoryTrustAsset::LiquidityPool { liquidity_pool_id })
            .unwrap_or(HistoryTrustAsset::Unknown);
    }
    history_asset(value, "")
        .map(HistoryTrustAsset::Classic)
        .unwrap_or(HistoryTrustAsset::Unknown)
}

fn text<'a>(value: &'a JsonValue, key: &str) -> Option<&'a str> {
    value.get(key).and_then(JsonValue::as_str)
}

fn text_owned(value: &JsonValue, key: &str) -> Option<String> {
    text(value, key).map(str::to_owned)
}

fn scalar_owned(value: &JsonValue, key: &str) -> Option<String> {
    match value.get(key)? {
        JsonValue::String(value) => Some(value.clone()),
        JsonValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ISSUER: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn payment_preserves_chain_identity_and_semantics() {
        let operation = HistoryOperation::from_horizon(&json!({
            "id": "101",
            "paging_token": "101",
            "transaction_hash": "abc123",
            "created_at": "2026-09-06T12:00:00Z",
            "source_account": "GSOURCE",
            "type": "payment",
            "from": "GSOURCE",
            "to": "GDESTINATION",
            "amount": "1.2500000",
            "asset_type": "native"
        }));

        assert_eq!(operation.operation_id.as_deref(), Some("101"));
        assert_eq!(operation.paging_token.as_deref(), Some("101"));
        assert_eq!(operation.transaction_hash.as_deref(), Some("abc123"));
        assert_eq!(operation.operation_type(), "payment");
        assert_eq!(
            operation.kind,
            HistoryOperationKind::Payment {
                from: Some("GSOURCE".to_owned()),
                to: Some("GDESTINATION".to_owned()),
                amount: Some("1.2500000".to_owned()),
                asset: Some(HistoryAsset::Native),
            }
        );
    }

    #[test]
    fn offer_assets_keep_full_code_and_issuer_identity() {
        let operation = HistoryOperation::from_horizon(&json!({
            "type": "manage_buy_offer",
            "offer_id": "42",
            "amount": "20.4521401",
            "price": "0.0300003",
            "buying_asset_type": "native",
            "selling_asset_type": "credit_alphanum4",
            "selling_asset_code": "EURT",
            "selling_asset_issuer": ISSUER
        }));

        let HistoryOperationKind::ManageBuyOffer {
            buying_asset,
            selling_asset,
            ..
        } = operation.kind
        else {
            panic!("expected manage-buy offer");
        };
        assert_eq!(buying_asset.unwrap().identity(), "XLM");
        assert_eq!(selling_asset.unwrap().identity(), format!("EURT:{ISSUER}"));
    }

    #[test]
    fn liquidity_pool_trustline_keeps_pool_identity() {
        let operation = HistoryOperation::from_horizon(&json!({
            "type": "change_trust",
            "asset_type": "liquidity_pool_shares",
            "liquidity_pool_id": "abcdef0123456789",
            "limit": "922337203685.4775807"
        }));

        assert_eq!(
            operation.kind,
            HistoryOperationKind::ChangeTrust {
                asset: HistoryTrustAsset::LiquidityPool {
                    liquidity_pool_id: "abcdef0123456789".to_owned(),
                },
                limit: Some("922337203685.4775807".to_owned()),
            }
        );
    }

    #[test]
    fn unknown_operation_keeps_type_and_chain_identity() {
        let operation = HistoryOperation::from_horizon(&json!({
            "id": 77,
            "paging_token": "77",
            "transaction_hash": "future-tx",
            "type": "future_protocol_operation"
        }));

        assert_eq!(operation.operation_id.as_deref(), Some("77"));
        assert_eq!(operation.transaction_hash.as_deref(), Some("future-tx"));
        assert_eq!(operation.operation_type(), "future_protocol_operation");
        assert_eq!(
            operation.kind,
            HistoryOperationKind::Other {
                operation_type: "future_protocol_operation".to_owned(),
            }
        );
    }
}
