# Fresnica SDK Process Binding

Status: pre-release Process Binding API v2.

The Process Binding is an optional stdin/stdout adapter over `fresnica-sdk`. It exists for RefPython, conformance tools, Electron-style Desktop hosts and other non-Rust consumers that need reviewed SDK/Core semantics without reimplementing cryptography. Rust applications normally link `fresnica-sdk` directly; native applications normally use the Native SDK.

It owns one-request/one-response JSON/base64 transport and stable process error/version mapping only. It does not own cryptography, persistence, networking, OS authentication or product Flows. Sensitive passphrases, mnemonics, secrets and WalletUnlockKey values travel over stdin, never argv/environment variables.

Binary: `fresnica-process`; environment discovery: `FRESNICA_PROCESS_BIN`. API v2 keeps the API v1 owner operations and adds the SDK v4 Soroban authorization-entry surface: protected-unlock signing, fresh-passcode signing, external Ed25519 prepare, and verified signature apply. These are transport wrappers only; authorization preimage/hash construction and signature validation remain authoritative in SDK/Core.

Desktop selection: Rust/Tauri backend -> direct SDK; Swift/native -> Native SDK; Electron/non-Rust -> Process Binding when a managed sidecar is appropriate. The host owns process integrity/update, secure storage and OS authorization.

## Security classification

The Process Binding is a privileged owner/host surface. Its one-shot transport reduces accidental argv/environment leakage, but it does not authenticate the parent process and it intentionally returns owner-sensitive results for operations such as mnemonic generation, Reveal and unlock-key derivation. The host must prevent untrusted renderers/plugins/processes from invoking the binary or observing its pipes, logs, crash reports and temporary state.

It must not be reused as Agent Access or exposed through remote RPC/MCP. Agent Access requires a separate transaction-policy surface that never offers Reveal, passphrase, secret/mnemonic or raw-unlock-key operations. Before a non-RefPython Desktop product adopts Process Binding, review whether that host needs the full owner API or a narrower operation profile.
