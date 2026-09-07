use serde_json::Value as JsonValue;

use crate::history_state::HistoryOperation;

/// Keep the first complete-transaction history surface bounded: each transaction requires one
/// additional Horizon operations request until a transaction-expanded provider API exists.
pub const MAX_TRANSACTION_HISTORY_LIMIT: usize = 50;

/// A complete successful Stellar transaction in which the account participated.
///
/// This is a narrow History read model, not a universal cross-platform Activity DTO. The Client
/// guarantees that `operations` contains the complete transaction operation set in ledger order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryTransaction {
    pub paging_token: String,
    pub transaction_hash: String,
    pub created_at: String,
    pub source_account: String,
    pub operations: Vec<HistoryOperation>,
}

impl HistoryTransaction {
    pub(crate) fn from_horizon(
        transaction: &JsonValue,
        raw_operations: &[JsonValue],
    ) -> Result<Self, String> {
        let paging_token =
            required_scalar(transaction, "paging_token", "transaction paging token")?;
        let transaction_hash = required_text(transaction, "hash", "transaction hash")?;
        let created_at = required_text(transaction, "created_at", "transaction timestamp")?;
        let source_account = required_text(transaction, "source_account", "transaction source")?;
        let successful = transaction
            .get("successful")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| "Horizon returned a transaction without success state".to_owned())?;
        if !successful {
            return Err(format!(
                "Horizon returned failed transaction {transaction_hash} in successful account history"
            ));
        }
        let operation_count = required_usize(transaction, "operation_count", "operation count")?;
        if raw_operations.len() != operation_count {
            return Err(format!(
                "Horizon returned incomplete operations for transaction {transaction_hash}: expected {operation_count}, got {}",
                raw_operations.len()
            ));
        }

        let operations = raw_operations
            .iter()
            .map(HistoryOperation::from_horizon)
            .map(|operation| {
                match operation.transaction_hash.as_deref() {
                    Some(hash) if hash == transaction_hash => Ok(operation),
                    Some(hash) => Err(format!(
                        "Horizon returned operation from transaction {hash} while loading {transaction_hash}"
                    )),
                    None => Err(format!(
                        "Horizon returned operation without transaction identity while loading {transaction_hash}"
                    )),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            paging_token,
            transaction_hash,
            created_at,
            source_account,
            operations,
        })
    }
}

fn required_text(value: &JsonValue, key: &str, label: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("Horizon returned a transaction without {label}"))
}

fn required_scalar(value: &JsonValue, key: &str, label: &str) -> Result<String, String> {
    match value.get(key) {
        Some(JsonValue::String(value)) => Ok(value.clone()),
        Some(JsonValue::Number(value)) => Ok(value.to_string()),
        _ => Err(format!("Horizon returned a transaction without {label}")),
    }
}

fn required_usize(value: &JsonValue, key: &str, label: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(JsonValue::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("Horizon returned a transaction without valid {label}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::HistoryOperationKind;

    fn transaction(operation_count: usize) -> JsonValue {
        json!({
            "paging_token": "778899",
            "hash": "txhash",
            "created_at": "2026-09-07T03:30:00Z",
            "source_account": "GSOURCE",
            "successful": true,
            "operation_count": operation_count,
        })
    }

    fn payment(id: &str, from: &str, to: &str) -> JsonValue {
        json!({
            "id": id,
            "paging_token": id,
            "transaction_hash": "txhash",
            "created_at": "2026-09-07T03:30:00Z",
            "source_account": "GSOURCE",
            "type": "payment",
            "from": from,
            "to": to,
            "amount": "1.0000000",
            "asset_type": "native",
        })
    }

    #[test]
    fn groups_complete_operations_in_provider_order() {
        let raw_operations = vec![
            payment("100", "GSOURCE", "GONE"),
            payment("101", "GONE", "GTWO"),
        ];
        let history = HistoryTransaction::from_horizon(&transaction(2), &raw_operations).unwrap();

        assert_eq!(history.paging_token, "778899");
        assert_eq!(history.transaction_hash, "txhash");
        assert_eq!(history.operations.len(), 2);
        assert_eq!(history.operations[0].operation_id.as_deref(), Some("100"));
        assert_eq!(history.operations[1].operation_id.as_deref(), Some("101"));
        assert!(matches!(
            history.operations[0].kind,
            HistoryOperationKind::Payment { .. }
        ));
    }

    #[test]
    fn rejects_partial_transaction_groups() {
        let error =
            HistoryTransaction::from_horizon(&transaction(2), &[payment("100", "GSOURCE", "GONE")])
                .unwrap_err();

        assert!(error.contains("incomplete operations"));
        assert!(error.contains("expected 2, got 1"));
    }

    #[test]
    fn rejects_operation_from_another_transaction() {
        let mut operation = payment("100", "GSOURCE", "GONE");
        operation["transaction_hash"] = json!("other-tx");

        let error = HistoryTransaction::from_horizon(&transaction(1), &[operation]).unwrap_err();
        assert!(error.contains("other-tx"));
        assert!(error.contains("txhash"));
    }

    #[test]
    fn rejects_failed_transaction_from_default_history() {
        let mut failed = transaction(0);
        failed["successful"] = json!(false);

        assert!(HistoryTransaction::from_horizon(&failed, &[])
            .unwrap_err()
            .contains("failed transaction"));
    }
}
