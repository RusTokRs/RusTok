# rustok-distribution documentation

## Scope

This support crate is the selected-distribution module registry owner. It has
no UI surface and no FFA/FBA boundary of its own.

`composition_identity()` publishes a canonical hash of the selected module
registry. The installer topology and receipt contract consume this identity in
the next topology-descriptor slice rather than a host-local or manually entered
distribution label.

## Verification

- `cargo check -p rustok-distribution --no-default-features`
- `cargo check -p rustok-server --no-default-features`
- `node scripts/verify/verify-api-surface-contract.mjs`
