# Fresnica Project Handoff

Updated: 2026-08-24

This document is the compact handoff for continuing Fresnica development. It summarizes the current product model, architecture boundaries, recent decisions, invariants that should not regress, and the remaining larger work.

## Repository State

- Repository: `manran/fresnica`
- Default branch: `main`
- Current main at this handoff: `28195df6e276ddbb3160f56a661a2ab75099006c`
- Latest merged PR: #40, **Adopt retained history cache model**
- Open PRs at handoff: none
- Latest full Python reference CI: **235 passed, 3 skipped**

The Python implementation is still the behavior/reference implementation. Future Rust Core work should port stable wallet/runtime semantics rather than inventing a second product model.

## Development Model

Fresnica should remain deliberately layered:

```text
CLI / TUI
    |
Wallet-oriented services and workflows
    |
Stellar adapter / stellar-sdk
    |
Stellar network / Horizon / anchor protocols
```

Important boundary rules:

- Prefer official `stellar-sdk` primitives for protocol behavior instead of reimplementing Stellar encoding, signing, transaction, memo, or stream semantics.
- UI code should orchestrate services and present state; protocol selection and response interpretation should live below Textual.
- Wallet identity/secrets, user preferences, and chain-derived caches remain separate storage concerns.
- All chain-derived cache identities must remain scoped by Stellar network.
- Full asset identity is authoritative: classic issued assets are `CODE:GISSUER`, never code-only.
- Changes should be validated with the locked uv environment: `uv sync --locked`, then `uv run pytest -q`.

See `architecture.md`, `storage.md`, `services.md`, and `decision-log.md` for the durable architecture record.

## Wallet / Runtime

Implemented and stable in the Python reference:

- mnemonic and secret-key wallets
- watch-only wallets
- encrypted signing material
- wallet create/import/list/use/delete and backup/export workflows
- lock/unlock lifecycle
- software signer and verified external Ed25519 signer abstraction
- per-network runtime service composition
- SQLite chain-data cache
- persistent UI settings
- contacts/address book

Watch-only wallets may inspect balances, activity, offers, and market data but must never enter signing flows.

Hardware transport adapters remain unimplemented.

## History Model

PR #40 replaced the short-lived bounded catch-up design from PR #39.

Default behavior:

- retain the newest **2,000 Horizon operations per account and network**
- when the local cache is empty, start at the current Horizon head and page **backwards** until 2,000 operations are cached or Horizon is exhausted
- once a cache exists, synchronize only **forward** from the newest local paging token to the current Horizon head
- there is **no fixed incremental page-count cap**
- as newer operations arrive, trim the oldest cached rows so normal local storage remains bounded

`Keep full history locally` is a persisted boolean opt-in:

- disables trimming
- still synchronizes new operations forward from the newest local cursor
- also backfills older operations still exposed by the connected Horizon instance
- cannot reconstruct records already pruned upstream
- disabling the option returns the cache to the newest 2,000 operations on the next synchronization

History UI behavior:

- `M Older` reveals more already-cached activities; it does not independently fetch an older Horizon page
- `F Full history` toggles the full-history preference and synchronizes
- `/` filters the locally loaded History view
- suspicious claimable activity is visible by default and may be hidden explicitly
- UTC/local time is a persisted presentation preference

Do **not** reintroduce `SyncResult(caught_up)` or the former `5 x 200` History catch-up product model.

Authoritative detail: `history-cache.md` and `decision-log.md`.

## SDEX / DEX

The current DEX is a wallet-oriented SDEX terminal, not a separate exchange engine.

Stable behavior:

- market identity uses full asset identities and is scoped by network + wallet
- Starred and Recent markets persist locally
- Popular discovery reuses the ranked StellarExpert asset catalogue
- held-asset pair ordering follows the Fex-derived ranking model
- the pair picker defaults focus directly to the market table
- `F` switches to the Favorites/Starred list; `Space` toggles the current pair star
- the live pair screen supports `W` pair swap and guards stale REST/SSE results by pair/revision
- current pair REST data remains the complete fallback; SSE augments realtime book/trade updates
- order book presentation is:

```text
Amount | BID Price || ASK Price | Amount
```

- BID/BUY is the left book, ASK/SELL is the right book
- BID amount normalization to BASE uses exact Horizon `price_r`; do not replace it with code-only or inverse-price shortcuts
- book rendering uses non-focusable Rich grids
- `BID · BUY` and `ASK · SELL` use full-width tinted section strips
- live pair screen defaults focus to `Your open offers`
- price presentation uses fixed Stellar 7-decimal semantics; sub-display nonzero values must not render as false zero
- immediately created and fully filled synthetic offers are presented as `Immediate` while retaining their raw synthetic IDs
- timezone toggling is local presentation state and must not trigger a network refresh

Account synchronization work from PR #38 also remains in force:

- account offers are fully paged before replacing the local snapshot
- account fill synchronization may be bounded internally, but partial state is explicit in the DEX UI (`fill sync partial · R continue`)

## Assets / Trustlines

User-facing terminology is **Manage Assets**. Protocol/domain naming such as ChangeTrust and trustline limits remains valid internally.

Normal TUI behavior:

- Add and Remove are exposed
- Set limit is intentionally hidden from the normal Manage Assets shortcut surface, while the underlying capability remains available internally/through lower-level flows
- asset selection uses the shared Asset Picker
- `/` live filtering is shared across Asset Picker, market selection, contacts, Manage Assets, and History

Curated asset discovery for the asset picker uses the same three external sources as Fex:

- Lobstr
- Soroswap
- StellarExpert

This is a separate curated cache from DEX Popular ranking. Do not merge the two concepts: DEX Popular should retain ranked-market ordering semantics.

Classic assets are deduplicated by full `CODE:GISSUER` identity. Soroban contract-only entries are not trustline candidates.

### Fresnica trustline marker

New Fresnica-created trustlines use the visible default limit:

`708269837873.6765`

This is an intentional Fresnica marker chosen for later ecosystem measurement. It is below the Stellar maximum and should remain visible in explicit creation/review flows. DEX automatic receiving-trustline creation also uses it.

Do not forcibly rewrite existing trustline limits during edit/remove flows, and do not change this marker without an explicit product decision.

## Anchor Transfers

Anchor work is split into two layers:

- `AnchorService`: SEP transport/protocol operations such as SEP-1, SEP-10, SEP-6, SEP-24
- `AnchorTransferService`: wallet-facing protocol selection, field planning, response interpretation, and transfer workflow projection

Current behavior:

- actionable SEP-24 deposit/withdraw is supported when capability metadata is complete
- SEP-6 deposit/withdraw is also supported
- usable SEP-24 is preferred; SEP-6 is the fallback when SEP-24 is unavailable
- SEP-10 authentication uses `stellar-sdk` verification/signing semantics
- Stellar memo types text/id/hash/return-hash are supported through the reviewed transfer pipeline
- anchor capability cache is scoped by network + exact asset + normalized home domain
- fchain.io XRP's unstructured SEP-6 response forms are handled explicitly
- SEP-6 KYC-required responses are detected and surfaced, but a full generic SEP-12 KYC collection workflow is **not** implemented yet

Full SEP-12 remains a legitimate future feature; do not silently treat KYC-required transfers as ready.

## Activity / Contacts UX

Current History/activity presentation includes:

- transaction-level grouping derived from cached raw Horizon operations
- contact labels as `Name · short-address`
- cached issuer home-domain labels when available
- explicit contract-call and asset-balance-change summaries
- classic clawback summaries
- narrow suspicious-claimable classification rather than broad dust filtering
- wrapping, scrollable activity details

Presentation should remain derived from raw cached operations so newly added contacts and metadata can redraw existing History without rebuilding the underlying cache.

## Recent Merge Sequence

The recent coherent UX/architecture run is:

- **#29** Improve DEX market discovery and realtime UX
- **#30** Refine activity presentation and suspicious claimables
- **#31** Make asset details and anchor workflows actionable
- **#32** Align DEX TUI with Fex market UX
- **#33** Polish DEX market presentation
- **#34** Refine contacts, anchor cache, history and DEX UX
- **#35** Improve DEX market focus and Fresnica trustline defaults
- **#36** Refine DEX focus and asset management
- **#37** Add SEP-6 transfers and list search
- **#38** Complete SDEX account synchronization
- **#39** Separate anchor workflow and bound history catch-up
- **#40** Adopt retained history cache model

Only the History catch-up portion of #39 was superseded by #40. The AnchorTransferService boundary and network-scoped anchor capability cache introduced in #39 remain current.

## Validation / Working Practice

Before writing:

1. Re-read current `main`; do not assume this handoff SHA is still current.
2. Prefer a coherent batch over isolated cosmetic commits when changes share one product decision.
3. Keep protocol behavior in services/adapters and presentation behavior in CLI/TUI modules.
4. Reuse existing models/helpers rather than creating parallel concepts.
5. Run the **full** Python reference suite with the locked uv environment.
6. Inspect the final diff before merge; temporary patch scripts/workflows used for isolated CI must not remain in the final PR.

The repository has previously used temporary GitHub Actions patch workflows when the local execution environment could not reach GitHub. Those helpers are implementation tooling only and must be removed before the final merge.

## Remaining Larger Work

Known larger items rather than immediate regressions:

- hardware transport adapters
- full generic SEP-12/KYC collection if Fresnica is to own that anchor workflow
- Rust Fresnica Core
- mobile bindings
- desktop application
- stable SDK API

The Python reference should continue to define behavior and test vectors until those semantics are stable enough to port.

## Start Here Next Session

1. Verify `main` HEAD and CI status.
2. Read `decision-log.md` for decisions that supersede older implementations.
3. Read `history-cache.md` before touching History synchronization.
4. Preserve the DEX, asset identity, trustline marker, and anchor boundaries above unless the product decision itself is intentionally being changed.
5. Check `tasks.md` and `roadmap.md` for the broad remaining phase work.
