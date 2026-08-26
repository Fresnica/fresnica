# SDEX Testnet live probe

The SDEX live probe is intentionally **not** part of normal pull-request CI.
Friendbot and public Horizon availability are external dependencies and must not
make ordinary Fresnica development flaky.

Run it locally from the Python reference directory:

```bash
cd reference/python
FRESNICA_LIVE_TESTNET=1 uv run pytest -q -s tests/test_sdex_testnet_live.py
```

Or run the GitHub Actions workflow **SDEX Testnet live probe** manually.

## What it creates

Each run generates three disposable random Testnet accounts in memory:

- issuer
- maker
- taker

The probe funds them with Friendbot and creates a `FRES` asset whose issuer is
the random issuer account. The asset identity is therefore unique to the run;
existing public Testnet markets cannot accidentally cross these offers.

Secret keys are not printed or persisted.

## What it verifies

The probe exercises the real Python reference Capability implementations and Stellar SDK against
Testnet:

1. Explicit receiving-trustline approval builds and submits
   `ChangeTrust + ManageSellOffer` in one transaction.
2. The temporary setup offer can be read as an `OpenOffer` and cancelled.
3. The issuer sends test asset units to the maker.
4. The maker creates a resting `ManageSellOffer` in `FRES/XLM`.
5. The taker submits two separate crossing `ManageBuyOffer` transactions.
6. The maker's remaining canonical offer projects to the expected SELL amount
   and COUNTER/BASE price.
7. `/trades?for_account` returns the two fills and Fresnica compresses them into
   one consecutive offer-level segment using the maker offer id and exact
   `price_r`.
8. The partially filled SELL offer is cancelled.
9. The taker creates a non-crossing resting BUY, Fresnica reads it back through
   pair-relative `OfferView`, updates it with `ManageBuyOffer`, then cancels it.
10. The taker finishes with no open offers for the unique market.

## Expected result

A successful run ends with one passing test and no persistent local files:

```text
1 passed
```

If Testnet infrastructure is temporarily unavailable, rerun the manual probe.
Do not weaken the normal unit/integration suite to accommodate public-network
flakiness.
