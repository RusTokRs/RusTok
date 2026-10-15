# Shared Platform Manifest Foundation Library (`rustok-manifest`)

- Date: 2026-10-15
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-manifest`, platform-architecture, `xtask`
- Extends: [Registry v2 clean contract without runtime compat](./2026-04-19-registry-v2-clean-contract-without-runtime-compat.md)
- Supersedes: None
- Superseded by: None

## Context

Platform module composition and package contracts in RusToK are defined by two declarative manifest layers:
1. Root composition manifest `modules.toml`.
2. Module-scoped contracts `rustok-module.toml`.

Prior to this decision, parsing and data structures for these manifests were duplicated across three separate locations:
- `apps/server/build.rs` (ad-hoc private deserialization structs for server build-time routing and GraphQL codegen);
- `xtask/src/xtask_types.rs` and `xtask/src/manifest_io.rs` (independent models for validation and CLI operations);
- `apps/server/src/modules/manifest/types.rs` (server runtime representation).

This duplication introduced a significant maintenance burden, divergence risk between preflight validation and build-time codegen, and prevented the introduction of clean, shared build-time manifest verification. Placing this logic in `rustok-core` was rejected because `rustok-core` carries heavy asynchronous, database (`sea-orm`), and network dependencies that would bloat build scripts (`build.rs`) and CLI tools (`xtask`).

## Decision

1. **Extract `crates/libs/rustok-manifest` as a canonical platform foundation library**:
   - `rustok-manifest` is a zero-runtime-dependency foundation crate (relying only on `serde`, `toml`, `semver`, and `thiserror`).
   - It owns the canonical typed AST, parsing, canonical snapshot hashing, and pure contract validation for `modules.toml` and `rustok-module.toml`.
2. **Unified Consumers**:
   - `apps/server/build.rs` consumes `rustok-manifest` as a `[build-dependency]` for codegen.
   - `xtask` consumes `rustok-manifest` for all validation, registry generation, and module publishing workflows.
   - `apps/server` runtime consumes `rustok-manifest` for runtime manifest parsing and baseline composition validation.
3. **Pure Validation Rules**:
   - Validation of manifest schema, module slugs, UI classifications, and dependency graph invariants (`depends_on` topological consistency and acyclicity) are implemented as pure Rust functions in `rustok-manifest::validation`.

## Sources of truth and ownership

- Canonical manifest file definitions: `modules.toml` (root) and `crates/modules/*/rustok-module.toml`.
- Manifest parsing, data types, and pure validation: `crates/libs/rustok-manifest`.
- Host compilation wiring and codegen: `apps/server/build.rs`.
- Preflight CLI verification and generation: `xtask`.

## Invariants

- `rustok-manifest` MUST NOT depend on database ORMs (`sea-orm`), web frameworks (`axum`), or async runtimes (`tokio`).
- Manifest models MUST parse valid `modules.toml` (schema version 2) and `rustok-module.toml` deterministically using `BTreeMap` for sorted key ordering.
- Dependency graphs declared in `depends_on` must be validated for acyclicity and closure.

## Non-goals

- Dynamic runtime module loading via dlopen / WASM (out of scope for this foundation model).
- Replacing `Cargo.toml` compilation dependency resolution.

## Data, transaction, and concurrency boundary

Not applicable. `rustok-manifest` is a pure in-memory data modeling, serialization, and validation library with no database, transactional, or concurrency side-effects.

## Context dimensions

Not applicable. Manifest files define build-time and deployment-level static platform contracts independent of request-time tenant, locale, or authorization context.

## Events and projections

Not applicable. Manifest definitions do not emit domain events or maintain persisted read-model projections.

## Failure semantics

- Parsing or schema validation failures in `rustok-manifest` fail fast with typed `ManifestError`.
- Build-time codegen fails closed if manifest syntax or invariants are violated.

## Migration and cutover

1. Introduce `crates/libs/rustok-manifest` with complete DTOs, parsing, and pure validation functions.
2. Wire `rustok-manifest` into root `Cargo.toml` workspace dependencies.
3. Replace ad-hoc types in `apps/server/build.rs` with `rustok-manifest`.
4. Replace duplicate types and IO routines in `xtask` with `rustok-manifest`.
5. Existing `modules.toml` and `rustok-module.toml` file formats remain unchanged (100% backward compatible).

## Alternatives considered

- **Keep parsing duplicated**: Rejected due to high risk of drift between `xtask module validate` and `build.rs` codegen.
- **Place manifest DTOs into `rustok-core`**: Rejected because `rustok-core` includes `sea-orm`, `tokio`, `reqwest`, etc., which would catastrophically increase `build.rs` compilation time and pull database dependencies into `xtask`.
- **Place manifest DTOs into `rustok-api`**: Rejected because `rustok-api` is intended for HTTP/GraphQL contracts and includes web framework dependencies.

## Verification

- `cargo check -p rustok-manifest` and `cargo test -p rustok-manifest` verifying round-trip TOML parsing and validation rules.
- `cargo check -p xtask` and `cargo xtask validate-manifest` running against existing `modules.toml`.
- Successful server build script execution via `cargo check -p rustok-server` (or build.rs invocation).

## Consequences

- Eliminates redundant manifest structs across `apps/server/build.rs`, `xtask`, and server runtime.
- Keeps build scripts and `xtask` lightweight and fast to compile.
- Provides a clean basis for future automated module scaffolding and code generators.
