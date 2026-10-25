# rustok-manifest / CRATE_API

## Role

`rustok-manifest` is the canonical platform contract library for manifest definitions in RusToK.
It provides strongly-typed models, pure schema validation, and topological dependency resolution
for root composition (`modules.toml`) and sovereign module packages (`rustok-module.toml`).

It is a **zero-runtime foundation library** containing no database or transport dependencies.

## Public modules

`error`, `types`, `validation`.

## Primary public types and functions

### Types
- `ModulesManifest` (alias `Manifest`) — root platform configuration schema (`modules.toml`).
- `ModuleSpec` — declaration of a single module dependency and runtime configuration.
- `BuildConfig`, `ServerBuildConfig`, `AdminBuildConfig`, `StorefrontBuildConfig` — compilation targets.
- `ModulePackageManifest` — sovereign module package contract schema (`rustok-module.toml`).
- `ModulePackageMetadata` — module identity, ownership, and trust levels.
- `ModulePackageProvides` — capability provisions (GraphQL, HTTP, CLI, Admin UI, Storefront UI).

### Functions
- `parse_modules_manifest`, `load_modules_manifest` — deserialize `modules.toml`.
- `parse_module_package_manifest`, `load_module_package_manifest` — deserialize `rustok-module.toml`.
- `validate_modules_manifest` — validate root composition integrity and cycle-free topological order.
- `validate_module_package_manifest` — validate package specification compliance.
- `is_valid_module_slug`, `is_valid_ui_classification`, `is_valid_ownership`, `is_valid_trust_level` — primitive validators.
- `compute_modules_manifest_hash` (feature `snapshot-hash`) — deterministic SHA-256 hash of resolved configuration.

## Contract invariants

1. **Schema Versioning**: `modules.toml` strictly requires `schema = 2`.
2. **Canonical Slugs**: Module slugs must be 1 to 48 characters long, consisting strictly of lowercase
   ASCII letters, numbers, and underscores, without leading or trailing underscores.
3. **Acyclic Dependency Graph**: Inter-module dependencies (`depends_on`) must form a directed acyclic
   graph (DAG) verified via Kahn's algorithm; cycles are rejected with `ManifestError::CyclicDependency`.
4. **Required vs Default-Enabled**: Modules declared with `required = true` cannot be listed in `default_enabled`.
5. **Deterministic Hashing**: When serialized for snapshot verification, manifests produce identical SHA-256 digests.

## Errors

- `ManifestError::TomlParse`: Invalid TOML syntax or structural mismatch.
- `ManifestError::UnsupportedSchema`: Schema version != 2.
- `ManifestError::InvalidSlug`: Slug violating lowercase identifier constraints.
- `ManifestError::MissingDependency`: Declared dependency not present in the manifest.
- `ManifestError::CyclicDependency`: Cycle detected in the dependency graph.
- `ManifestError::InvalidOwnership` / `InvalidTrustLevel`: Unsupported contract enum values.
