# M6 replay runtime host composition

Status: `source_complete_owner_execution_pending`

This composition freezes the complete immutable Index source/schema registries and publishes three
Index-owned capabilities from the same boundary:

- bounded replay dry-run;
- bounded shared replay runtime containing durable Full plus exact-key Targeted execution;
- one module-work registration for due reconciliation execution.

The server wraps all replay execution surfaces in one request-bound operator: durable Full and bounded
Targeted through `SharedIndexReplayRuntime`, plus side-effect-free Shadow through
`SharedIndexReplayDryRunRuntime`. It also publishes the schema-wide/exact-locale Shadow continuation
adapter. None of these boundaries add automatic replay-job scheduling.

The shared continuation contract has one current unversioned envelope and binds optional canonical
locale identity inside encrypted claims. Shadow dry-run and GraphQL execution carry that same locale
scope end to end.

## Composition order

1. selected modules contribute generic schema, source, and PostgreSQL source-factory contracts;
2. the server materializes `SharedIndexSchemaRegistry` and `SharedIndexSourceRegistry`;
3. `materialize_postgres_index_replay_runtime` requires both immutable registries;
4. it publishes replay dry-run and calls `register_postgres_index_reconciliation_work`;
5. only complete source/schema composition creates `ModuleWorkRegistrations` for Index;
6. it constructs `IndexReplayTargetedExecutor<PostgresMutationStore>` from the same frozen source/schema registries and host database;
7. it publishes `SharedIndexReplayRuntime` containing the durable Full runner plus that bounded Targeted executor, with Full run, Targeted run, interruptible Full and cancel entry points kept distinct;
8. the server retrieves `SharedIndexReplayDryRunRuntime` and wraps Full, Targeted and Shadow behind `IndexReplayOperatorRuntime`;
9. the server materializes the deployment continuation keyring and publishes `IndexReplayShadowTransportRuntime` plus source-page diagnosis from that same keyring snapshot;
10. the server wraps reconciliation separately in its guarded operator runtime;
11. GraphQL exposes durable Full run/cancel, exact-key Targeted run, and schema-wide/exact-locale Shadow;
12. GraphQL schema initialization supplies `StopHandle::is_stopping` only to authorized durable Full replay run commands.

An absent source registry publishes no replay runtime, no dry-run runtime, no Shadow transport runtime
and no empty Index work registration. A source registry without the shared schema registry fails
closed. Duplicate replay or reconciliation-work materialization also fails closed. Server composition
fails closed if a replay runtime exists without the dry-run runtime required by the guarded Shadow
route.

The Index materializer performs no SQL and calls neither `tokio::spawn` nor a polling loop.
Constructing `PostgresMutationStore` for Targeted only captures the host `DatabaseConnection`; actual
SQL remains inside the existing mutation sink and occurs only after host authorization, exact-target
admission and whole-batch preflight. Later server bootstrap collects module-work registrations and
starts the single generic `ModuleWorkScheduler` only when registrations exist.

## Replay operator and command boundary

`IndexReplayOperatorRuntime` remains the server-owned replay invocation authority. It requires exact
non-nil tenant/actor request context, a current request-scoped permission snapshot and effective
`modules:manage`.

Durable ordinary/interruptible Full run rejects cross-tenant requests before delegation and
cancellation derives tenant only from authorized context. `run_targeted` applies the same exact tenant
and `modules:manage` guard to the canonical `IndexSourceLoadRequest` before delegating to
`SharedIndexReplayRuntime::run_targeted`. `run_shadow` applies the same guard before delegating to the
side-effect-free dry-run runtime.

Targeted and Shadow each keep a separate typed operator error wrapper around the unchanged Full/cancel
`IndexReplayOperatorError`, so adding those execution surfaces does not widen the existing GraphQL
Full/cancel error contract or create another authorization model.

`SharedIndexReplayRuntime::run_targeted` wraps the validated load request into
`IndexReplayModeSelection::Targeted` and executes
`IndexReplayTargetedExecutor<PostgresMutationStore>`. The executor validates active-schema entity and
locale shape before source resolution/load, performs one bounded exact-key load, preflights the full
returned batch, then persists only admitted source mutations through the existing stable-event replay
sink.

Targeted has no replay job, checkpoint, lease, heartbeat, worker, cancellation state, graceful-stop
handling, scheduler registration or automatic retry/requeue state. Partial progress safety relies on
the source-owned stable event UUID / `index_inbox` duplicate path rather than a new checkpoint.

## Targeted GraphQL boundary

`runIndexReplayTargeted` is mounted on the existing `IndexReplayMutation` object. It derives tenant and
actor from server request context and requires request-bound `modules:manage` before parsing untrusted
schema, entity UUID or locale strings.

The caller supplies one exact schema routing identity plus 1..=256 target keys. Each key contains only
an entity UUID and optional locale. Locale is canonicalized per key through `LocaleKey`; the transport
then creates the canonical `IndexSourceLoadRequest` and delegates only to
`IndexReplayOperatorRuntime::run_targeted`. It does not call `SharedIndexReplayRuntime` directly.

The Targeted payload exposes requested/mutation/missing/applied/duplicate/stale counts only. The
resolved source name carried internally by `IndexReplayTargetedOutcome` is not serialized. GraphQL
accepts no tenant, actor, source name, generic mode, worker, page budget, job/checkpoint, lease,
cancellation, retry/requeue, scheduler or partition controls.

Targeted does not use `StopHandle` because it has no durable pending state. An exact retry is a new
bounded invocation over the same requested keys and relies on stable source event identities for
idempotent convergence.

## Shadow continuation boundary

`IndexReplayShadowTransportRuntime` sits above the guarded operator. It owns no database handle,
scheduler, job, checkpoint, lease, cancellation or retry state. It repeats exact-tenant authorization
before opening continuation, derives schema-wide or exact-locale tenant/schema/source scope from the
frozen source registry, calls only `IndexReplayOperatorRuntime::run_shadow`, and seals any outgoing raw
source cursor before returning a transport-safe outcome.

`IndexSourceContinuationScope::from_registry` is schema-wide; `for_locale` binds one exact canonical
`LocaleKey`. The transport constructs matching `IndexReplayDryRunRequest::new` or `for_locale`, and the
dry-run runtime builds each actual source page with matching `IndexSourceScanRequest::new` or
`for_locale`. Exact-locale dry-run fails closed for `LocaleMode::None` before source execution.

Schema-wide and exact-locale tokens cannot cross scopes, and different canonical locales cannot
exchange tokens. The codec retains no old-format decoder or format-version family.

The GraphQL boundary authorizes before custom parsing caller input. Durable Full uses a fixed 100-row
× 8-page chunk, per-page heartbeat and 60-second lease. Shadow uses the same fixed 100 × 8 scan budget
but no worker, heartbeat or lease. Targeted uses the canonical exact-key 1..=256 bound and no page or
continuation budget.

Transport adapters do not call `SharedIndexReplayRuntime` or `SharedIndexReplayDryRunRuntime`
directly. Full/cancel and Targeted go through `IndexReplayOperatorRuntime`; Shadow goes through the
sealed `IndexReplayShadowTransportRuntime`, which itself calls the operator.

## In-page host interruption boundary

`PostgresIndexReplayRunner` has a separate `run_interruptible` path that delegates one host-owned probe
to the existing `IndexReplayWorker::run_next_page_interruptible` safe points.

An interrupted Full page is not marked failed and does not manufacture persisted cancellation. After
preserving any cancellation race, the runner yields the fenced job back to `pending` with lease
ownership cleared and the last committed checkpoint unchanged. A later attempt can replay the same
page; already-durable deliveries remain safe through inbox deduplication and source-version ordering.

`SharedIndexReplayRuntime::run_interruptible` and `IndexReplayOperatorRuntime::run_interruptible`
carry that boolean probe without importing server lifecycle types into the Index crate.

GraphQL schema initialization resolves or atomically creates one `StopHandle` in shared server runtime
state and retains a watch receiver even for API-only hosts. `runIndexReplay` reads only
`StopHandle::is_stopping`; it never calls `StopHandle::stop`, and no shutdown field is accepted from
GraphQL input.

Targeted and Shadow do not use this durable interruption path. Targeted has no resumable durable
pending state, while Shadow resumes only from a sealed continuation returned by a completed bounded
call under the same schema/locale scope.

The retained SQLite runner packet covers Full interruption before source scan and interruption after
one mutation is durable but before checkpoint commit. Actual GraphQL/process-shutdown execution
remains maintainer-run.

## Reconciliation scheduling boundary

The work registration here is reconciliation-only. It discovers due pending or expired-running
reconciliation jobs and delegates claim/takeover to `PostgresIndexReconciliationRunner`.

It does not schedule replay/rebuild jobs, create a second task, own a database lease, or expose a
scheduler handle through replay transports.

## Explicitly open

- execute/admit dedicated Targeted GraphQL and PostgreSQL exact-key behavior;
- execute/admit schema-wide and exact-locale Shadow GraphQL plus continuation-key deployment evidence;
- execute/admit retained replay interruption/restart evidence and end-to-end process-shutdown command evidence;
- durable Full GraphQL command execution/admission evidence and any separately justified HTTP/CLI/admin surfaces;
- automatic replay/rebuild job scheduling;
- retained PostgreSQL replay and reconciliation scheduler execution evidence;
- operator-visible scheduler health and metrics;
- partition replay scope only after a real partition-capable source can filter before pagination.

No additional independent source-only M6 replay boundary is open without executed evidence or a real
partition-capable source contract.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, database scenarios, workflows and CI are
maintainer-run and were not executed by the implementation agent.
