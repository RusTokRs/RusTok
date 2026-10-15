# Implementation Plan: `rustok-manifest` Foundation Extraction

## Scope

Extract shared manifest DTOs, pure validators, and snapshot hashing into `crates/libs/rustok-manifest`, unifying `modules.toml` and `rustok-module.toml` processing across `apps/server/build.rs`, `xtask`, and the server runtime.

## Current State

- Dedicated foundation crate `crates/libs/rustok-manifest` implemented.
- Pure validators for slugs, UI classifications, and acyclic dependency graphs implemented.
- Deterministic snapshot hashing implemented with roundtrip and invariant unit tests.

## Milestones

- [x] **Milestone 1**: Create `crates/libs/rustok-manifest` with canonical DTOs, parsing, and pure validation rules.
- [x] **Milestone 2**: Author ADR `2026-10-15-rustok-manifest-foundation-library.md` and index it in `DECISIONS/README.md`.
- [x] **Milestone 3**: Register `rustok-manifest` in root `Cargo.toml` workspace dependencies.
- [x] **Milestone 4**: Adopt `rustok-manifest` in `apps/server/build.rs` to replace duplicate internal structs.
- [x] **Milestone 5**: Adopt `rustok-manifest` in `xtask` to replace local types in `manifest_io.rs` and `xtask_types.rs`.
- [x] **Milestone 6**: Run scoped verifications (`cargo check -p rustok-manifest`, `cargo test -p rustok-manifest`, `cargo xtask validate-manifest`, `cargo xtask module validate blog`, `cargo xtask generate-registry`).

## Verification

- `cargo check -p rustok-manifest`
- `cargo test -p rustok-manifest`
- `cargo check -p xtask`
- `cargo xtask validate-manifest`

## Update Rules

Synchronize this plan synchronously whenever the foundation contract, schema version, or consumer integration boundaries change.
