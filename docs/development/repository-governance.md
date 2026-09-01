# Repository Governance and CI

`main` should require pull requests, block force-push/deletion, require an up-to-date branch and resolved conversations, and require the stable check `Required CI / validate`.

## Validation layers

Fresnica deliberately separates fast development feedback from expensive cross-platform integration.

### Local pre-push

Before pushing Rust changes:

1. run the repository-provided portable rustfmt tool in `write` mode on changed Rust files;
2. run the same tool in `check` mode;
3. inspect `git diff --check` and the semantic diff;
4. run any locally available focused tests.

GitHub CI remains a backstop, not the first formatter.

### Required CI

`.github/workflows/ci-required.yml` owns the stable merge gate. It always validates SDK compatibility metadata and the SDK-boundary guard, checks only changed Rust files for formatting, then tests affected Rust surfaces and their immediate Core/SDK/Process dependencies. It does not build release binaries or run every downstream product/toolchain on every push.

The stable check name remains:

```text
Required CI / validate
```

### Integration CI

Expensive downstream workflows remain path-scoped and run for non-draft pull requests (or explicit `workflow_dispatch`). These include Native/Apple/Android packaging, WASM packaging, RefPython Process Binding integration, Rust CLI/TUI release compatibility and other consumer-toolchain gates.

Recommended development sequence:

```text
local edit
  -> portable rustfmt
  -> Draft PR / focused pushes
  -> Required CI
  -> mark Ready once the slice is complete
  -> one full path-relevant integration matrix
  -> squash merge
```

If a final integration failure requires another push, the relevant integration workflows run again. This is intentional; repeated full matrices during ordinary draft iteration are not.

`Main bundle` remains a post-merge synchronization artifact, not a correctness gate. Temporary probe/relay PRs should be closed and their branches deleted once conclusions are absorbed into code/tests/docs.
