use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../..")
        .canonicalize()
        .expect("repo root");
    let marker = root.join(".validation-typed-signers-applied");
    if marker.exists() {
        return;
    }

    for name in [
        "core-transaction.patch",
        "ledger-authorization.patch",
        "remaining.patch",
        "test-import.patch",
    ] {
        let patch = root.join("validation/typed-signers").join(name);
        println!("cargo:rerun-if-changed={}", patch.display());
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("apply")
            .arg(&patch)
            .status()
            .expect("run git apply");
        assert!(status.success(), "apply validation patch {name}");
    }

    fs::write(marker, b"applied").expect("write validation marker");
}
