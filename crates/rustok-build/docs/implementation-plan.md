# Implementation plan for `rustok-build`

## Current state

The capability owns build/release persistence models, immutable execution-plan contracts, queued execution, command construction, manifest-snapshot materialization, and the process runner. The server retains only worker event and release-activation adapters; `rustok-cli core rebuild` invokes this capability directly.

`ReleaseActivationHook` is the explicit seam for server-owned post-activation
effects. It prevents OAuth synchronization and platform-state projection from
becoming hidden dependencies of build persistence or CLI execution.

## Verification

- `cargo check -p rustok-build`
- API surface contract guard after server call sites move.
