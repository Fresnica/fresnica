# Fresnica SDK Process Binding

Status: pre-release Process Binding API v1.

The Process Binding is an optional stdin/stdout adapter over `fresnica-sdk`. It exists for RefPython, conformance tools, Electron-style Desktop hosts and other non-Rust consumers that need reviewed SDK/Core semantics without reimplementing cryptography. Rust applications normally link `fresnica-sdk` directly; native applications normally use the Native SDK.

It owns one-request/one-response JSON/base64 transport and stable process error/version mapping only. It does not own cryptography, persistence, networking, OS authentication or product Flows. Sensitive passphrases, mnemonics, secrets and WalletUnlockKey values travel over stdin, never argv/environment variables.

Binary: `fresnica-process`; environment discovery: `FRESNICA_PROCESS_BIN`. API v1 operations are version, parse-account, protect-secret/mnemonic, generate/derive mnemonic signer, reprotect, derive/validate unlock key, sign with unlock key or fresh passphrase, reveal, and external Ed25519 prepare/apply.

Desktop selection: Rust/Tauri backend -> direct SDK; Swift/native -> Native SDK; Electron/non-Rust -> Process Binding when a managed sidecar is appropriate. The host owns process integrity/update, secure storage and OS authorization.
