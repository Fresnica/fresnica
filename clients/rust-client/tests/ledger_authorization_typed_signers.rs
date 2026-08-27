use fresnica_client::{LedgerAccountAuthorization, LedgerSignerKind};

const ACCOUNT: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
const SIGNED_PAYLOAD: &str = "PA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJUAAAAAQACAQDAQCQMBYIBEFAWDANBYHRAEISCMKBKFQXDAMRUGY4DUPB6IBZGM";

fn account_with_signer(key: &str, signer_type: &str) -> serde_json::Value {
    serde_json::json!({
        "account_id": ACCOUNT,
        "thresholds": {
            "low_threshold": 1,
            "med_threshold": 2,
            "high_threshold": 3
        },
        "signers": [{
            "key": key,
            "weight": 1,
            "type": signer_type
        }]
    })
}

#[test]
fn accepts_signed_payload_with_matching_horizon_type() {
    let account = LedgerAccountAuthorization::from_horizon(&account_with_signer(
        SIGNED_PAYLOAD,
        "ed25519_signed_payload",
    ))
    .unwrap();
    assert_eq!(
        account.signers[0].condition.kind,
        LedgerSignerKind::Ed25519SignedPayload
    );
}

#[test]
fn rejects_horizon_signer_type_key_mismatch() {
    let error = LedgerAccountAuthorization::from_horizon(&account_with_signer(ACCOUNT, "preauth_tx"))
        .unwrap_err();
    assert_eq!(
        error,
        "Horizon signer type preauth_tx does not match signer key"
    );
}
