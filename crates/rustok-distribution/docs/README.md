# rustok-distribution documentation

## Scope

This support crate is the selected-distribution module registry owner. It has
no UI surface and no FFA/FBA boundary of its own.

## Verification

- `cargo check -p rustok-distribution --no-default-features`
- `cargo check -p rustok-server --no-default-features`
- `node scripts/verify/verify-api-surface-contract.mjs`
