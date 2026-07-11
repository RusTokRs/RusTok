# Implementation plan for `rustok-modules`

## Scope

Own the mandatory module artifact control plane and marketplace without making
the server know optional domain crates.

## Current State

The Core entry point and immutable artifact descriptor/lineage contract are
implemented. Effective-module policy calculation now belongs here too: the host
loads the active manifest and persisted tenant overrides, then delegates core,
default and override resolution to `resolve_effective_modules`; Core modules
cannot be disabled and unknown persisted slugs are not resurrected. The
module capability also owns toggle topology validation through
`validate_module_toggle`; persistence, hooks and operation journaling remain in
the server lifecycle adapter for the next extraction. The
operation status, failure classification and recovery-action contracts now also
belong to this module, so the future persistence adapter will not introduce
server-owned lifecycle DTOs. `ModuleOperationJournal` now owns operation record
creation and status transitions over the caller-provided transaction boundary;
`TenantModuleStateStore` owns tenant module-state upserts over the same boundary.
`run_module_lifecycle_hook` owns module context construction and pre/post hook
dispatch. `execute_module_toggle` now owns the journal-aware normal toggle
sequence; server recovery orchestration remains a separate follow-up. The
digest-pinned OCI installation contract also verifies package identity and
payload bytes before admitting a Rhai, WASM, or sidecar artifact to the shared
sandbox. `SeaOrmArtifactInstallationStore` and its scoped PostgreSQL migration
now persist immutable installation records under RLS.
`OciDistributionArtifactRegistry` resolves digest-pinned manifest/config/layer
tuples and verifies their relationship before admission. Registry signatures,
SBOM verification and owner transports remain to be moved from the server.
`ArtifactRuntime` now resolves the installed digest-pinned reference at execution
time, verifies its package identity against the durable record, and invokes the
shared `rustok-sandbox` runtime. Alloy draft execution and sidecar support are
still separate follow-up work.

## Milestones

1. Move manifest, composition, governance and tenant lifecycle services/models
   from `apps/server` into this Core module.
2. Persist platform installation, capability-grant, migration and rollback state.
3. Resolve and verify OCI artifacts, then activate them through `rustok-sandbox`.
4. Make Alloy publish, fork and evolve Rhai artifact releases through the same
   immutable descriptor contract.
5. Replace server Cargo module features with static distribution promotion.

## Verification

- Artifact descriptor, executor-selection and lineage contract tests.
- Registry signature/dependency/install/rollback integration tests.
- Tenant isolation and GraphQL/native transport parity tests.

## Update Rules

Update this plan, module registry and central control-plane plan whenever
artifact identity, lifecycle, marketplace governance or sandbox admission changes.
