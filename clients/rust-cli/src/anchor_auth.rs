use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use fresnica_core::{
    parse_transaction_envelope_xdr, verify_transaction_envelope_signature,
};
use stellar_xdr::{
    AccountId, Memo, MuxedAccount, OperationBody, Preconditions, PublicKey,
    TransactionEnvelope,
};

const SEP10_CLOCK_GRACE_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSep10Challenge {
    pub client_account: String,
    pub matched_home_domain: String,
}

pub fn verify_sep10_challenge(
    challenge_xdr: &[u8],
    server_signing_key: &str,
    client_account: &str,
    home_domain: &str,
    web_auth_domain: &str,
    network_passphrase: &str,
    now_unix: u64,
) -> Result<VerifiedSep10Challenge, String> {
    let envelope = parse_transaction_envelope_xdr(challenge_xdr)
        .map_err(|error| format!("invalid SEP-10 challenge XDR: {error}"))?;
    let TransactionEnvelope::Tx(value) = &envelope else {
        return Err("SEP-10 challenge must use a classic V1 transaction envelope".to_owned());
    };
    let transaction = &value.tx;
    let server = classic_muxed_account(server_signing_key, "server signing key")?;
    let client = classic_muxed_account(client_account, "client account")?;

    if transaction.source_account != server {
        return Err("SEP-10 transaction source does not match the server signing key".to_owned());
    }
    if transaction.seq_num.0 != 0 {
        return Err("SEP-10 transaction sequence number must be zero".to_owned());
    }

    let bounds = match &transaction.cond {
        Preconditions::Time(bounds) => bounds,
        Preconditions::V2(value) => value
            .time_bounds
            .as_ref()
            .ok_or_else(|| "SEP-10 challenge requires time bounds".to_owned())?,
        Preconditions::None => return Err("SEP-10 challenge requires time bounds".to_owned()),
    };
    let min_time = bounds.min_time.0;
    let max_time = bounds.max_time.0;
    if max_time == 0 {
        return Err("SEP-10 challenge requires a finite maximum time".to_owned());
    }
    if now_unix.saturating_add(SEP10_CLOCK_GRACE_SECONDS) < min_time || now_unix > max_time {
        return Err("SEP-10 challenge is outside its valid time bounds".to_owned());
    }

    if !matches!(&transaction.memo, Memo::None | Memo::Id(_)) {
        return Err("SEP-10 challenge only permits an ID memo".to_owned());
    }

    let first = transaction
        .operations
        .first()
        .ok_or_else(|| "SEP-10 challenge must contain at least one operation".to_owned())?;
    if first.source_account.as_ref() != Some(&client) {
        return Err("SEP-10 first operation source does not match the client account".to_owned());
    }
    let OperationBody::ManageData(first_data) = &first.body else {
        return Err("SEP-10 first operation must be ManageData".to_owned());
    };
    let first_name = utf8_name(first_data.data_name.as_slice(), "first ManageData key")?;
    if first_name != format!("{home_domain} auth") {
        return Err("SEP-10 first ManageData key does not match the expected home domain".to_owned());
    }
    let nonce = first_data
        .data_value
        .as_ref()
        .ok_or_else(|| "SEP-10 nonce must not be null".to_owned())?;
    if nonce.len() != 64 {
        return Err("SEP-10 nonce must be 64 bytes of base64 text".to_owned());
    }
    let decoded_nonce = STANDARD
        .decode(nonce.as_slice())
        .map_err(|_| "SEP-10 nonce is not valid base64".to_owned())?;
    if decoded_nonce.len() != 48 {
        return Err("SEP-10 nonce must decode to 48 bytes".to_owned());
    }

    let mut saw_web_auth_domain = false;
    for operation in transaction.operations.iter().skip(1) {
        let OperationBody::ManageData(data) = &operation.body else {
            return Err("SEP-10 subsequent operations must be ManageData".to_owned());
        };
        let name = utf8_name(data.data_name.as_slice(), "ManageData key")?;
        if name == "client_domain" {
            return Err(
                "SEP-10 client_domain challenges are not supported unless the client requested attribution"
                    .to_owned(),
            );
        }
        if operation.source_account.as_ref() != Some(&server) {
            return Err("SEP-10 subsequent operation source must be the server account".to_owned());
        }
        if name == "web_auth_domain" {
            if saw_web_auth_domain {
                return Err("SEP-10 challenge contains duplicate web_auth_domain operations".to_owned());
            }
            saw_web_auth_domain = true;
            let value = data
                .data_value
                .as_ref()
                .ok_or_else(|| "SEP-10 web_auth_domain must not be null".to_owned())?;
            if value.as_slice() != web_auth_domain.as_bytes() {
                return Err("SEP-10 web_auth_domain does not match the authentication server".to_owned());
            }
        }
    }

    verify_transaction_envelope_signature(&envelope, network_passphrase, server_signing_key)
        .map_err(|_| "SEP-10 challenge is not signed by the discovered server key".to_owned())?;

    Ok(VerifiedSep10Challenge {
        client_account: client_account.to_owned(),
        matched_home_domain: home_domain.to_owned(),
    })
}

fn classic_muxed_account(value: &str, label: &str) -> Result<MuxedAccount, String> {
    let account = AccountId::from_str(value)
        .map_err(|_| format!("SEP-10 {label} must be a Classic G address"))?;
    match account.0 {
        PublicKey::PublicKeyTypeEd25519(key) => Ok(MuxedAccount::Ed25519(key)),
    }
}

fn utf8_name<'a>(value: &'a [u8], label: &str) -> Result<&'a str, String> {
    std::str::from_utf8(value).map_err(|_| format!("SEP-10 {label} is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::StringM;

    const MAINNET: &str = "Public Global Stellar Network ; September 2015";
    const TESTNET: &str = "Test SDF Network ; September 2015";
    const SERVER: &str = "GDEISG5WA25KU6HHB7N4HVQKID4A7FDDR3FKD32R6C7KCV7YLYKVY7S7";
    const CLIENT: &str = "GBAQD4VYNI2255CFRDNDM4LVAEITMCNS7HJCI7I46XJE756ITCJXLV7E";
    const HOME_DOMAIN: &str = "thisisatest.sandbox.anchor.anchordomain.com";
    const CHALLENGE: &str = concat!(
        "AAAAAgAAAADIiRu2BrqqeOcP28PWCkD4D5Rjjsqh71HwvqFX+F4VXAAAAGQAAAAAAAAA",
        "AAAAAAEAAAAAXzrUcQAAAABfOtf1AAAAAAAAAAEAAAABAAAAAEEB8rhqNa70RYjaNnF1",
        "ARE2CbL50iR9HPXST/fImJN1AAAACgAAADB0aGlzaXNhdGVzdC5zYW5kYm94LmFuY2hv",
        "ci5hbmNob3Jkb21haW4uY29tIGF1dGgAAAABAAAAQGdGOFlIQm1zaGpEWEY0L0VJUFZu",
        "cGVlRkxVTDY2V0tKMVBPYXZuUVVBNjBoL09XaC91M2Vvdk54WFJtSTAvQ2UAAAAAAAAA",
        "AfheFVwAAABAheKE1HjGnUCNwPbX8mz7CqotShKbA+xM2Hbjl6X0TBpEprVOUVjA6lqM",
        "J1j62vrxn1mF3eJzsLa9s9hRofG3Ag=="
    );

    fn challenge_xdr() -> Vec<u8> {
        STANDARD.decode(CHALLENGE).unwrap()
    }

    fn verify(xdr: &[u8], now: u64) -> Result<VerifiedSep10Challenge, String> {
        verify_sep10_challenge(
            xdr,
            SERVER,
            CLIENT,
            HOME_DOMAIN,
            "auth.example.com",
            MAINNET,
            now,
        )
    }

    #[test]
    fn verifies_official_sep10_challenge_shape_and_server_signature() {
        let verified = verify(&challenge_xdr(), 1_597_691_000).unwrap();
        assert_eq!(verified.client_account, CLIENT);
        assert_eq!(verified.matched_home_domain, HOME_DOMAIN);
    }

    #[test]
    fn rejects_wrong_network_server_signature() {
        let xdr = challenge_xdr();
        let result = verify_sep10_challenge(
            &xdr,
            SERVER,
            CLIENT,
            HOME_DOMAIN,
            "auth.example.com",
            TESTNET,
            1_597_691_000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_challenge_for_another_client() {
        let result = verify_sep10_challenge(
            &challenge_xdr(),
            SERVER,
            SERVER,
            HOME_DOMAIN,
            "auth.example.com",
            MAINNET,
            1_597_691_000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_expired_challenge() {
        assert!(verify(&challenge_xdr(), 1_597_691_894).is_err());
    }

    #[test]
    fn rejects_wrong_home_domain_before_client_signing() {
        let result = verify_sep10_challenge(
            &challenge_xdr(),
            SERVER,
            CLIENT,
            "attacker.example",
            "auth.example.com",
            MAINNET,
            1_597_691_000,
        );
        assert!(result.is_err());
    }

    #[test]
    fn rejects_malformed_first_manage_data_key() {
        let mut envelope = parse_transaction_envelope_xdr(&challenge_xdr()).unwrap();
        let TransactionEnvelope::Tx(value) = &mut envelope else {
            unreachable!();
        };
        let OperationBody::ManageData(data) = &mut value.tx.operations[0].body else {
            unreachable!();
        };
        data.data_name = stellar_xdr::String64(
            StringM::<64>::try_from("attacker.example auth").unwrap(),
        );
        let mutated = fresnica_core::transaction_envelope_xdr(&envelope).unwrap();

        assert!(verify(&mutated, 1_597_691_000).is_err());
    }
}
