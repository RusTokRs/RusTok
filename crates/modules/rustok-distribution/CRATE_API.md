# Crate API: rustok-distribution

## Public Surface

### Primary Public Functions

- `build_registry`: Assembles compile-time selected module implementations into `ModuleRegistry`.
- `build_runtime_extensions`: Seeds runtime extensions from active modules and registers selected cross-module bridges.
- `composition_identity`: Computes stable deterministic fingerprint (`hash`, `revision`, `modules`) of active distribution.
- `generate_static_distribution`: Generates static distribution manifest and source files for reviewed promotions.

### Public Constants & Types

- `GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH`: Static cargo manifest path constant (`crates/modules/rustok-distribution/Cargo.toml`).
- `GENERATED_DISTRIBUTION_MANIFEST_PATH`: Static distribution manifest path constant (`.rustok/generated/static-distribution.json`).
- `GENERATED_DISTRIBUTION_REGISTRY_PATH`: Static registry path constant (`crates/modules/rustok-distribution/src/generated_promotions.rs`).
- `CompositionIdentity`: Immutable identity of compiled modules (`revision`, `hash`, `modules`).
- `CompositionModule`: Canonical module metadata in composition (`slug`, `version`, `kind`, `dependencies`).
- `CompositionModuleKind`: Classification of compiled module (`Core`, `Optional`).
- `GeneratedStaticDistributionFiles`: Manifest file collection DTO (`manifest`, `manifest_json`, `cargo_dependencies_toml`, `registry_source`).
- `GeneratedStaticDistributionManifest`: Static distribution metadata representation.
- `GeneratedStaticDistributionSource`: Generated Rust code container and build metadata.
- `StaticDistributionGenerationError`: Errors arising during static distribution code generation (`InvalidWorkItem`, `Serialization`).

## Platform Capability Bindings

- Static compile-time composition root for RusToK host executables.
- Bridges domain modules with index schemas and runtime extensions.

## Domain Collaborations

- Registers all active platform domain modules into the core registry.
- Bridges product indices when `mod-product` feature is enabled.
- Bridges inventory stock index when `mod-inventory` feature is enabled.
- Bridges AI translation factories when `ai-translation` feature is enabled.

## Minimum Contract Set

### Input DTOs/Commands

- `build_registry`: Assembles and returns configured `ModuleRegistry`.
- `build_runtime_extensions`: Seeds and materializes runtime extensions from active module registry.
- `composition_identity`: Calculates deterministic sha256 fingerprint from registered module metadata.
- `generate_static_distribution`: Takes validated `ModuleStaticDistributionWorkItem` and generates distribution artifacts.

### Domain Invariants

- Composition identity hash is deterministic, 64-character lowercase hex string, and uniquely identifies active module set.
- All registered modules maintain proper dependency order and invariant initialization.
- Dynamic extension collisions are rejected at registration time.
- All public models and errors implement `Debug`, `Clone`, and `PartialEq`.

### Events / Outbox Side Effects

- Assembles event consumers and schema providers into shared runtime context.
- Read and composition operations are side-effect free.

### Errors / Failure Codes

- `StaticDistributionGenerationError::InvalidWorkItem`: Work item payload or build configuration is invalid.
- `StaticDistributionGenerationError::Serialization`: JSON serialization error during artifact generation.
- `rustok_core::Error::Validation`: Registration collision when identical port factory is already registered.

## Inventory stock Index bridge

`mod-inventory` selects an Inventory-owned nonlocalized stock schema and a native-reader source
only when `InventoryModule` actually contributed its runtime selection. The existing schema,
source-factory and mutation-event catalogs remain canonical; no second Index registry is added.
The source factory also contributes the real sealed event listener through the Index module's
listener lifecycle after shared source materialization. Executable hosts do not read Inventory
tables or own Inventory conversion rules.

The bridge requires Auth's current worker service context for the exact source consumer. It
uses the original owner record/change IDs, minimum committed versions, retained tombstones,
generic Index persistence and an original-identity durable replay enqueue on activation.
Absent credentials/capabilities or failed persistence never acknowledge as successful no-ops.
Product and other source factories retain their existing selection and remote consumers.
Source presence does not establish authorization, tenant schema readiness, worker operation,
Index freshness, client recovery or authoritative cutover.

The concrete bounded bridge is documented in [inventory-index.md](docs/inventory-index.md).
It is authored, not compiled or deployed evidence. Canonical Index materialization registers the
selected-only generic worker using its single shared runtime; job/checkpoint result verification
is not a second ACK ledger. Failed-job retry/operator recovery, credentials/deployment, backend
and full operator-client parity remain pending. No extra stock transport/UI is exposed.
