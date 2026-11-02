# `rustok-cli-platform` Crate API & Architecture Contract

## 1. Architectural Purpose & Boundaries

`rustok-cli-platform` provides the core platform-level operations and maintenance command provider for the RusToK CLI. It exposes operational commands for inspecting version metadata, collecting database performance statistics and EXPLAIN plans (`core db-baseline`), orchestrating queued module manifest build rebuilds (`core rebuild`), and checking or executing schema migrations (`migrate up`, `migrate status`).

Key responsibilities:
- Implements the platform `CommandProvider` (`PlatformCommandProvider`).
- Exposes `command_provider(&RuntimeComposition) -> Box<dyn CommandProvider>`.
- Collects PostgreSQL `pg_stat_statements` and EXPLAIN query execution plans across hot-path tables (`db_baseline::execute`).
- Dispatches queued build jobs through `rustok-build::BuildExecutionService` (`rebuild::execute`).
- Runs SeaORM schema migrations via `rustok-migrations::Migrator`.

## 2. Platform Provider Descriptors & Namespace Routing

`PlatformCommandProvider` registers 5 canonical command descriptors:
1. `core version`: Returns package name and semantic version metadata.
2. `core db-baseline`: Gathers slow query statistics and EXPLAIN plans for tenant-scoped query verification.
3. `core rebuild`: Executes a specific or next-queued build plan; declares `supports_dry_run: true`.
4. `migrate up`: Applies all pending SeaORM database migrations against the configured host database.
5. `migrate status`: Queries and returns the list of unapplied migration identifiers.

## 3. Database Baseline & Query Plan Extraction

- **Backend Awareness**: Adapts EXPLAIN syntax across SQLite (`EXPLAIN QUERY PLAN`) and PostgreSQL (`EXPLAIN (FORMAT TEXT)`).
- **Graceful Degradation**: If `pg_stat_statements` is unavailable or running on a non-Postgres engine, returns `available: false` with descriptive error details rather than failing execution.
- **Tenant Isolation**: Resolves target tenant from `--tenant-id` option or defaults to the active default tenant through `TenantService`.
- **Artifact Export**: If `--output` is specified, writes formatted JSON baseline telemetry to the target path.

## 4. Rebuild Execution Pipeline & Dry-Run Support

- Integrates with `rustok_build::BuildExecutionService` using an in-memory or host database connection.
- Supports targeted execution by `--build-id <uuid>` or automatic dequeue of the next pending build (`execute_next_queued_build`).
- Honors `dry_run: true` to simulate manifest compilation and step execution without mutating build state.

## 5. Universal Logic Dimensions Verification

1. **Preconditions & Bounds**:
   - Commands requiring database access fail gracefully with typed `CliCoreError::CommandFailed` when executed on a database-less runtime.
   - `--top-n` must be a valid positive integer; invalid inputs return `InvalidInput`.
   - `--build-id` validates UUID format.
2. **Branch Completeness & Anti-Swallowing**:
   - Unknown commands return typed `CliCoreError::UnknownCommand`.
   - Migration, database connection, and file write errors are wrapped in `CliCoreError::CommandFailed`.
3. **State Transition Integrity**:
   - Schema migrations run through transactional migrator execution.
   - Build execution adheres to `BuildExecutionService` state transition invariants.
4. **Side-Effect Completeness**:
   - Baseline file export (`--output`) writes full UTF-8 JSON payloads.
   - Version command is completely side-effect free.
5. **Algorithmic Correctness**:
   - Time-window calculations for dashboard baseline snapshots compute exact 30-day current and previous periods.
6. **Order of Operations & Concurrency**:
   - Database connection acquired from runtime before query execution.
   - Dry-run validation performed prior to build job execution.
