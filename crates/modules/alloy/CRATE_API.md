# alloy / CRATE_API

## Public surface

### Primary Public Types and Signatures

- `pub struct AlloyModule`: Native RusToK module engine implementing `RusToKModule` and `MigrationSource`.
- `pub fn create_default_engine() -> ScriptEngine`: Constructs default Rhai-backed scripting engine with standard libraries.
- `pub fn create_engine_with_config(config: RhaiConfig) -> ScriptEngine`: Constructs Rhai engine with custom limits and settings.
- `pub fn create_engine_for_phase(phase: ExecutionPhase) -> ScriptEngine`: Constructs Rhai engine tailored with phase-appropriate capability bridge.
- `pub fn build_alloy_runtime(...) -> SharedAlloyRuntime`: Factory for assembling host-neutral Alloy execution runtime.
- `pub struct ScriptEngine`: Core script engine providing compilation, sandboxed execution, and AST validation.
- `pub struct ScriptOrchestrator<R: ScriptRegistry>`: Multi-phase orchestrator for entity lifecycle hooks (`Before`, `After`, `OnCommit`).
- `pub struct AlloyAuthoringService`: Service governing script authoring, drafts, lifecycle transitions, code reviews, and workspace test executions.
- `pub struct Scheduler`: Automated execution scheduler for scheduled and cron-triggered scripts.
- `pub struct ExecutionContext`: Ambient execution context carrying tenant identity, user id, execution phase, and entity proxies.
- `pub struct Script`: Domain entity representing a registered script with triggers, status, and source workspace.
- `pub struct RhaiWorkspace`: Multi-file or single-source Rhai script source bundle with canonical content hashing.
- `pub struct SeaOrmStorage`: Relational persistence adapter for scripts and source revisions.
- `pub struct SeaOrmExecutionLog`: Database audit sink persisting execution results and diagnostics.
- `pub struct AlloyQuery` & `pub struct AlloyMutation`: Root GraphQL schema fields providing script management and execution.
- `pub fn axum_router(...) -> axum::Router`: REST router providing execution history and release governance endpoints.
- `pub enum ScriptError`: Comprehensive error enum covering compile errors, timeouts, memory/operation limits, and aborts.

## Platform Capability Bindings

- **Sandboxed Script Execution**: Interacts with `rustok-sandbox` for isolated Rhai execution with strictly bounded resources.
- **Transactional Migrations**: Implements `MigrationSource` declaring migrations for script definitions, revisions, and execution logs.
- **Hook Integration & Orchestration**: Provides hooks for domain entities (`deal`, `order`, `product`, `invoice`, etc.) without polluting domain crates.
- **Audit Logging**: Emits structured execution log entries for all manual, scheduled, and hook-driven executions.

## Domain Collaborations

- **`rustok-sandbox`**: Rhai execution kernel, capability brokers, and resource quotas.
- **`rustok-core`**: Module metadata, runtime extensions, and migration lifecycle.
- **`rustok-mcp`**: MCP tool adapters for script authoring, scaffolding, and validation.
- **`apps/server`**: Host composition of GraphQL, REST routes, and background scheduler tasks.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateAlloyScriptCommand`: Requires valid name, non-empty source workspace, and valid `ScriptTrigger`.
- `UpdateAlloyScriptCommand`: Requires existing script ID and updated source workspace or trigger.
- `ChangeAlloyScriptLifecycleCommand`: Carries target script ID and explicit `AlloyScriptLifecycleAction` (`Activate`, `Deactivate`, `Deprecate`).
- `ReviewAlloyScriptCommand`: Carries review decision (`Approve`, `Reject`), reviewer ID, and optional review notes.
- `RunAlloyScriptCommand`: Input payload for ad-hoc or test invocation of a script draft or active script.

### Domain Invariants
- **Sandbox Boundary**: All script executions must run within bounded execution limits (max operations, call depth, memory limits, and timeouts).
- **Execution Log Completeness**: Every executed script run (whether success, failure, timeout, or abort) must record an immutable execution log entry.
- **Draft Isolation**: Scripts under review or in draft status must never execute in production hook pipelines until approved and activated.
- **Tenant Context Isolation**: Script storage, execution logs, and scheduler jobs must strictly enforce tenant scoping without cross-tenant bleed.
- **Abort Signaling**: Calling `abort("reason")` within a `Before` hook must halt entity persistence with `ScriptError::Aborted`.

### Events / Outbox Side Effects
- Does not directly publish outbox events; script executions trigger internal audit sink mutations.

### Errors / Failure Codes
- `ScriptError::Compile`: Syntax or semantic error during script parsing/compilation.
- `ScriptError::Timeout`: Script execution exceeded configured wall-clock timeout budget.
- `ScriptError::OperationLimit`: Script exceeded allowed Rhai operation quota.
- `ScriptError::ResourceLimit`: Script exceeded memory, string, array, or map size limits.
- `ScriptError::Aborted`: Script intentionally halted lifecycle transition via `abort(...)`.
- `AlloyAuthoringError::NotFound`: Requested script or revision does not exist.
- `AlloyAuthoringError::InvalidState`: Illegal lifecycle transition (e.g. activating an unreviewed draft).
