# Backend Module Verification Guide

This guide lists verification paths for backend module work. Prefer the smallest check that
covers the changed boundary, but do not skip guardrails that protect the architecture.

## Fast Static Guardrails

Run these for Loco/FBA/API surface work:

```bash
node scripts/verify/verify-api-surface-contract.mjs
node scripts/verify/verify-loco-inventory.mjs
git diff --check
```

Use `rg` to confirm source-level removals:

```bash
rg -n "loco_rs::controller::format|format::json" apps/server/src/controllers crates -g "*.rs"
rg -n "loco_rs::app::AppContext" crates/rustok-*/ apps/server/src -g "*.rs"
```

If a file was intentionally removed or split, delete stale file-level checks instead of
keeping guards for paths that no longer exist. Keep package-level dependency checks when the
crate must still stay Loco-free.

## Module Validation

For module manifest or ownership changes:

```bash
cargo xtask module validate <slug>
```

For FFA/FBA status changes, update both:

- module-local `docs/implementation-plan.md`;
- central readiness board in `docs/modules/registry.md`.

For backend layout changes, also inspect the physical placement:

```bash
rg -n "clap|std::process::exit|println!|eprintln!" crates/rustok-MODULE/src -g "*.rs"
rg -n "apps::server|loco_rs::app::AppContext|loco_rs::controller::format" crates/rustok-MODULE -g "*.rs"
```

Expected result:

- domain/application code is in `src/`;
- evidence artifacts are in `contracts/`;
- local FFA/FBA status is in `docs/implementation-plan.md`;
- CLI command adapters, when present, are in module-local `cli/`;
- `apps/server` only mounts and composes owner-owned entrypoints.

## Targeted Rust Checks

Use targeted checks when code changed:

```bash
cargo check -p <module-crate>
cargo test -p <module-crate> --lib
cargo check -p rustok-server --no-default-features
```

For heavy server profiles, prefer `--no-default-features` first. Full server test profiles may
compile unrelated UI/admin crates; run them only when the changed boundary requires that
evidence.

## Transport Contract Checks

For GraphQL/REST/`#[server]` changes:

```bash
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
```

When only a static boundary changed, document why reference artifacts were not regenerated.

## FBA Evidence

FBA slices need evidence appropriate to the status:

- source-level descriptors and registry JSON for planned/in-progress;
- static matrix and fallback smoke for boundary-ready candidates;
- compiled runtime smoke before transport-verified or parity-verified promotion.

Use module-local verifier scripts when they exist, for example:

```bash
npm run verify:workflow:fba
npm run verify:foundation:fba-runtime-smoke
```

Do not promote FBA readiness only because a type exists. Readiness requires consumer/provider
metadata, error mapping, fallback policy and verification evidence.

## Documentation Checks

Before completing a backend documentation or contract change:

```bash
rg -n "rustok-web|rustok-runtime|rustok-fba|rustok-cli-core|loco_rs|cargo loco" docs apps crates -g "*.md"
```

Confirm that active docs point to the target architecture and old Loco notes are marked as
deprecated or historical inventory.
