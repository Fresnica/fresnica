# Fresnica Process Binding

`fresnica-process-binding` is the optional versioned stdin/stdout adapter over `fresnica-sdk` for RefPython, conformance tools and suitable non-Rust Desktop hosts. Rust applications link the SDK directly; native applications normally use the Native SDK.

The host owns persistence, networking, OS authentication, lifecycle and UI. The binding owns JSON/base64/process transport only; SDK/Core remain authoritative for identity, protection, signing and errors. Sensitive values travel over stdin, never argv/environment variables.

`PROCESS_BINDING_API_VERSION` is independent from SDK/Core API versions. Build with `cargo build --release --manifest-path bindings/process/Cargo.toml --bin fresnica-process`.
