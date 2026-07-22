# rustok-distribution documentation

## Scope

This support crate is the selected-distribution module registry owner. It has
no UI surface and no FFA/FBA boundary of its own.

`composition_identity()` publishes a canonical hash of the selected module
registry. Trusted CLI and HTTP hosts bind this identity into the installer
topology before preflight and apply, rather than accepting a host-local or
manually entered distribution label. Distributed role deployment is still
pending its `rustok-build` host adapter and durable per-role receipts.

`generate_static_distribution()` accepts only a complete running owner claim
that passes `ModuleStaticDistributionWorkItem::validate()`. It emits three
deterministic build-time outputs: a Cargo `[dependencies]` fragment using generated
aliases and fixed materialization paths, the Rust registry source that registers
the reviewed native entry types, and a canonical JSON manifest. Its output
digest binds every immutable manifest field, output destination, and exact
generated Cargo/Rust byte sequence. The generator never writes the repository or runs
Cargo. A CI executor must apply the files only inside the digest-pinned
materialized platform workspace.

`rustok-static-distribution-worker` is the separate process that stages these
outputs into an immutable job bundle. Its fixed deployment launcher, rather
than this crate or the control plane, owns CAS materialization, Cargo execution,
tests, signing, evidence publication, and the bound terminal receipt.

## Verification

- `cargo check -p rustok-distribution --no-default-features`
- `cargo check -p rustok-server --no-default-features`
- `node scripts/verify/verify-api-surface-contract.mjs`

The current control-plane work permits only lightweight formatting, diff, and
metadata checks; the compile commands above remain the target verification gate.
