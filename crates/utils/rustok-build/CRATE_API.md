# rustok-build / CRATE_API

## Public surface

### Primary Types & Models
- `BuildStage`: progression stages of the build lifecycle (`Queued`, `Preparing`, `Compiling`, `Packaging`, `Completed`, `Failed`).
- `BuildStatus`: terminal and active states (`Pending`, `Running`, `Success`, `Failed`, `Cancelled`).
- `DeploymentProfile`: targeted optimization profile (`Dev`, `Release`).
- `BuildExecutionPlan`: complete plan binding role, runtime mode, profiles, and step commands.
- `RoleBuildPlan`: individual role targets (`Server`, `Admin`, `Storefront`).
- `BuildRuntimeMode`: runtime compilation modes (`Native`, `Wasm`, `Container`).
- `FrontendBuildPlan`: Trunk/Wasm and static frontend build specifications.
- `BuildRequest`: build submission input (`build_id`, `tenant_id`, `module_specs`, `profile`).
- `BuildExecutionReport`: execution summary including timing, logs hash, and artifact digest.
- `PlatformBuildSnapshot`: framework-neutral projection of a build entity.

### Port Traits & Services
- `BuildControl`: trait owning build submission, cancellation, and inspection operations.
- `BuildExecutionService`: executor processing queued build plans.
- `BuildService`: domain service for build entity lifecycle and deduplication.
- `BuildEventPublisher`: transactional event publishing port (`EventBusBuildEventPublisher`, `NoopBuildEventPublisher`).

### Manifest & Command Helpers
- `run_build_command(...)`: sandboxed process execution helper.
- `normalize_module_contribution_manifest(...)`: validates and canonicalizes module contribution manifests.

## Invariants

- **Safe Code Boundary**: Forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Complete Execution Identity Deduplication**: Builds are deduplicated by artifact identity, compilation profile, and runtime mode rather than module manifest alone.
- **Port-Based Event Publishing**: Build events are published strictly through explicit `BuildEventPublisher` ports without hidden out-of-band communication.
- **Pure Build Authority**: Owns build plan compilation and history records with zero production release, deployment head, or rollback authority.
