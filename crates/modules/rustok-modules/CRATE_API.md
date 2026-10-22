# rustok-modules / CRATE_API

## Public Modules
`artifact`, `artifact_capability_router`, `artifact_cas`, `authoring`, `build`, `data_backfill`, `data_copier`, `data_snapshot`, `dynamic_lifecycle`, `marketplace`, `module`, `oci`, `policy`, `promotion`, `runtime`, `secrets`, `settings`, `trust`.

## Primary Public Types and Signatures
- `pub struct ModulesModule`: Core platform module registration and migration provider.
- `pub struct ArtifactPackageDescriptor`: Metadata and integrity manifest for distribution packages.
- `pub struct ModuleLifecycle`: State machine coordinating installed, active, disabled, and upgrade states.
- `pub struct ModuleRegistryService`: Module marketplace catalog and package discovery.
- `pub struct PolicyEngine`: Cryptographic verification of signatures, provenance, and trust policies.

## Invariants
- **Digest-addressed immutability**: All module artifacts are verified against cryptographic content hashes (SHA-256) before extraction, publication, or execution.
- **Fail-closed admission**: Unsigned, untrusted, or mismatched module packages are rejected prior to schema migration or dynamic execution.
- **Dependency graph acyclicity**: Module dependency resolution strictly detects and rejects circular dependencies.

## Platform Capability Bindings
- **rustok-core**: Module lifecycle traits (`RusToKModule`), migration traits (`MigrationSource`), and base error infrastructure
- **rustok-api**: Host runtime context and worker handler traits (`ModuleWorkHandler`, `ModuleWorkItem`)
- **rustok-runtime**: Worker task registrations (`ModuleWorkRegistrations`) and instance filesystem layout
- **rustok-events**: Canonical event envelope and contract validation
- **rustok-outbox**: Transactional event dispatching and durable command idempotency receipts

## Domain Collaborations
- **All platform and domain modules**: Canonical module registration, activation toggles, and dependency resolution
- **rustok-tenant**: Tenant-level module enablement overlays and scoped configuration

## Minimum Contract Set

### Input DTOs/Commands
- `ModuleInstallCommand`: Command installing or upgrading an artifact package from a registered source.
- `ModuleToggleCommand`: Command activating or disabling an installed module.
- `ModulePublicationCommand`: Command publishing a module artifact to the platform registry.

### Domain Invariants
- Cryptographic provenance and signatures must be verified before any local storage or database state mutation.
- Module toggles and promotions commit atomically within database transaction boundaries.

### Events / Outbox Side Effects
- Emits `ModuleInstalledEvent`, `ModuleActivatedEvent`, `ModuleDisabledEvent`, `ModuleUpgradedEvent` via `rustok-outbox`.

### Errors / Failure Codes
- Strongly-typed `ModuleError`, `ArtifactError`, `PolicyError`, and `RegistryError` identifying failure causes without masking.
