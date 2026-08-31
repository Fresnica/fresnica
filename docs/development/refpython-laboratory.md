# RefPython Laboratory Governance

RefPython is Fresnica's executable laboratory for product semantics, Application Flows and candidate Capability behavior. It is not a second security authority and it is not automatically a cross-platform specification.

## Normal path

```text
RefPython experiment -> Candidate evidence -> Capability / ADR / language-neutral vectors -> production implementations
```

RefPython normally leads uncertain Account/Signer/Recovery Source lifecycle, Payment/Trustline/SDEX product semantics, Anchor orchestration, transaction recovery UX, History/Activity grouping, contacts and UI state machines.

Use maturity states: **Experimental** (RefPython only, no compatibility promise), **Candidate** (tested invariant can be stated independently of Python), **Normative** (promoted to shared contract/vector), **Implemented** (production implementations conform), and **Retired**.

## Security boundary

RefPython does not originate encryption/KDF/envelope formats, WalletUnlockKey privilege, zeroization requirements, Reveal/Export declassification, Core identity verification, Stellar XDR/hash/signature rules, official protocol security fixes, Native/UniFFI/WASM/Process ABI, OS authentication, or network endpoint security policy. Those begin with official protocol evidence, threat analysis and the owning Core/SDK/platform contract.

When available, RefPython delegates Core-owned operations through `Fresnica Process Binding -> Fresnica SDK -> Rust Core`; it must not add new wallet cryptography merely to keep an experiment self-contained. Retained Python crypto is only independent conformance evidence, migration archaeology, or fallback reference behavior.

Before promotion, record the problem, success/failure scenarios, invariant without Python names, identity/network effects, security/privacy effects, product-specific exclusions, language-neutral fixtures where useful, and production acceptance criteria.

Urgent protocol/security fixes, Core/SDK packaging, official Stellar API migrations and platform-only behavior may bypass RefPython-first. The target steady state for product semantics remains RefPython-first.
