# `rustok-build` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-build` defines the platform build and release persistence contracts for RusToK. It manages build entities, tracks build lifecycle execution plans, normalizes module manifest contributions, and publishes transactional build events.

Key responsibilities:
- Persists and tracks platform build states (`BuildStage`, `BuildStatus`, `DeploymentProfile`).
- Coordinates immutable build execution plans (`BuildExecutionPlan`, `FrontendBuildPlan`, `RoleBuildPlan`) for server and frontend compilation targets.
- Provides atomic queued build claiming and execution dispatching through `BuildService` and `BuildExecutionService`.
- Exposes neutral API projections (`PlatformBuildSnapshot`) and read-only host query ports (`BuildControl`, `SharedBuildControl`).
- Normalizes and validates module manifest contributions (`NormalizedModuleContributionManifest`) preventing hand-authored privilege escalation.
- Publishes build lifecycle domain events (`BuildEvent`) with explicit scope separation (`BuildEventScope::Platform` vs `BuildEventScope::Tenant`).

## 2. Build Lifecycle States & Execution Plans

1. **Build Status Lifecycle**:
   - States: `Queued` -> `Running` -> `Success` | `Failed` | `Cancelled`.
   - `is_final()` guarantees terminal state classification.
   - Atomic CAS transitions in PostgreSQL/SQLite ensure that at most one worker claims a queued build at a time (`claim_queued_build`, `claim_next_queued_build`).
2. **Build Stages**:
   - `Pending` -> `Checkout` -> `Build` -> `Test` -> `Deploy` -> `Complete`.
3. **Execution Plan Composition**:
   - Server plan specifies `cargo_package`, `cargo_profile`, `cargo_target`, and `cargo_features`.
   - Frontend plans specify Trunk/Cargo toolchains, workspace paths, and output artifact paths (`FrontendBuildPlan`).
   - `BuildCommandSpec` translates plan data into sandboxed OS command executions (`run_build_command`) passing `RUSTOK_MODULES_MANIFEST` environments.

## 3. Transactional Event Publishing & Scopes

- `BuildEventPublisher`: Port trait decoupling the build service from transport implementations.
- `EventBusBuildEventPublisher`: Publishes domain events to `rustok_core::EventBus` with non-nil actor and correlation identities.
- `BuildEventScope`:
  - `Platform`: Global composition changes are published with a nil tenant UUID (`Uuid::nil()`) to prevent accidental tenant misattribution.
  - `Tenant(Uuid)`: Tenant-scoped builds retain explicit tenant identity.
- `BuildEventPublicationContext`: Enforces non-nil actor UUID, non-nil correlation UUID, and bounded trace IDs $\le$ 512 bytes.

## 4. Manifest Normalization & Contribution Brokering

- `normalize_module_contribution_manifest(...)`: Validates raw module contribution manifests against builder capabilities.
- Enforces strict rules:
  - Rejects hand-authored reserved identity keys (`ownerProvider`, `providerVersion`).
  - Injects canonical versioned provider metadata from the module envelope.
  - Validates role export schemas and capability dependencies.
- Prevents malicious or misconfigured module manifests from escalating privileges during the build process.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - History page queries enforce `limit` between 1 and 100, and `offset` $\le$ 1,000,000.
   - Trace IDs bounded between 1 and 512 bytes; actor and correlation IDs must be non-nil.
   - Contribution manifests reject undeclared builder capabilities and malformed component types.
2. **Branch Completeness & Anti-Swallowing**:
   - Build step failures in `run_build_command` immediately record the error message and transition build status to `Failed`.
   - Manifest snapshot serialization errors propagate explicitly without leaving orphaned builds.
3. **State Transition Integrity**:
   - Mutually exclusive build claiming (`update_many` with `Status.eq(Queued)`) prevents split-brain execution across multiple workers.
   - Terminal builds cannot be re-executed or modified.
4. **Side-Effect Completeness**:
   - Materializes temporary manifest snapshots to isolated build-specific directory paths (`build_manifest_snapshot_path(build_id)`).
   - Cleans null values from manifest snapshots prior to TOML serialization.
5. **Algorithmic Correctness**:
   - Build request deduplication computes SHA-256 digests over snapshot, profile, artifact identity, and execution plan.
   - Progress calculation: `15 + (((index + 1) * 75) / total_steps)`.
6. **Order of Operations & Concurrency**:
   - Builds claimed atomically inside transactions before spawning child processes.
   - Events published only after transaction commits or state transitions succeed.
