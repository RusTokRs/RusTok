# Crate API: rustok-distribution

## Public surface

### Primary Public Functions
- `build_registry`: Assembles compile-time selected module implementations into `ModuleRegistry`.
- `build_runtime_extensions`: Seeds runtime extensions from active modules.
- `composition_identity`: Computes stable deterministic fingerprint and module manifest of active distribution.
- `generate_static_distribution`: Generates static distribution manifest and source files.

### Public Constants & Types
- `GENERATED_DISTRIBUTION_CARGO_MANIFEST_PATH`: Static cargo manifest path constant.
- `GENERATED_DISTRIBUTION_MANIFEST_PATH`: Static distribution manifest path constant.
- `GENERATED_DISTRIBUTION_REGISTRY_PATH`: Static registry path constant.
- `GeneratedStaticDistributionFiles`: Manifest file collection DTO.
- `GeneratedStaticDistributionManifest`: Static distribution metadata representation.
- `GeneratedStaticDistributionSource`: Generated Rust code container.
- `StaticDistributionGenerationError`: Errors arising during static distribution code generation.

## Platform Capability Bindings
- Static compile-time composition root for RusToK host executables.
- Bridges domain modules with index schemas and runtime extensions.

## Domain Collaborations
- Registers all active platform domain modules into the core registry.
- Bridges product indices when `mod-product` feature is enabled.
- Bridges AI translation factories when `ai-translation` feature is enabled.

## Minimum Contract Set

### Input DTOs/Commands
- `build_registry`: Assembles and returns configured `ModuleRegistry`.
- `generate_static_distribution`: Takes target output directory and serializes distribution artifacts.

### Domain Invariants
- Composition identity hash is deterministic and uniquely identifies active module set.
- All registered modules maintain proper dependency order and invariant initialization.
- Dynamic extension collisions are rejected at registration time.

### Events / Outbox Side Effects
- Assembles event consumers and schema providers into shared runtime context.

### Errors / Failure Codes
- `StaticDistributionGenerationError::Io`: File system writing failures.
- Registration errors returned when conflicting runtime ports are registered.

