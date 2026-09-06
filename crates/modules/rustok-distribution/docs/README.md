# rustok-distribution documentation

## Scope

This support crate is the selected-distribution module registry owner. It has
no UI surface and no FFA/FBA boundary of its own.

`build_runtime_extensions(...)` combines module-owned contributions with
explicitly selected cross-module bridges. The `ai-translation` feature is a
composition feature, not a module slug: it requires `mod-ai` and
`mod-translation` and publishes a Translation-owned lazy factory without
placing AI/Translation imports in the server host. The production server
profile selects this bridge. Composition evidence verifies the fail-closed
deployment state: without the result keyring, the factory resolves to no
machine provider and manual Translation workflows remain available.

`composition_identity()` publishes a canonical hash of the selected module
registry. Trusted CLI and HTTP hosts bind this identity into the installer
topology before preflight and apply, rather than accepting a host-local or
manually entered distribution label. Distributed role deployment is still
pending the trusted owner-admission resolver and desired/observed rollout
adapter. Installer apply uses one exact bundle request and one receipt with
per-role observations; it never creates per-role release heads through
`rustok-build`.

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
- `cargo test -p rustok-distribution --no-default-features --features ai-translation selected_ai_translation_bridge_publishes_factory_and_stays_optional_without_keyring`
- `node scripts/verify/verify-api-surface-contract.mjs`
- `node scripts/verify/verify-ai-translation-boundary.mjs`

The current control-plane work permits only lightweight formatting, diff, and
metadata checks; the compile commands above remain the target verification gate.
