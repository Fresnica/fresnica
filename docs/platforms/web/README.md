# Web Platform References

Web implementations use the shared Application Flow/Capability contracts but require browser-specific security and storage decisions.

- [WASM / Web security boundary](wasm-security.md)

Do not copy Mobile Keychain/Keystore or `WalletUnlockKey` assumptions into the browser without an explicit Web security design.

Provider-specific smart-account/passkey work is documented separately at [`../../capabilities/passkey-smart-account.md`](../../capabilities/passkey-smart-account.md).
