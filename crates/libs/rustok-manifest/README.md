# `rustok-manifest`

Canonical manifest models, pure validation, and dependency graph invariants for the RusToK platform and module contracts.

## Purpose

`rustok-manifest` is the foundational, zero-runtime-dependency library responsible for parsing, deserializing, validating, and hashing RusToK manifest files:
- The platform composition manifest: `modules.toml`
- The scoped module package contract: `rustok-module.toml`

It provides one canonical source of truth for both build-time tooling (`apps/server/build.rs`), developer CLI tools (`xtask`), and server runtime verification.

## Responsibilities

- Provide canonical typed AST models for `modules.toml` (`ModulesManifest`, `ModuleSpec`, `BuildConfig`) and `rustok-module.toml` (`ModulePackageManifest`, `ModulePackageMetadata`, `ModulePackageProvides`).
- Implement framework-independent, pure validation rules for module slugs, UI classifications, ownership, trust levels, and dependency acyclicity.
- Calculate deterministic SHA-256 snapshot hashes of platform module configurations.
- Eliminate duplicated TOML parsing logic and divergent struct definitions across the repository.

## Entry points

- `load_modules_manifest` / `parse_modules_manifest`: Read and parse `modules.toml`.
- `load_module_package_manifest` / `parse_module_package_manifest`: Read and parse `rustok-module.toml`.
- `validate_modules_manifest`: Ensure platform composition invariants hold.
- `validate_module_package_manifest`: Ensure scoped module contracts hold.
- `compute_modules_manifest_hash`: Generate deterministic state hash.

## Interactions

- **`apps/server/build.rs`**: Consumes `rustok-manifest` as a fast build-dependency to generate Axum routes and GraphQL schemas.
- **`xtask`**: Consumes `rustok-manifest` for `cargo xtask validate-manifest`, `cargo xtask module validate`, and `cargo xtask generate-registry`.
- **`apps/server`**: Verifies running composition against the immutable manifest contract.

## Docs

- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module docs](./docs/README.md)
- [Platform docs index](../../../docs/index.md)
