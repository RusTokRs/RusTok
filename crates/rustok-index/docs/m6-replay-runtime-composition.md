# M6 replay runtime host composition

Status: `source_complete_owner_execution_pending`

This composition freezes the complete immutable Index source/schema registries and publishes three Index-owned capabilities from the same boundary:

- bounded replay dry-run;
- bounded durable replay runtime;
- one module-work registration for due reconciliation execution.

The server now wraps both replay execution surfaces in one request-bound operator: durable Full replay through `SharedIndexReplayRuntime` and side-effect-free Shadow replay through `SharedIndexReplayDryRunRuntime`. It does not add automatic replay-job scheduling. Public Shadow transport remains separate from runtime materialization and scheduler ownership.

## Composition order

1. selected modules contribute generic schema, source, and PostgreSQL source-factory contracts;
2. the server materializes `SharedIndexSchemaRegistry` and `SharedIndexSourceRegistry`;
3. `materialize_postgres_index_replay_runtime` requires both immutable registries;
4. it publishes replay dry-run and calls `register_postgres_index_reconciliation_work`;
5. only complete source/schema composition creates `ModuleWorkRegistrations` for Index;
6. it publishes `SharedIndexReplayRuntime` with ordinary and lifecycle-neutral interruptible run entry points;
7. the server retrieves the already-materialized `SharedIndexReplayDryRunRuntime` and wraps Full plus Shadow behind `IndexReplayOperatorRuntime`;
8. the server wraps reconciliation separately in its guarded operator runtime;
9. GraphQL currently exposes only durable Full run/cancel commands; Shadow GraphQL transport remains a separate next slice;
10. GraphQL schema initialization supplies the server-owned `StopHandle::is_stopping` probe to authorized durable replay run commands without making shutdown caller-controlled.

An absent source registry publishes no replay runtime, no dry-run runtime, and no empty Index work registration. A source registry without the shared schema registry fails closed. Duplicate replay or reconciliation-work materialization also fails closed. Server composition also fails closed if a durable replay runtime exists without the dry-run runtime that the guarded Shadow route requires.

The Index materializer performs no SQL and calls neither `tokio::spawn` nor a polling loop. Later server bootstrap collects all module-work registrations, starts the single generic `ModuleWorkScheduler` only when registrations exist, and binds that scheduler to the same shared `StopHandle` lifecycle used by durable replay GraphQL execution.

## Replay operator and command boundary

`IndexReplayOperatorRuntime` remains the only server-owned replay invocation authority. It requires an exact non-nil tenant/actor request context, a current request-scoped permission snapshot, and effective `modules:manage`.

Durable ordinary/interruptible run rejects cross-tenant requests before delegation and cancellation derives tenant only from the authorized context. `run_shadow` applies the same exact tenant and `modules:manage` guard before delegating to the side-effect-free dry-run runtime. Shadow has a separate typed operator error wrapper so the existing Full/cancel GraphQL error contract does not need to absorb an unexposed mode.

The current GraphQL transport authorizes before parsing caller schema/job input and delegates only durable Full run/cancel to this operator. Tenant, actor, worker identity, source name, database handles, scheduler controls, replay resource budgets, and shutdown state are not caller fields. The durable run mutation creates a server-owned worker identity and uses a fixed 100-row × 8-page chunk, per-page heartbeat, and 60-second lease.

Transport adapters must not call either `SharedIndexReplayRuntime` or `SharedIndexReplayDryRunRuntime` directly. See the durable server transport contract in `apps/server/docs/index-replay-graphql-transport.md` and the Shadow host boundary in `m6-bounded-replay-dry-run.md`.

## In-page host interruption boundary

`PostgresIndexReplayRunner` has a separate `run_interruptible` path that delegates one host-owned probe to the existing `IndexReplayWorker::run_next_page_interruptible` safe points.

An interrupted page is not marked failed and does not manufacture a persisted cancellation. After preserving any cancellation race, the runner yields the fenced job back to `pending` with lease ownership cleared and the last committed checkpoint unchanged. A later attempt can replay the same page; already-durable deliveries remain safe through inbox deduplication and source-version ordering.

`SharedIndexReplayRuntime::run_interruptible` and `IndexReplayOperatorRuntime::run_interruptible` carry that boolean probe through the immutable replay/runtime and authorization boundaries without importing the server lifecycle type into the Index crate or operator composition.

GraphQL schema initialization resolves or atomically creates one `StopHandle` in shared server runtime state and retains a watch receiver even for API-only hosts. `runIndexReplay` reads only `StopHandle::is_stopping` and invokes the guarded interruptible operator. It never calls `StopHandle::stop`, and no shutdown field is accepted from GraphQL input.

The retained SQLite runner packet covers interruption before source scan and interruption after one mutation is durable but before checkpoint commit. The latter resumes as `Duplicate` on attempt 2 before completing the checkpoint/job. Actual GraphQL/process-shutdown execution remains maintainer-run.

## Reconciliation scheduling boundary

The work registration added here is reconciliation-only. It discovers due pending or expired-running reconciliation jobs and delegates actual claim/takeover to `PostgresIndexReconciliationRunner`.

It does not schedule replay/rebuild jobs, create a second task, own a database lease, or expose a scheduler handle through either operator runtime.

## Explicitly open

- authorization-first GraphQL transport for guarded Shadow replay, including caller-carried bounded resume state and canonical locale handling;
- execute/admit retained replay interruption/restart evidence and end-to-end process-shutdown command evidence;
- durable GraphQL command execution/admission evidence and any separately justified HTTP/CLI/admin surfaces;
- automatic replay/rebuild job scheduling;
- retained PostgreSQL replay and reconciliation scheduler execution evidence;
- operator-visible scheduler health and metrics;
- targeted replay mutation execution over the bounded `IndexSource::load` contract;
- partition replay scope only after a real partition-capable source can filter before pagination.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
