# Historical Rust Core Process Protocol

Status: **superseded**.

The original `fresnica-core` stdin/stdout protocol was the first transport used to prove that RefPython could delegate cryptographic operations to Rust Core. It has been retired from `main` now that the semantic boundary is `Fresnica SDK -> bindings/process -> host`.

Current process consumers use [`../sdk/process-binding.md`](../sdk/process-binding.md). The Process Binding is versioned independently, delegates every wallet/security operation through `fresnica-sdk`, and keeps persistence, networking, OS authentication and UI outside the binding.

Historical protocol v2 remains available in Git history and pre-retirement commits for archaeology; it is not an active compatibility surface.
