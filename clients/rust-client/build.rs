use std::env;
use std::path::PathBuf;
use std::process::Command;

fn git_apply(root: &PathBuf, patch: &PathBuf, reverse: bool, check: bool) -> bool {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).arg("apply");
    if reverse {
        command.arg("--reverse");
    }
    if check {
        command.arg("--check");
    }
    command.arg(patch);
    command.status().expect("run git apply").success()
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
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
    ] {
        let patch = patches.join(name);
        println!("cargo:rerun-if-changed={}", patch.display());
        if git_apply(&root, &patch, false, true) {
            assert!(git_apply(&root, &patch, false, false), "apply {name}");
        } else {
            assert!(
                git_apply(&root, &patch, true, true),
                "validation patch is neither applicable nor already applied: {name}"
            );
        }
    }
}
