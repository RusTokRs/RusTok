# M6 replay runtime host composition

Status: `source_complete_owner_execution_pending`

This composition freezes the complete immutable Index source/schema registries and publishes three Index-owned capabilities from the same boundary:

- bounded replay dry-run;
- bounded shared replay runtime containing durable Full plus exact-key Targeted execution;
- one module-work registration for due reconciliation execution.

The server wraps all replay execution surfaces in one request-bound operator: durable Full and bounded Targeted through `SharedIndexReplayRuntime`, plus side-effect-free Shadow through `SharedIndexReplayDryRunRuntime`. It also publishes a separate schema-wide/exact-locale Shadow transport adapter that seals caller-carried continuation state. None of these boundaries add automatic replay-job scheduling.

The shared continuation contract has one current unversioned envelope and binds optional canonical locale identity inside encrypted claims. Shadow dry-run and GraphQL execution carry that same locale scope end to end.

## Composition order

1. selected modules contribute generic schema, source, and PostgreSQL source-factory contracts;
2. the server materializes `SharedIndexSchemaRegistry` and `SharedIndexSourceRegistry`;
3. `materialize_postgres_index_replay_runtime` requires both immutable registries;
4. it publishes replay dry-run and calls `register_postgres_index_reconciliation_work`;
5. only complete source/schema composition creates `ModuleWorkRegistrations` for Index;
6. it constructs `IndexReplayTargetedExecutor<PostgresMutationStore>` from the same frozen source/schema registries and host database;
7. it publishes `SharedIndexReplayRuntime` containing the durable Full runner plus that bounded Targeted executor, with ordinary Full, Targeted, lifecycle-neutral interruptible Full and cancel entry points kept distinct;
8. the server retrieves the already-materialized `SharedIndexReplayDryRunRuntime` and wraps Full, Targeted and Shadow behind `IndexReplayOperatorRuntime`;
9. the server materializes the deployment continuation keyring, then publishes `IndexReplayShadowTransportRuntime` and the source-page diagnosis runtime from that same keyring snapshot;
10. the server wraps reconciliation separately in its guarded operator runtime;
11. GraphQL exposes durable Full run/cancel plus schema-wide or exact-locale Shadow through the sealed transport adapter; Targeted has no public transport in this slice;
12. GraphQL schema initialization supplies the server-owned `StopHandle::is_stopping` probe only to authorized durable Full replay run commands without making shutdown caller-controlled.

An absent source registry publishes no replay runtime, no dry-run runtime, no Shadow transport runtime and no empty Index work registration. A source registry without the shared schema registry fails closed. Duplicate replay or reconciliation-work materialization also fails closed. Server composition fails closed if a replay runtime exists without the dry-run runtime that the guarded Shadow route requires.

The Index materializer performs no SQL and calls neither `tokio::spawn` nor a polling loop. Constructing `PostgresMutationStore` for Targeted only captures the host `DatabaseConnection`; actual SQL remains inside the existing mutation sink and occurs only after host authorization, exact-target admission and full returned-batch preflight. Later server bootstrap collects all module-work registrations, starts the single generic `ModuleWorkScheduler` only when registrations exist, and binds that scheduler to the same shared `StopHandle` lifecycle used by durable Full replay GraphQL execution.

## Replay operator and command boundary

`IndexReplayOperatorRuntime` remains the server-owned replay invocation authority. It requires an exact non-nil tenant/actor request context, a current request-scoped permission snapshot, and effective `modules:manage`.

Durable ordinary/interruptible Full run rejects cross-tenant requests before delegation and cancellation derives tenant only from the authorized context. `run_targeted` applies the same exact tenant and `modules:manage` guard to the canonical `IndexSourceLoadRequest` before delegating to `SharedIndexReplayRuntime::run_targeted`. `run_shadow` applies the same guard before delegating to the side-effect-free dry-run runtime. Targeted and Shadow each keep a separate typed operator error wrapper around the unchanged Full/cancel `IndexReplayOperatorError`, so adding either execution surface does not widen the existing GraphQL Full/cancel error contract or create another authorization model.

`SharedIndexReplayRuntime::run_targeted` wraps the already-validated load request into `IndexReplayModeSelection::Targeted` and executes `IndexReplayTargetedExecutor<PostgresMutationStore>`. The Targeted executor validates active-schema entity/locale shape before source resolution/load, performs one bounded exact-key load, preflights the full returned batch, then persists only admitted source mutations through the existing stable-event replay sink.

Targeted has no replay job, checkpoint, lease, heartbeat, worker, cancellation state, graceful-stop handling, scheduler registration or automatic retry/requeue state. Partial progress safety relies on the existing source-owned stable event UUID / `index_inbox` duplicate path, not on a new checkpoint.

`IndexReplayShadowTransportRuntime` sits above the guarded operator. It owns no database handle, scheduler, job, checkpoint, lease, cancellation or retry state. It repeats exact-tenant authorization before opening continuation, derives schema-wide or exact-locale tenant/schema/source scope from the frozen source registry, calls only `IndexReplayOperatorRuntime::run_shadow`, and seals any outgoing raw source cursor before returning a transport-safe outcome.

`IndexSourceContinuationScope::from_registry` is schema-wide; `for_locale` binds one exact canonical `LocaleKey`. The transport constructs the matching `IndexReplayDryRunRequest::new` or `for_locale`, and the dry-run runtime builds every actual source page with the same `IndexSourceScanRequest::new` or `for_locale` scope. Exact-locale dry-run fails closed for `LocaleMode::None` before source execution.

Schema-wide and exact-locale tokens cannot cross scopes, and different canonical locales cannot exchange tokens. The codec does not retain an old-format decoder or format-version family.

The GraphQL transport authorizes before parsing caller input. Durable Full uses a fixed 100-row × 8-page chunk, per-page heartbeat and 60-second lease. Shadow uses the same fixed 100-row × 8-page scan budget but no worker, heartbeat or lease. Its only resumable caller state is the authenticated confidential continuation token. Targeted public transport remains intentionally absent until a dedicated authorization-first exact-target input boundary is source-complete.

Transport adapters must not call either `SharedIndexReplayRuntime` or `SharedIndexReplayDryRunRuntime` directly. A future Targeted transport must call only `IndexReplayOperatorRuntime::run_targeted`. See `apps/server/docs/index-replay-graphql-transport.md`, `m6-targeted-replay-mutation-application.md` and `m6-bounded-replay-dry-run.md`.

## In-page host interruption boundary

`PostgresIndexReplayRunner` has a separate `run_interruptible` path that delegates one host-owned probe to the existing `IndexReplayWorker::run_next_page_interruptible` safe points.

An interrupted page is not marked failed and does not manufacture a persisted cancellation. After preserving any cancellation race, the runner yields the fenced job back to `pending` with lease ownership cleared and the last committed checkpoint unchanged. A later attempt can replay the same page; already-durable deliveries remain safe through inbox deduplication and source-version ordering.

`SharedIndexReplayRuntime::run_interruptible` and `IndexReplayOperatorRuntime::run_interruptible` carry that boolean probe through the immutable replay/runtime and authorization boundaries without importing the server lifecycle type into the Index crate or operator composition.

GraphQL schema initialization resolves or atomically creates one `StopHandle` in shared server runtime state and retains a watch receiver even for API-only hosts. `runIndexReplay` reads only `StopHandle::is_stopping` and invokes the guarded interruptible operator. It never calls `StopHandle::stop`, and no shutdown field is accepted from GraphQL input.

Targeted does not use this durable interruption path: it owns no durable pending state, and each bounded exact-key invocation either returns an outcome or a typed failure. Exact retry relies on stable source event identities. Shadow also does not use the durable interruption path; a later Shadow invocation can resume only from a sealed continuation returned by a completed bounded call under the same schema/locale scope.

The retained SQLite runner packet covers Full interruption before source scan and interruption after one mutation is durable but before checkpoint commit. The latter resumes as `Duplicate` on attempt 2 before completing the checkpoint/job. Actual GraphQL/process-shutdown execution remains maintainer-run.

## Reconciliation scheduling boundary

The work registration added here is reconciliation-only. It discovers due pending or expired-running reconciliation jobs and delegates actual claim/takeover to `PostgresIndexReconciliationRunner`.

It does not schedule replay/rebuild jobs, create a second task, own a database lease, or expose a scheduler handle through either replay transport.

## Explicitly open

- dedicated authorization-first Targeted public transport over the guarded host capability;
- execute/admit schema-wide and exact-locale Shadow GraphQL plus continuation-key deployment evidence;
- execute/admit retained replay interruption/restart evidence and end-to-end process-shutdown command evidence;
- durable GraphQL command execution/admission evidence and any separately justified HTTP/CLI/admin surfaces;
- automatic replay/rebuild job scheduling;
- retained PostgreSQL replay and reconciliation scheduler execution evidence;
- operator-visible scheduler health and metrics;
- partition replay scope only after a real partition-capable source can filter before pagination.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
