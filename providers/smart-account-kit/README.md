# Fresnica Smart Account Kit provider

This is an experimental provider adapter for Stellar `C...` smart accounts authorized by passkeys through the upstream `smart-account-kit` package.

It is **not** part of Fresnica's protected Ed25519 software-signer path:

```text
classic Fresnica software wallet
  G account -> protected S/mnemonic -> WalletUnlockKey -> Ed25519 envelope signature

passkey smart wallet
  C account -> smart-account context rules -> External(WebAuthn verifier, credential) -> Soroban auth
```

No `WalletUnlockKey`, Fresnica protected envelope, mnemonic or Stellar secret is created by this provider.

## Upstream pin

The prototype is pinned to `smart-account-kit` **0.6.2**. The package is pre-1.0, so update the provider deliberately when the upstream API or deployed contracts change. Upstream currently warns that the SDK/demo/relayer integration has **not undergone an independent security audit**; this Fresnica provider is therefore a Testnet interoperability prototype, not a production/mainnet approval.

The checked-in Testnet configuration mirrors the upstream Protocol 27 demo deployment published on 2026-07-09:

- RPC: `https://soroban-testnet.stellar.org`
- smart-account WASM hash: `1b5f4534a76322da2ad7c745f6900857a6802b0ca79850c35a03561df997785a`
- WebAuthn verifier: `CC7EKIHQP3TN4CARQDND6CEOY2UXLWWC2X5GHTD5NLAT7BG5GPZIOM3F`
- native XLM SAC: `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`

The public testnet relayer is a fee payer, not a signer or custodian. Production use must make an explicit relayer/deployer decision. The first provider also rejects overrides of the network passphrase, account WASM hash and verifier identities so an application cannot accidentally combine Mainnet transport/configuration with the reviewed Testnet deployment. Alternate Testnet RPC/relayer/storage/RP settings remain configurable.

## Provider surface

The first provider deliberately stays small:

- silent session restore;
- explicit connect to a known `C...` account/credential;
- passkey authentication followed by account discovery;
- passkey smart-account creation;
- safe `signAndSubmit` delegation;
- Testnet native-XLM `transfer` smoke path;
- disconnect.

There is intentionally no wrapper for upstream `kit.sign()`: current `smart-account-kit` documentation requires re-simulation after WebAuthn signatures because their final size differs from simulation placeholders. Fresnica therefore keeps `signAndSubmit()` as the first safe submission primitive.

Raw WebAuthn registration/authentication responses and passkey public-key material are not returned by the normalized provider lifecycle API. The provider exposes the credential ID only as the external signer reference needed to reconnect/discover smart accounts.

## Local contract tests

These tests need only Node 22 and use an injected fake kit:

```sh
npm test --prefix providers/smart-account-kit
```

To validate the actual installed upstream package on a machine with npm access:

```sh
cd providers/smart-account-kit
npm install
npm run check:installed
bash scripts/validate-local.sh
```

A real Testnet/WebAuthn browser ceremony is the next validation checkpoint; it cannot be honestly replaced with a Node-only unit test.

## Real browser/Testnet smoke

The checked-in smoke page uses the pinned upstream package and the published Protocol 27 Testnet deployment. It creates a real passkey, deploys/funds a smart account through the public Testnet relayer, and can sign + submit a native XLM transfer.

```sh
cd providers/smart-account-kit
npm install
npm run check:installed
npm run testnet:dev
```

Open the localhost URL printed by Vite. `localhost` is a WebAuthn secure context. The smoke page intentionally does not log raw WebAuthn responses or passkey public-key material.

For the transfer checkpoint, send a small Testnet amount to a disposable Testnet `G...` or `C...` recipient and record the returned transaction hash/ledger for the future auth-XDR fixture. Do not treat the public Testnet relayer as a production dependency.

After a confirmed transfer, the smoke page now captures the exact public `func` + signed `auth` XDR that was sent to the Testnet relayer. It immediately verifies the captured authorization before enabling **Download verified auth fixture**. Verification independently:

- decodes the Soroban authorization entry;
- reads the on-chain `AuthPayload.context_rule_ids`;
- recomputes the Protocol 27 signature payload and context-bound auth digest;
- checks that the WebAuthn `clientDataJSON.challenge` equals that digest;
- extracts the passkey P-256 public key from the External signer `keyData`;
- verifies the compact WebAuthn P-256 signature over `authenticatorData || SHA256(clientDataJSON)`.

The captured WebAuthn assertion is public transaction authorization material that is already carried in the signed auth entry; the fixture recorder does not capture authenticator private keys, mnemonic/secret material, Fresnica `WalletUnlockKey`, or unrelated browser requests.

Verify a downloaded fixture again from the command line:

```sh
npm run fixture:verify -- ./fresnica-smart-account-auth-<hash>.json
```

Once a real browser/Testnet run passes both in-page and CLI verification, copy that fixture into `spec/test-vectors/` and use it as the first real smart-account context-rule conformance vector. Do not check in a synthetic vector as a substitute for this checkpoint.
