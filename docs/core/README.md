# Fresnica Core References

These documents describe detailed Rust Core / SDK security and protocol contracts.

Start with the short cross-platform authority document first:

- [`../core-security-boundary.md`](../core-security-boundary.md)

Then use these references when changing the relevant implementation boundary:

- [Core client protocol](client-protocol.md) - versioned Core Client API/process/native semantics.
- [Client/Core security](client-security.md) - account/signer/client security model in detail.
- [Signer architecture](signer.md) - signer provider and account relationship design.
- [Software signer protection](protection.md) - protected signer envelope and authorization details.
- [Reveal / export](secret-export.md) - high-privilege recovery-material disclosure boundary.

These files may contain Rust/API implementation detail. They do not replace the Application Capability contracts.
