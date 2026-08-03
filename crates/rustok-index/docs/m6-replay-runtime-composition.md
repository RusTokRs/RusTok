# M6 replay runtime host composition

Status: `source_complete_owner_execution_pending`

This composition freezes the complete immutable Index source/schema registries and publishes three Index-owned capabilities from the same boundary:

- bounded replay dry-run;
- bounded guarded replay runtime;
- one module-work registration for due reconciliation execution.

It does not add automatic replay-job scheduling or a command transport.

## Composition order

1. selected modules contribute generic schema, source, and PostgreSQL source-factory contracts;
2. the server materializes `SharedIndexSchemaRegistry` and `SharedIndexSourceRegistry`;
3. `materialize_postgres_index_replay_runtime` requires both immutable registries;
4. it publishes replay dry-run and calls `register_postgres_index_reconciliation_work`;
5. only complete source/schema composition creates `ModuleWorkRegistrations` for Index;
6. it publishes `SharedIndexReplayRuntime`;
7. the server wraps replay and reconciliation capabilities in guarded operator runtimes.

An absent source registry publishes no replay runtime, no dry-run runtime, and no empty Index work registration. A source registry without the shared schema registry fails closed. Duplicate replay or reconciliation-work materialization also fails closed.

The materializer performs no SQL and calls neither `tokio::spawn` nor a polling loop. Later server bootstrap collects all module-work registrations, starts the single generic `ModuleWorkScheduler` only when registrations exist, and binds that scheduler to the shared `StopHandle` lifecycle.

## Replay operator boundary

`IndexReplayOperatorRuntime` remains the only server-owned replay invocation boundary. It requires an exact non-nil tenant/actor request context, a current request-scoped permission snapshot, and effective `modules:manage`. Run rejects cross-tenant requests before delegation; cancellation derives tenant only from the authorized context.

Transport adapters must not call `SharedIndexReplayRuntime` directly.

## Reconciliation scheduling boundary

The work registration added here is reconciliation-only. It discovers due pending or expired-running reconciliation jobs and delegates actual claim/takeover to `PostgresIndexReconciliationRunner`.

It does not schedule replay/rebuild jobs, create a second task, own a database lease, or expose a scheduler handle through either operator runtime.

## Explicitly open

- authorized replay GraphQL, HTTP, CLI, or admin command surfaces;
- automatic replay/rebuild job scheduling;
- retained PostgreSQL replay and reconciliation scheduler execution evidence;
- operator-visible scheduler health and metrics;
- in-page mutation/checkpoint timeout completion;
- targeted/full/shadow repair and complete drift diagnosis.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
