# Fresnica Project Handoff

Updated: 2026-08-26

This is the **continuation/state document** for Fresnica. Stable architecture and security rules are not duplicated here; start at [`README.md`](README.md) and the five common contracts.

Do not treat any SHA in a handoff as permanently current. Verify GitHub `main`, current CI and release metadata before development.

## 1. Canonical architecture

Cross-project vocabulary is fixed as:

```text
Application Flows
  -> Application Capabilities
  -> Fresnica SDK/Core + Stellar/network/repository/platform ports
```

- **Flow**: user goal, product sequence, confirmation, UI/UX.
- **Capability**: shared wallet/application semantic contract.
- **Fresnica Core**: Rust cryptographic/security authority.
- **Port/infrastructure**: platform/network/storage mechanisms.

Read:

1. [`architecture.md`](architecture.md)
2. [`application-flows.md`](application-flows.md)
3. [`application-capabilities.md`](application-capabilities.md)
4. [`core-security-boundary.md`](core-security-boundary.md)
5. [`platform-implementation.md`](platform-implementation.md)

`Service` is an older implementation term. Mobile may use local `Feature` organization, but `Feature` is not the cross-platform Capability name.

## 2. Core / SDK baseline

The Core/SDK foundation is established and should be treated as stable unless a concrete compatibility/security need requires change.

Current Mobile/native baseline:

```text
Native SDK release       native-sdk-v0.2.1
Native Binding API       2
Universal SDK API        3
Core Client API          3
RN adapter source        0.2.1
```

Key validated delivery paths include:

- generalized Native SDK / UniFFI layer;
- Android direct-consumer AAR;
- Apple iOS/macOS `FresnicaSDK.xcframework` + FFI XCFramework;
- independent Swift consumer import/typecheck;
- React Native 0.87 Apple/CocoaPods adapter consumer;
- WASM package/runtime conformance;
- Testnet smart-account/WebAuthn provider fixture and confirmed transfer flow.

`mobile-sdk-v0.1.0` and `native-sdk-v0.1.0` are compatibility/history, not new-project baselines.

See [`sdk/README.md`](sdk/README.md) and [`platforms/mobile/README.md`](platforms/mobile/README.md).

## 3. Security model that must not regress

The authoritative short contract is [`core-security-boundary.md`](core-security-boundary.md).

The critical identity rule remains:

```text
Account identity != Signer capability != Recovery source
```

Consequences include:

- watch-only requires no local secret/mnemonic/passcode for reads;
- attaching secret/mnemonic material verifies derived signer identity before persistence mutation;
- detaching a local signer preserves account identity;
- `C...` contract identity is not an Ed25519 signer public key;
- routine signing and Reveal/Export have different authorization privilege;
- system auth is a platform authorization mechanism, not a replacement wallet cryptographic format;
- ordinary application/JS state must not receive private keys, mnemonics or native unlock keys.

Detailed references are indexed in [`core/README.md`](core/README.md).

## 4. Application Capability status

The authoritative catalog/maturity is [`application-capabilities.md`](application-capabilities.md) and [`capabilities/README.md`](capabilities/README.md). The current catalog contains **19 Capabilities: 9 Normative and 10 Defined**.

### Normative / current strong semantics

- Account;
- Signer;
- Balance / Availability;
- Payment;
- Transaction;
- Trustline;
- SDEX;
- Anchor common Classic path;
- Signing Coordination.

The Rust reference implementation in `clients/rust-client` currently exercises most of these for CLI/TUI.

Important implemented semantics include:

- network-scoped account/wallet state;
- watch-only/local-signer enforcement;
- exact seven-decimal/stroop transaction amounts;
- reserve/liability/fee preflight;
- `CreateAccount` vs `Payment` selection;
- trustline add/limit/remove rules;
- SDEX BUY/SELL direction preservation, exact `price_r`, order-book normalization, trades/fills/candles;
- transaction pending/uncertain-submission guard;
- SEP-1 / Classic SEP-10 / SEP-24 / SEP-6 / status / common SEP-12 Anchor behavior.

### Defined / intentionally not over-standardized

- Wallet aggregate;
- Backup / Restore;
- Asset Discovery / Catalog;
- History / Activity;
- Contacts / Destination Resolution;
- Application Security;
- Dapp Interaction;
- Ledger Authorization;
- Hardware / External Signer Interaction;
- Network / Gateway.

These names/boundaries are shared, but mature implementation behavior should be recorded as Reference Semantics rather than hidden in platform code. `fresnica-mobile` and future Web/Desktop projects may lead contract upgrades through evidence-backed documentation PRs that link to their own implementation/tests; source co-location is not required. Normative Capabilities may likewise accept explicitly non-normative Reference extensions until independent evidence is strong enough for promotion.

## 5. Rust engineering clients

`clients/rust-client` is the Rust Capability reference/reusable implementation. It is not mandatory runtime code for Mobile/Web/Desktop.

### Rust CLI

The CLI is the reference native command client. It consumes shared Capability behavior rather than keeping protocol/business logic in command handlers.

### Rust TUI

The TUI is an engineering/reference UI over the same Rust Capability implementation. Current shared functionality includes:

- wallet/account selection and watch-only state;
- balances/activity;
- reviewed Payment and Trustline writes;
- SDEX offer create/update/cancel;
- open offers;
- pair order book;
- recent trades;
- candles.

Further terminal UI work such as account-fill presentation is optional product work, not an architecture blocker.

See [`platforms/terminal/README.md`](platforms/terminal/README.md).

## 6. Anchor / SEP status

The common Classic Anchor Capability contract includes:

- SEP-1 discovery;
- validated two-phase SEP-10 challenge/sign/exchange;
- SEP-24 preferred transfer initiation;
- SEP-6 fallback;
- transaction status;
- reviewed withdrawal payment handoff;
- common SEP-12 customer status and scalar/binary updates.

The Rust Anchor path keeps protocol/transport semantics in `fresnica-client`; CLI remains product/prompt/rendering orchestration. Rust and RefPython require exact-case `code + issuer` asset identity and reject automatic Anchor redirects. The Rust reference path now uses reusable Ledger Authorization + Signing Coordination for local Ed25519 multisig/delegated SEP-10, models `PreconditionsV2.extraSigners`, and recognizes already-present Ed25519, pre-authorized transaction, Hash-X and signed-payload authorization material. Provider-backed collection for Hash-X, signed-payload and external/hardware signers remains demand-driven; RefPython remains a narrower behavioral reference rather than evidence of general multisig support.

Still demand-driven/deferred:

- SEP-45 contract-account authentication execution;
- uncommon nested SEP-12 values + `/customer/files` file-ID workflow;
- concrete-anchor compatibility fixes discovered in real integration.

See [`capabilities/anchor.md`](capabilities/anchor.md).

## 7. Mobile boundary

The independent `fresnica-mobile` product can proceed without waiting for another shared Rust application layer.

Recommended reading:

```text
docs/README.md
  -> five common contracts
  -> platforms/mobile/README.md
  -> fresnica-mobile Feature-first design
```

Mobile mapping:

```text
Mobile Feature
  -> implements Application Flow(s)
  -> consumes Mobile Capability implementations
  -> uses Stellar JS SDK/gateways/repositories as appropriate
  -> delegates Core-owned security operations to Fresnica Native SDK
```

Mobile is **not required to link `fresnica-client`**. If Mobile discovers stable cross-platform semantics, update the common Capability contract rather than mechanically copying Rust internals.

## 8. Experimental / deferred product areas

Do not implement these merely to close a checklist:

- Dapp transport/session details;
- production hardware/Ledger provider while exact XDR/provider compatibility remains gated;
- SEP-45 execution;
- uncommon SEP-12 nested/file-ID workflow;
- generic Windows/Linux non-Rust SDK packaging without a chosen consumer language/API;
- platform-specific Flow/UI work not required by an active product.

Passkey/smart-account work remains provider/Testnet-specific reference material and must not be confused with the protected Ed25519 software-signer model.

## 9. Validation state

The SEP-12 shared-customer batch at commit `7b6972b` was validated on a real GitHub Rust runner with:

- SDK boundary check;
- rust-client tests;
- CLI tests;
- TUI tests;
- rust-client release build;
- CLI release build;
- TUI release build.

The later Anchor protocol extraction (`2c076d7` in the current development history at the time of this handoff) has passed local/static checks but still requires the same real Rust cargo gate after it reaches GitHub.

Do not upgrade static/rustfmt checks into a claim that Rust compilation passed.

## 10. CI / synchronization

Normal `main` pushes intentionally use the lightweight `Main bundle` workflow rather than every expensive platform validation suite.

The workflow publishes a verified `fresnica-main-bundle` artifact and `main-bundle` commit status. Use that exact bundle as the preferred GitHub -> development baseline.

Real Rust/Apple/Android/Web/platform gates should run at meaningful validation boundaries, not on every documentation or small implementation commit.

## 11. Immediate next work

The shared Core/SDK foundation is complete enough to hand product pressure to the independent `fresnica-mobile` project. Do not keep adding generic shared abstractions merely to close remaining checkboxes.

1. Land the current validated checkpoint and use the resulting `main-bundle` as the new baseline.
2. Let `fresnica-mobile` consume the pinned Native SDK and common Capability contracts while owning Realm, Horizon/network orchestration, platform auth and product UX.
3. Change Core/SDK only for a concrete security/protocol defect or evidence from a real Mobile/Web/Desktop integration.
4. Keep external/hardware signer transport, uncommon SEP extensions and additional platform packages demand-driven until a real consumer exists.
5. Preserve API/version compatibility, conformance fixtures and release gates while the product layer matures.

## 12. Start here next session

1. Verify GitHub `main` and relevant CI/release state.
2. Read [`docs/README.md`](README.md) and the five common contracts.
3. Read [`roadmap.md`](roadmap.md) / [`tasks.md`](tasks.md) only for current project state.
4. If changing a Capability, read its file under [`capabilities/`](capabilities/README.md).
5. If changing signer/passcode/system-auth behavior, read [`core-security-boundary.md`](core-security-boundary.md) plus the relevant Core/platform detail.
6. Preserve the current Core/SDK security authority and avoid source-level cross-platform symmetry for its own sake.
