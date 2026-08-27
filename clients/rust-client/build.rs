use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn apply_patch(root: &PathBuf, patch: &PathBuf) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("apply")
        .arg("--check")
        .arg(patch)
        .status()
        .expect("check validation patch");
    assert!(status.success(), "check {}", patch.display());

    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("apply")
        .arg(patch)
        .status()
        .expect("apply validation patch");
    assert!(status.success(), "apply {}", patch.display());
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let marker = root.join(".git/fresnica-local-multisig-validation-applied");
    if marker.exists() {
        return;
    }

    let patches = root.join("validation/local-multisig");
    for name in [
        "anchor.patch",
        "transaction-flow.patch",
        "dex.patch",
        "ledger-authorization.patch",
        "lib.patch",
        "payment.patch",
        "signing-coordination.patch",
        "transaction.patch",
        "trustline.patch",
        "test-xdr-fix.patch",
        "sep10-small.patch",
        "sep10-anchor-protocol.patch",
    ] {
        apply_patch(&root, &patches.join(name));
    }
    fs::write(marker, b"ok").expect("write validation marker");
}
