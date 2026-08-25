# SDK compatibility manifest

`manifest.json` is the machine-readable release/adapter compatibility checkpoint for the current Fresnica SDK layers.

It does not replace the version constants owned by Core/SDK/bindings. Instead, it records the compatible set that a release is expected to ship together. `validate.mjs` reads the source-of-truth constants/package versions and fails if the manifest or React Native adapter contract drifts.

Run locally:

```sh
node sdk/compatibility/validate.mjs
```

This check is intentionally lightweight. It does not build Rust, Android, Apple or WASM artifacts; those package-specific validation commands remain authoritative for binary correctness.
