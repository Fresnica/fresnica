# Repository Governance and Required CI

`main` should require pull requests, block force-push/deletion, require an up-to-date branch and resolved conversations, and require the stable check `Required CI / validate`.

`.github/workflows/ci-required.yml` owns shared semantic correctness: SDK compatibility metadata, Rust formatting/tests/builds for Core/SDK/Process/Rust clients, the SDK-boundary guard, and RefPython integration through the Process Binding. Native packaging, React Native, WASM, release and live-Testnet workflows remain separate path/toolchain gates.

`Main bundle` remains a post-merge synchronization artifact, not the correctness gate. Temporary probe/relay PRs should be closed and their branches deleted once conclusions are absorbed into code/tests/docs.
