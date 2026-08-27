use std::fs;

fn replace_if_needed(path: &str, from: &str, to: &str) {
    let source = fs::read_to_string(path).expect("read validation source");
    if source.contains(to) {
        return;
    }
    assert!(source.contains(from), "expected validation patch target in {path}");
    fs::write(path, source.replacen(from, to, 1)).expect("write validation source");
}

fn replace_all_if_present(path: &str, from: &str, to: &str) {
    let source = fs::read_to_string(path).expect("read validation source");
    if source.contains(from) {
        fs::write(path, source.replace(from, to)).expect("write validation source");
    }
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
    replace_all_if_present(
        "src/ledger_authorization.rs",
        "data_name: \"auth\".try_into().unwrap(),",
        "data_name: stellar_xdr::StringM::<64>::try_from(\"auth\").unwrap().into(),",
    );
}
