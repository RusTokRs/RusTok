# rustok-distribution documentation

## Scope

This support crate is the selected-distribution module registry owner. It has
no UI surface and no FFA/FBA boundary of its own.

`composition_identity()` publishes a canonical hash of the selected module
registry. Trusted CLI and HTTP hosts bind this identity into the installer
topology before preflight and apply, rather than accepting a host-local or
manually entered distribution label. Distributed role deployment is still
pending its `rustok-build` host adapter and durable per-role receipts.

## Verification

- `cargo check -p rustok-distribution --no-default-features`
- `cargo check -p rustok-server --no-default-features`
- `node scripts/verify/verify-api-surface-contract.mjs`
