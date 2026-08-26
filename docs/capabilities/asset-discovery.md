# Asset Discovery / Catalog Capability

Maturity: **Defined**

## Purpose

Asset Discovery / Catalog is the shared capability name for finding candidate Stellar assets for product Flows such as Manage Assets/Trustlines and SDEX market selection.

It does not define which assets a product recommends, promotes or ranks first.

## Agreed boundary

An implementation must preserve exact asset identity:

```text
XLM
or
CODE:GISSUER
```

A displayed asset code or brand name is not sufficient identity.

The capability may provide optional descriptive metadata such as domain, display name, organization or source, but metadata must not replace exact asset identity.

A user/product Flow must retain a path to use an exact manually supplied asset identity even when discovery providers are unavailable or incomplete.

## Reference Semantics: RefPython

The strongest current implementation evidence is:

- [`reference/python/fresnica/asset_catalog.py`](../../reference/python/fresnica/asset_catalog.py)
- [`reference/python/tests/test_asset_catalog.py`](../../reference/python/tests/test_asset_catalog.py)
- [`reference/python/fresnica/market_discovery.py`](../../reference/python/fresnica/market_discovery.py)
- [`reference/python/tests/test_market_discovery.py`](../../reference/python/tests/test_market_discovery.py)

Useful candidate semantics from that implementation are:

### Cache-first availability

Discovery uses a local cache as a usable fallback when remote ranking/catalog sources fail. A transient discovery-provider failure therefore need not prevent a user from selecting already-known assets or entering an exact identity manually.

### Multiple sources may contribute metadata

The reference can consume ranked and curated public lists, de-duplicate by exact asset identity and merge useful metadata. Provider/source identity remains diagnostic/catalog metadata rather than wallet asset identity.

### Network-aware discovery

The reference only offers its public recommendation feed for mainnet. Other networks remain usable through exact manual asset identity.

The general candidate semantic is that catalog availability/recommendation data is network-scoped; the specific mainnet-only provider policy is not universal.

### Discovery and product recommendation are separate

The catalog supplies candidate assets and metadata. Market-pair ordering, held-asset preference, featured assets and other recommendation policy belong to the consuming Flow/product layer unless later promoted explicitly.

## Implementation-specific choices today

The following RefPython choices are not shared contract requirements:

- StellarExpert as the ranked provider;
- SEP-42/public asset-list sources or their current ordering;
- current cache file schema;
- current fetch limits/timeouts;
- current curated-source merge precedence;
- recommendation ranking;
- special treatment of any branded asset such as USDC;
- one exact asset-entry DTO.

Mobile/Web/Desktop may use other providers, local indexes or product-curated data while preserving exact identity and failure/fallback semantics.

## Promotion criteria

Promote narrower Asset Discovery semantics when independent implementations converge on a stable request/result model for:

- network scope;
- exact identity plus optional metadata;
- cache/fallback meaning;
- provider/source provenance where needed;
- separation of discovery from recommendation/product ranking.

A platform implementation may contribute evidence through a documentation PR without moving its source code into this repository.
