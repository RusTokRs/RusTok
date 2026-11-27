# Server App — Implementation Plan

## GraphQL system diagnostic authority parity (2026-11-27)

`sessionStats` now uses verified Auth application time and the same tenant,
revocation, creation-time, sliding-expiry and 90-day absolute-deadline fences
as Auth session listing. `eventsStatus.iggyMode` reports the active runtime
mode, `inactive` for an initialized Outbox-only runtime, and the deployment
setting only before runtime initialization. Query-shape and mode-selection
regressions were added. Maintainer verification remains pending; compilation,
formatting and tests were not run in this session.

## CORS allow-list parsing fails closed (2026-11-27)

A non-Unicode `RUSTOK_CORS_ALLOWED_ORIGINS` value is treated as an explicitly
empty allow-list rather than as an absent variable, so malformed explicit
configuration cannot silently inherit development loopback origins. Host-only
origins ending in `/` preserve HTTPS normalization when using port 443.
Source regressions were added; compilation, formatting and tests remain pending.

## Bounded post-abort worker joins (2026-11-25)

Runtime and module worker shutdown helpers now use a separate one-second
post-abort join grace. Worker groups still broadcast abort to every remaining
member before joining any of them, then share one deadline across cleanup.
If a task remains unresponsive, its join handle is released after the grace so
shutdown does not await it indefinitely; Tokio cancellation remains cooperative
and cleanup is not guaranteed. Added a regression using an already-started
`spawn_blocking` task to prove the join helper returns when abort cannot stop
the task. Tests and compilation remain with the maintainer.

## Registry direct-session admission (2026-11-25)

Aligned registry controller and middleware admission with the canonical
`AuthPrincipalKind` classifier and non-nil identity requirements. Added
regressions for inconsistent grants, required/optional controller authority,
and rejection before artifact authorization database access. The follow-up
also seals unchecked authority constructors, validates persisted principal
invariants without legacy runtime fallback, and rejects nil owner-transfer
targets. Regression sources cover canonical principal round-trips, malformed
principal rejection, and nil owner targets. Maintainer verification remains pending: `cargo check -p rustok-server`,
`cargo test -p rustok-server --lib registry` and remediation gate.

## Default SuperAdmin bootstrap identity and conflict recovery (2026-11-25)

Source changes align lookup with the canonical normalized account identity and
handle the typed duplicate-email conflict after the creation transaction
ends. Regressions cover normalized existing accounts, exact-identity conflict
recovery and invalid email before tenant provisioning. The conflict regression
models a committed winner; it does not establish PostgreSQL multi-replica
contention behavior. Maintainer verification remains pending:
`cargo check -p rustok-server`, `cargo test -p rustok-server --lib superadmin`,
concurrent PostgreSQL startup verification and the remediation gate.

## External Iggy configuration validation parity (2026-11-25)

Source changes reuse the connector's structural validation before accepting
external settings, and share stored configuration resolution between readiness
and startup. The deployment fallback returns the normalized address list
rather than the unchecked original. Added deployment normalization and
connector-incompatible configuration regressions. Maintainer verification:
`cargo check -p rustok-server`, `cargo test -p rustok-server --lib iggy_connector_settings_service`,
`cargo test -p rustok-iggy-connector --lib`, real broker tests and remediation gate.
No compilation or live broker result is claimed.

## Worker group cancellation ordering follow-up (2026-11-25)

Source changes cancel all remaining group members before joining their
cleanup and resolve the stop signal through one atomic singleton operation.
Added regressions for dependent task cleanup and concurrent stop-handle
resolution. Maintainer verification remains pending:
`cargo check -p rustok-server`, `cargo test -p rustok-server --lib app_lifecycle`,
`cargo test -p rustok-server --lib stop_handle`, and the remediation gate.
No compile or runtime result is claimed.

## Forwarded authority parsing follow-up (2026-11-25)

Source changes reject duplicate RFC Forwarded parameters, preserve quoted
delimiters and avoid direct-host fallback after malformed forwarding. Added
regressions for duplicate/malformed authority, quoted extension delimiters
and duplicate Host fields. Maintainer verification remains pending:
`cargo check -p rustok-server`, `cargo test -p rustok-server --lib request_trust`,
tenant/channel/OAuth boundary checks and the remediation gate. No compilation
or live proxy evidence is claimed.

## Redis rate-limit recovery and acquisition deadlines (2026-11-25)

Source changes bound shared connection-slot contention and repair persistent
counter TTLs before operations that may fail. Added a stalled-initializer
regression and extended the existing isolated Redis regression to cover
integer overflow and wrong-type keys. Maintainer verification remains pending:
`cargo check -p rustok-server`, `cargo test -p rustok-server --lib rate_limit`,
the ignored real-Redis test using `RUSTOK_CACHE_REAL_REDIS_URL`, and the
remediation gate. No compilation or runtime evidence is claimed.

## HTTP observability response integrity follow-up (2026-11-25)

Source changes enforce fail-closed readiness response normalization, preserve
router method status and prohibit caching health/metrics responses. Regressions
cover inconsistent and malformed readiness responses, aggregate metadata
redaction, HEAD/POST behavior and edge cache policy. Maintainer verification:
`cargo check -p rustok-server`, targeted `cargo test -p rustok-server --lib`
for metrics_auth and security_headers, remediation gate and deployed probe/
proxy verification remain pending. No runtime or compile result is claimed.

## Global event configuration persistence follow-up (2026-11-25)

Source changes remove the read-before-insert race in both global settings
tables and reject malformed external Iggy addresses without echoing input.
Regression sources cover concurrent and repeated profile persistence and
endpoint admission. Maintainer verification remains pending:
`cargo check -p rustok-server`, targeted `cargo test -p rustok-server --lib`
for event-delivery and Iggy settings, and the remediation gate. PostgreSQL
concurrent first-save behavior and real broker connectivity remain unverified;
this change does not make Iggy readiness and profile persistence one
cross-table transaction.

### Workflow durable cron admission / outcome review refinement (2026-11-23)

Cron now atomically checks the current firing attempt and locked workflow
definition, then inserts its execution journal in the same transaction before
any external step begins. This closes the operator-requeue race where an older
worker could still insert a journal after a review found none. Long-running
attempts heartbeat only while the same token is current; the engine rechecks
the token before further steps and finalizes its status under a firing-row
write fence so a late worker cannot overwrite operator recovery.

Operator `confirm_external_completed` / `confirm_external_failed` can
resolve the full overall workflow execution outcome only with human
`workflows:manage` authority, bounded evidence and explicit confirmation
that the former worker is stopped. The operator must account for **all**
remaining steps and possible remote side effects, not just one HTTP response.
`confirm_terminal` remains a reconciliation of a previously terminal
journal, while `requeue_confirmed_unstarted` refuses any firing with an
execution journal. All updates and immutable review rows commit together.
This is not provider-level exactly-once. Migrator composition already includes
Workflow-owned migrations through the canonical ModuleMigrationSource.

Source regressions now cover competing claimers, immutable audit, stale token,
unstarted requeue, terminal review, external failure resolution, HTTP
no-blind-retry and schedule revision on step edits. None were run by request.


## Server audit — durable Workflow cron integration (2026-11-23)

`init_workflow_runtime` now verifies Workflow-owned cursor/firing/audit
schema before publishing the scheduler handle and refuses to spawn after a
stop during that async check. The scheduler keeps bounded durable slot
backfill and execution claims inside `rustok-workflow`, not duplicated in
the host. Aged ambiguous firings require explicit review rather than auto
replay; event-triggered workflows also fail closed on uncertain running
execution receipts. Proposed ADR and owner HTTP/GraphQL contracts are
documented in Workflow. All new tests/migrations require maintainer
verification; none have been run.

## Server audit continuation — Workflow cron execution supervision (2026-11-23)

The Workflow cron owner has been changed from detached execution spawns to
scheduler-owned `JoinSet` supervision. On normal host stop it stops admitting
new firings and joins accepted executions. The server composes this via
`start_with_stop` instead of aborting a child scheduler task immediately.
Owned tasks are aborted if the scheduler is forcibly aborted after the host's
existing bounded shutdown deadline. Source regressions cover pre-signaled
stop and waiting for an admitted execution. This does not implement durable
cross-replica cron claims, missed runs or crash recovery; those remain an
owner-level architecture task. No Rust tests/compilation/formatting were run.

## Server audit continuation — producer/dispatcher shutdown ordering (2026-11-23)

The host previously joined Outbox relay shutdown concurrently with module event
dispatcher shutdown. The dispatcher could unsubscribe while the relay was
finishing a committed Outbox delivery and waiting for the local listener
acknowledgement. Shutdown now joins all upstream producers (including the
cooperatively draining relay) first, then requests the dispatcher stop, and
finally drains the event forwarder. This keeps the required listener registry
available for accepted relay work and the forwarder available for event writes
from a finishing dispatcher handler. A focused test holds an in-flight relay
drain and asserts that the dispatcher remains registered until it completes.

Each stage still has the existing bounded shutdown/abort fallback. Process
loss, a deadline exceeded by an external provider, and a rejected local
consumer remain recoverable owner-side delivery outcomes, not exactly-once
guarantees. Regression code is authored, not compiled or executed.

## Server audit continuation — cooperative Outbox relay drain (2026-11-23)

The Outbox relay supervisor previously aborted its inner `JoinSet` on every
graceful stop. A pass can have dispatched an event to an external transport
and still be awaiting its durable status acknowledgement, so aborting the
future at that point manufactures an ambiguous result and unnecessary replay.
The inner poller now watches shutdown before each pass and while sleeping,
then finishes any admitted `OutboxRelayPort::process_pending_once` pass before
returning. The supervisor joins the inner poller on stop rather than aborting
it immediately; unexpected child exits still use the existing bounded restart
supervision. A regression constructs a real SQLite Outbox row and a blocking
transport, requests stop during publication, and verifies that the row is
acknowledged `dispatched` after the transport completes.

This is graceful process-local shutdown, not exactly-once external delivery:
the host's five-second forced-abort deadline, broker ambiguity, crash/restart,
and lease expiry still require owner-side fencing and idempotent consumers.
Test code was authored but not run; no compilation or formatting was run.

## Server audit continuation — durable worker shutdown and dispatcher recovery retry (2026-11-23)

The Product Media release loop previously raced its owner batch against a stop
notification using `tokio::select!`. A graceful stop could cancel the batch
after an external Media release but before the owner confirmed absence and
acknowledged its committed queue row. It now checks stop before admitting a
batch and awaits the already admitted batch to completion. Paid-order label
recovery also rechecks stop after waking on its ticker, before starting an
external provider sweep. Both use one server-local admission helper that never
cancels an operation after it starts. Source regressions cover stop-before-start
and stop-during-operation behavior.

Repeated module dispatcher bootstrap used to return immediately when the
required dispatcher was already healthy, so independently reserved optional
Product Media, paid-order label, financial or payment recovery workers could
never be retried after an initially absent dependency. The already-live path
now revisits those idempotent child starters. A focused `mod-product` regression
verifies a pre-published healthy dispatcher can register its missing Product
Media release worker and remains shutdown-drainable.

The graceful-stop behavior does not guarantee process-crash completion or
override the host's bounded forced-abort deadline. Replay, ownership,
cross-replica fencing and external side-effect reconciliation remain the
durable operation owner's responsibility. All added tests are source-only
and unrun by request; no compilation, formatting or runtime checks were done.

## Server audit continuation — scheduler startup liveness (2026-11-23)

The module-work and Workflow cron scheduler initializers previously treated a stored
`JoinHandle` as healthy on repeat bootstrap even if its task had already
terminated. The module-work initializer now uses the shared worker-liveness
guard to fail closed without deleting the handle needed for shutdown accounting.
The Workflow cron initializer applies the same guard and propagates this startup
failure through application bootstrap instead of returning a false success.
Concurrent healthy initializers still reuse the running handle, and host stop
continues to prevent late task publication under each initialization lock.

Focused source regressions create already-finished tasks for both paths and assert
that repeat initialization returns an error while preserving shutdown ownership.
They have not been executed; Cargo, Rust tests, compilation and formatting are
reserved for maintainer verification. This change does not solve cron execution
deduplication across replicas, missed schedules after process crashes, or detached
execution-task supervision; those require the Workflow owner's durable contract.


## Code-review corrections — shutdown admission and cooperative listeners (2026-11-22)

A second adversarial review found that Notification Candidate and Notification
Fanout could durably defer work after a tenant-policy lookup returned during
shutdown. Both paths now check the host stop signal before the deferral and
again before processing/materialization. Fanout also stops before beginning
the second work-selection query after its source pass.

Artifact maintenance now checks shutdown after loading blob-retention policy
and before destructive blob reconciliation, and after tenant discovery before
loading the next retention snapshot. This closes the preparation/effect
boundary where a shutdown arriving during a read could still admit a new GC
operation.

The cache Redis status monitor, tenant cache-generation listeners, tenant
locale-generation listeners, and RBAC durable-generation watchdog now wrap
their cancellation-safe background loops in a cooperative stop gate. They
exit promptly instead of remaining alive until the generic worker abort
timeout. The shared `run_until_stopped` helper is intentionally limited to
cache invalidation, status observation, and reconciliation work whose
interruption is recovered by a full generation sync on the next startup; it
must not wrap a claimed durable operation or external side effect that needs
completion/compensation.

RBAC pre-migration diagnostics now distinguish a missing
`rbac_invalidation_state` table from unrelated missing relations and missing
columns, including SQLite, PostgreSQL, and MySQL diagnostics.

## Server audit continuation — RBAC watchdog diagnosis and bounded module recovery admission (2026-11-22)

The RBAC durable-generation watchdog previously classified any SQLite
`no such table` or Postgres `undefinedTable` error as a permissible
pre-install condition while the local RBAC generation checkpoint was
uninitialized. Unrelated missing tables could therefore hide an actual
schema fault. The watchdog now suppresses only missing-relation errors
that explicitly name `rbac_invalidation_state`; fixture coverage rejects
unrelated SQLite, Postgres and generic undefined-table diagnostics.
Its startup admission/old-exited-task join is also stop-aware, using
the same pre-publication cancellation helper as cache listeners. A
blocked competing starter now returns on host stop without spawning
another watchdog; regression tests cover that contention and a
previously signaled stop.

Two recovery controllers, `module_transition_watchdog` and
`module_artifact_maintenance`, checked shutdown before starting an
entire tick, then ran multiple independent durable operations across
await points. A shutdown arriving mid-pass therefore still permitted
retention hold reaping, transition evaluation, additional tenant GC
sweeps or trust-admission rechecks to begin. They now use
`may_admit_next_worker_operation` **between** separate effects and
before each next tenant/candidate. They never cancel an already
entered database mutation, storage deletion, or external trust
reverification merely to honor the stop signal. Not-yet-started work
remains available to the next supervised instance.

These changes do not override the existing bounded host join/abort
timeout or guarantee completion of an effect that independently
blocks longer than its shutdown deadline. Tests, compilation, and
formatting were not run.


## Server audit continuation — cache listener startup cancellation on shutdown (2026-11-22)

Five cache/control-plane listeners serialized startup under their own
per-runtime mutexes but performed potentially slow asynchronous setup while
holding those locks. Channel and RBAC invalidation, tenant-locale and
tenant-cache generation could wait on startup reconciliation or abandoned
workers; Redis status monitoring could wait on its initial Redis status
read. Shutdown's corresponding handle-take paths only waited five seconds
for the start locks before returning without task handles. A slow direct
initializer could therefore outlive the shutdown drain, even if it would
eventually observe the stop flag before publishing its worker.

The `stop_handle` owner now exposes `await_worker_startup_or_stop` for
**unpublished/cancellable** initialization. The helper subscribes to the
monotonic stop watch before inspecting its value, preventing a lost
notification between precheck and subscription. It favors stop if both
the setup result and stop are ready, and refuses to report setup success
after stop becomes active. Channel, RBAC, locale, tenant generation and
Redis-status starters use it while acquiring their startup lock,
awaiting a previously aborted worker's join, and performing initial
cache/readiness recovery. An interrupted setup drops its outstanding
preparation future and releases the startup mutex; since completed
worker publication is still synchronous under that mutex, the ordinary
shutdown take/abort/join retains published task ownership.

This helper MUST NOT be used for an already-claimed durable operation or
a side effect with incomplete compensation; it is limited to cache
bootstrap/reconciliation and prior aborted-task joins. It does not
eliminate the five-second shutdown fallback for uncooperative blocking
code, and process abort still provides no cleanup guarantee. Source
regressions cover a never-completing setup future, an occupied
registration mutex, and a stop signaled before initialization.
Tests, builds and formatting were not run by the agent.


## Server audit continuation — Comments direct-worker shutdown registration (2026-11-22)

The opt-in Comments TCP listener already had one startup reservation and
checked shutdown after its asynchronous socket bind. The Comments durable
schedule-audit source-retry handoff similarly reserved a unique worker and
checked stop after constructing its durable dependencies. However, neither
path synchronized the final stop-check → `tokio::spawn` → published handle
sequence with `shutdown_runtime_workers` taking those handles. A direct
starter could publish a task after shutdown had already taken the old
value, detaching the task from its intended join/abort owner.

Both Comments starters now acquire the existing process-local
`DirectWorkerRegistrationLock` for only the final admission/spawn/handle
publication, and shutdown takes both their handles under that same gate.
The TCP socket bind occurs before acquiring the gate, so a slow bind cannot
block shutdown. Once the stop flag is broadcast, a later starter refuses
publication under the gate. Shutdown still removes the separate TCP listener
address metadata after taking its task, and the existing per-worker startup
reservations continue to prevent duplicate starts. No change to persistent
handoff claim leases or delivery semantics.

Existing source-level tests for shutdown admission and terminated-join
handles remain; the direct-registration gate contention regression also
covers the shared registration primitive. A Comments-specific regression now
holds the publication gate, asserts shutdown cannot take either task while
that gate is held, publishes the TCP listener and durable handoff handles,
then confirms both are taken and joined/aborted without a detached owner.
Runtime/broker/SQL tests and formatting/build verification remain
maintainer-owned and were not run.


## Server audit continuation — direct worker registration and shutdown (2026-11-22)

Several public worker starters can run outside the router's
`ServerBootstrapInitializationLock`. The Product Search channel projection
reconciler used a non-atomic `contains → spawn → insert` sequence, so
competing callers could overwrite a live JoinHandle, and shutdown could
take its snapshot between the check and publication. Its repeat-start path
also considered a finished handle to be healthy. Notifications outbox intake,
candidate and fanout, Forum Search inbox and contract consumers, Product
Index refresh, and Social Graph Index workers already reserved competing
startups, but each still left a final `is_stopping → spawn → shared_insert`
window unprotected from the corresponding shutdown `shared_take`.

The runtime now has a shared, process-local `DirectWorkerRegistrationLock`
for these eight direct workers. Their synchronous final admission/spawn/handle
publication is protected by the same lock as each shutdown handle take.
Blocking mutex guards are never held across async preparation, receipt
backfill, broker opening, task joins, or other awaits. Shutdown broadcasts
stop before its drain; a starter waiting for the lock checks the stop flag
*under* the lock and cannot publish after its handle was taken. Product
Search channel repair uses the same lock for its whole synchronous starter,
so concurrent starts have one owner and terminated tasks fail bootstrap
rather than producing false success.

Source tests cover contention between a held publication gate and a shutdown
snapshot, plus Product Search shutdown refusal and terminated-handle
reporting. Tests, compilation, formatting and runtime failure injection
were not run; verification remains with the maintainer. This process-local
publication lock does not replace the durable broker/DB recovery contracts.


## Server audit continuation — ambiguous pre-execution installer HTTP claim (2026-11-22)

A committed HTTP `install_http_jobs` INSERT may have lost its DB response.
The server retries/reconciles its fixed job ID, but after exhausting the retry
budget it dropped its local admission without trying to settle a matching
`running` row; an installer executor that never ran could then leave the
unique active-job fence blocking subsequent installations indefinitely.

The persistence owner now exposes an exact `job_id`/`session_id`/
`submitted_at` + `status = running` compare-and-set for use only after the
host has definitively **not entered installer execution**. When claim
creation exhausts its retries, the runtime-owned task attempts bounded
cleanup before releasing its local reservation. Lost update acknowledgements
are reconciled by reading the same job; missing rows require no cleanup,
while mismatched identity and surprising terminal status fail closed. The
HTTP caller retains its existing failed-creation error mapping. This does
not release an installation that may have entered external side effects.

Regressions assert exact-identity state change, unrelated-submission rejection,
terminal immutability, and host recovery of a previously committed-but-
unacknowledged claim. The tests and build/format commands were not run by
the agent; failure injection against PostgreSQL/SQLite and worker termination
remains with the maintainer.

## Server audit continuation — module dispatcher shutdown ownership (2026-11-22)

The host startup reservation prevented two module dispatchers from
starting, but not a different race: `spawn_module_event_dispatcher`
could check the global stop signal, then lose a scheduling race
against `shutdown_runtime_workers` taking
`ModuleEventDispatcherHandle`. The direct startup API can be called
outside `ServerBootstrapInitializationLock`, and shutdown previously
took the dispatcher handle without an additional registration gate.
If the direct caller then spawned and published after the snapshot,
the dispatcher task was left running without a shutdown join owner.

A new process-local `ModuleEventDispatcherRegistrationLock` guards
the final check-stop → spawn → publish sequence and the shutdown
snapshot. The same gate now covers the four child recovery worker
starters that follow successful dispatcher startup: paid-order labels,
marketplace financial, payment-provider events and product-media release.
Shutdown obtains their handles under the matching gate. The stop check
for every worker now occurs inside it, so shutdown either joins a
published worker or a late starter is rejected. The product-media
starter also adopts a drop-safe typed startup reservation instead of
keeping a permanent marker after a failed/abandoned attempt. Existing
startup reservations still exclude simultaneous publishers. The lock
is released before any handle's graceful drain, preventing a teardown
deadlock.

Regression scenarios cover a held registration gate during
concurrent shutdown and a stop signal arriving while a direct
dispatcher startup waits for the gate. Additional scenarios exercise
generic child-worker handle take and a payment recovery worker
waiting for the gate during shutdown. These tests are authored
but unrun.
No build, formatting or runtime verification was executed.

## Server audit continuation — EventBus transport registration race (2026-11-21)

The server had two transport registration paths using
`ServerRuntimeContext::shared_insert_if_absent`. The EventBus forwarder
handled a losing insert by fetching the registered winner (with a
fallback), but `transactional_event_bus_from_context` ignored the
boolean and returned its locally created `OutboxTransport` even after
a concurrent publisher had installed another transport. This can split
transactional writes from the runtime transport selected by the
forwarder, bypassing configured delivery behavior for one caller.

Both paths now use the already-present
`shared_get_or_insert_with::<Arc<dyn EventTransport>>` atomic
singleton contract. It returns the exact published transport regardless
of whether that caller created it. The event bus lifecycle/stop lock is
unchanged, as are existing transaction writer contracts. New regression
tests cover racing resolver calls, transactional construction and
preinstalled transport ownership. Tests/build/format commands not run.

## Server audit continuation — tenant HTTP error redaction (2026-11-21)

`TenantResolutionError::error_message` previously returned
`to_string()` for every non-policy error. Its Display includes the
untrusted request Host, asserted tenant slug, configured domain names,
header names and arbitrary validator reason strings. A malformed or
oversized request could cause the API's error envelope to echo those
identifiers verbatim, despite the nearby `TenantContextLoadError`
already returning only stable public descriptions.

Public tenant resolution errors now use static bounded messages while
retaining all existing status codes, machine-readable error codes and
typed internal error structure. This does not change which requests
are admitted, nor the specific validation rules. A regression exercises
all data-bearing variants with an overlong credential-like string and
requires the actual serialized HTTP response to remain short and
redacted. Tests, builds and formatting were not run.

## Server audit continuation — middleware raw-path diagnostic closure (2026-11-21)

The previous path-label audit fixed the root HTTP trace span, metrics-auth
and rate-limit diagnostics, but a follow-up traversal of the middleware
directory found seven more request entry points logging the original URI
pathname: `auth_context`, `tenant`, `channel`, `locale`,
`security_headers`, `registry_artifact_access`, and
`registry_publish_policy`. These execute within the edge span and
could still record unknown dynamic URL segments, account identifiers or
credential-like path material on both success and failure.

All those diagnostic sites now use the existing fixed
`trace_path_family` rather than retaining separate sanitization
policies. Registry log entries also bucket extension HTTP methods with
the fixed `trace_method_family`. The original path and HTTP method are
preserved for routing, authentication, CSP selection, and authorization.
Existing middleware behavior tests plus the shared family helper's
focused regression remain available to the maintainer; none were run.

## Server audit continuation — bounded HTTP trace identifiers (2026-11-21)

The edge `RusTokMakeSpan` already used bounded `path_group` after
earlier audit changes, but it still copied arbitrary
`x-request-id`, `x-correlation-id`, `traceparent` headers and
custom HTTP method names into the root span as-is. Those fields are
client-controlled and could be unbounded or contain credential-like
strings even when no route matches.

The trace-span boundary now accepts only canonical UUIDs or
32-character hex trace IDs for request/correlation identifiers and
strict W3C version-00 traceparent syntax with nonzero trace/span IDs.
Missing, malformed and overlong values map to `-`, and extension
HTTP methods map to `OTHER`. The validated fields retain ordinary
correlation for UUID-based callers. No HTTP headers are rewritten;
downstream request/response ID propagation remains unchanged.
The focused unit test exercises accepted and rejected values and
the fixed method vocabulary. Tests, build and formatting not run.

## Server audit continuation — tenant cache-generation recovery semantics (2026-11-21)

The tenant cache-generation listener already reconciled the shared backend
generation on startup, subscription readiness, Pub/Sub gaps and a 30-second
watchdog. However, its `recover_shared_generation` updated the tracker and
backend generation without clearing the independent
`ModuleEffectivePolicyCache`. Missing a tenant or platform invalidation
therefore left cached effective authorizations in other tenants until TTL
expiry, even as reconciliation reported a recovered durable offset. The
ordinary in-order listener also acknowledged its tracker *before* clearing
the policy and shared backend caches.

The listener now holds one shared async transition gate across local bus,
Redis handler and periodic recovery. A changed/unverified durable generation
clears **all** effective policies and shared backends before advancing the
recovered checkpoint; unchanged generations leave fresh caches intact.
In-order events apply their local invalidations before acknowledging.
Unrecognized bounded event keys cause a conservative full policy clear,
never an acknowledgement with no policy effect. The canonical `*`
manual-global key is explicitly recognized; nil tenant UUIDs instead
fail over to the conservative global clear. The inner recovery helper
avoids recursively acquiring the gate on a gap.

New regressions cover missed generations, subsequent unrelated-tenant
cache eviction on a gap, no-op unchanged reconciliation, invalid keys,
and shared-lock ownership across listener clones. The full effect is
process-local and does not claim distributed exactly-once delivery.
No tests/build/format commands were run.

## Server audit continuation — tenant locale generation transition ownership (2026-11-21)

The tenant locale-generation listener is cloned into three independent
local, Redis and periodic reconciliation task owners. All three previously
ran asynchronous `current_generation`, cache invalidation and tracker
acknowledgement concurrently, then separately set a single readiness flag.
The bounded tracker protected individual map operations but did not protect
the whole observe → invalidate → acknowledge → health sequence. An older
in-flight recovery could therefore report readiness after a newer failure,
or classify an event against a checkpoint that was being advanced by a
different task.

A shared `Arc<tokio::sync::Mutex<()>>` now serializes both
`recover_if_advanced` and `handle_message` across every cloned listener.
These entrypoints preserve the existing durable checks, full-cache clear
fallback, failure reporting and retry behavior. The gate is held through
the final readiness update; internal helpers do not recursively acquire it.
A focused Tokio regression verifies both entrypoints wait for a lock
held by another clone. This does not provide multi-replica serialization.
No tests/builds/formatting were executed.

## Server audit continuation — registry publish middleware parity (2026-11-21)

The outer `registry_remote_claim::claim_atomic` middleware delegated only
the exact `POST /v2/catalog/publish` pathname to the host publication
metadata policy; the owning `registry_publish_policy::is_publish_path`
explicitly accepted trailing slash variants. A routed alias could therefore
reach the registry handler without the same ownership, trust-level and
session-authority checks. Both boundaries now share the policy's canonical
path recognizer; a focused regression covers exact and trailing slash
variants and rejects non-publish/mismatched methods.

The metadata policy also previously rewrote the JSON request body while
retaining the incoming `Transfer-Encoding` header. The normalized-body
constructor now drops both stale `Content-Length` and
`Transfer-Encoding`, matching the already-correct MCP body-normalization
pattern. A regression checks that unrelated headers survive.
Source/runtime tests, builds and formatting remain maintainer-run.

## Server audit continuation — production CORS authorization (2026-11-21)

Production CORS previously inserted `http://127.0.0.1:5150` whenever
the allow-list was absent or unusable, even though credentialed
cross-origin access was enabled. The `RUSTOK_HOST`/`APP_HOST` fallback
could also silently treat a production bind address (such as
`0.0.0.0:5150`) as trusted browser-origin configuration. A local page
could therefore be granted a browser CORS permission which no operator
explicitly approved.

`build_cors_layer` now leaves the production allow-list empty without
valid explicit origins; `resolve_cors_allowed_origins_from_env` applies
bind-host fallback in development only. Configured entries are parsed
as actual HTTP(S) origins; embedded URL credentials, paths, queries,
fragments and other schemes are rejected instead of being treated as
arbitrary header strings. Wildcard listener addresses (`0.0.0.0`,
`[::]`) are also rejected as explicit browser origins, rather than
being normalized into localhost authorization. Valid origins are canonicalized before being
passed to `CorsLayer`. This does not reject same-origin HTTP traffic
and does not substitute for normal authentication. Focused regression
tests cover missing/invalid origins, loopback, credentials and
production-versus-development host fallback. Unrun per maintainer request.

## Server audit continuation — bounded HTTP path diagnostics (2026-11-21)

The edge `RusTokMakeSpan` traced `request.uri().path()` verbatim, and
metrics-auth and both rate-limit middleware layers repeated this user input
in debug diagnostics. A path can contain bearer-like reset links, account
identifiers or arbitrary content even for unmatched routes; the tracing
fields were not subject to the existing bounded Prometheus route-label
policy.

The shared `trace_path_family` helper in `middleware/http_stack.rs`
exposes only fixed route families at the pre-routing boundary: exact public
health/metrics/GraphQL entry points, generic API/admin buckets and a generic
fallback. The edge span and middleware diagnostics use this bounded field.
The original path is still used for policy matching and Axum route dispatch,
while Prometheus continues to label matched route templates after routing.
Unit regressions assert secrets in nested paths cannot become trace path
fields, including lookalike `/apiary` prefixes. No runtime checks executed.

## Server audit continuation — poison receipt SQL fallback type fix (2026-11-21)

A review of the previous ambiguous DLQ publication patch found a Rust type
error in `ConsumerPoisonReceiptStore::reserve_and_claim_in_transaction`.
After a lost concurrent claim, the fallback decoded a `QueryResult` into
a public `ConsumerPoisonReceipt` and then called
`publishing_claim_expired(&current)` on the decoded receipt. That helper
expects a raw `QueryResult` containing the SQL-only expiry alias.

The fallback now reads the expiry alias before decoding and branches on the
saved boolean, keeping the public neutral receipt free of publisher/lease
metadata. Updated the existing source verifier to enforce the ordering.
No test, compiler or formatting commands were executed.

## Server audit continuation — global DLQ observer lifecycle (2026-11-21)

The physical Event DLQ duplicate-alert observer and its telemetry companion
were separate check-then-spawn paths: both called
`shared_contains::<...Handle>()` before unconditionally publishing a
`shared_insert`, without fencing shutdown. Concurrent direct invocations
could orphan one live Tokio task or overwrite the handle, and a late caller
could publish after shutdown had snapshotted workers. Repeated startup also
silently treated a retained but exited handle as an active observer.

The two starters now share an `EventDlqObserverInitializationLock`
(`std::sync::Mutex`; neither startup function suspends while holding it).
Shutdown takes the same lock after signaling stop and before taking its
handle snapshot, preserving the bootstrap → runtime-workers → observer lock
order. After acquisition, starters reject already-stopping hosts and check
whether any retained task finished. An exited task remains registered for
shutdown and emits a stable error diagnostic instead of being overwritten.
Static no-task modes remain idempotent, and startup errors remain
non-blocking for event delivery. Focused tests exercise completed handles
and post-stop startup for each companion.

No tests, compiler checks or rustfmt were executed; the maintainer owns
verification and runtime contention evidence.

## Server audit continuation — Social Graph observer lifecycle (2026-11-21)

Two independent Social Graph Index operational observers (consumer position and
neutral poison-receipt aggregate) used a check-then-spawn `shared_contains`
check followed by an unconditional `shared_insert`. Concurrent external
starters could each spawn a task and overwrite the other's shutdown handle.
Repeated startup also returned success for a completed observer task, leaving
health/metrics stale. Neither observer fenced startup during host shutdown.

Both paths now reuse typed `ServerRuntimeStartReservation` and
`published_worker_running` with initial, contended and post-reservation
checks, then verify the common `StopHandle` before publishing and committing
the reservation. A shared `SocialGraphObserverInitializationLock` serializes
both direct observer starters with shutdown's handle snapshot. The startup
lock order remains bootstrap → runtime worker initialization → observer;
an observer never acquires an outer lock while owning its registration lock. A failed/invalid optional poll configuration releases the
reservation; no broker offset or durable receipt is mutated.
Unit regressions cover exited Tokio tasks and competing startup reservations.

Also corrected the Social Graph raw-poison publication diagnostic unit test
to match the previous fail-closed change: an ambiguous external send is
`reconciliation required`, not a proven failed send.

These tests were authored but not executed; no compilation, formatting or
integration runs were performed per maintainer instruction.

## Server audit continuation — ambiguous poison DLQ results (2026-11-21)

Confirmed in `ConsumerPoisonReceiptStore` and the Forum Search/Social Graph
raw-poison consumers: `claim_receipt_sql` reclaimed expired `publishing`
receipts based only on time, while both consumers released the claim and
reissued the same external Iggy send after a transport error. A broker send
may have succeeded despite a missing response; lease expiry/release cannot
prove non-delivery. This violates the durable-effect recovery decision.

The connector now claims only `reserved` receipts; a still-active
`publishing` receipt returns `Busy`, and an expired one reports
`ReconciliationRequired` without updating its existing immutable identity,
first-error diagnostics or publisher token. The original owner can still
settle `published` after its lease expiry. The two consumers no longer
release a `publishing` receipt or perform an automatic second broker send
after an ambiguous send failure. The source offset remains uncommitted.

Regression coverage includes SQLite expiry and PostgreSQL late-publisher
reconciliation, and the existing retained execution/evidence contract and
static guards were updated to assert this safer invariant. Runtime tests
and format/compile checks remain unexecuted per maintainer instruction.

**Limit:** This deliberately fails closed until an effect owner/operator
verifies whether the external send committed. Neither generic lease expiry
nor a deterministic Iggy message ID is enough to authorize replay. An
authenticated operator reconciliation workflow with auditable evidence is
still a separate required owner capability, not implemented in this patch.

## Server audit continuation — post-receive broker shutdown fence (2026-11-21)

Forum Search typed contract consumer, Product Index refresh and Social Graph
Index consumer previously awaited broker delivery and shutdown in the same
`tokio::select!`. When both futures were ready, Tokio could choose the receive
branch even after the stop signal was already visible. The worker then started
projection or raw-poison DLQ publication instead of draining. The durable
broker cursor is not acknowledged by receive, so a returned-but-unprocessed
delivery should remain recoverable after process restart.

All three loops now perform a second stop check immediately after receiving,
before *any* event or decode-failure handling. A shared helper in
`stop_handle.rs` documents operation admission and has a deterministic
post-await/stop-signal regression. The existing stop branches and durable
acknowledgement contracts are unchanged.

The new checks cannot guarantee that shutdown and effect execution are a
cross-thread atomic operation; effect owners remain responsible for durable
replay, attempt fencing and idempotency. Tests, cargo compilation and
formatting were not run per maintainer request.

## Server audit continuation — exhausted comments handoff recovery throughput (2026-11-22)

Following merged PR #4772, the Comments source-retry worker still finalized only
one expired, exhausted source claim per polling cycle, despite accepting up to
256 new publication claims per cycle. Under a large post-crash backlog this
serialized dead-letter recovery to the idle poll interval, delaying the
operator's terminal evidence without improving transaction safety.

The host now processes up to `max_claims_per_cycle` expired final attempts per
cycle, using the existing exact PostgreSQL SKIP LOCKED/expiry-fenced owner
transition for each row. The dead-letter sweep has a separate bounded budget
from publication claims, avoiding starvation of ready source events. The shared
shutdown signal is checked before every next transition, but an already
started database update is awaited. A full sweep uses the short active-cycle
delay; storage errors retain the configured retry delay. Source regressions
cover per-item shutdown/budget admission and non-idle continuation after a
full sweep; no SQL, Cargo, formatting or runtime tests were run by the agent.

The independently exposed `record_failure` source transition also omitted
an active-lease expiry predicate: matching a stale claim token and attempt
was not sufficient evidence that its lease was still authoritative. It now
requires `handoff_claim_expires_at > NOW()` like the runner-specific
`record_active_failure` transition. The source regression asserts the SQL
expiry fence; both paths remain fail-closed if the claim has expired.

This is not automatic redelivery of ambiguous external effects. Terminal
publication and source attempt receipts remain owner-authored and require the
existing PostgreSQL transaction/fencing contract.

## Server audit continuation — retry/shutdown operation admission (2026-11-21)

Following the startup-liveness fixes in draft PR #4777, inspected the Comments
schedule audit source-retry handoff and Notifications candidate/fanout worker
operation sequences. The host already checked stop at the start of the worker
cycle and batch item, but async intermediate calls could outlive that check:

- Comments source-retry handoff could claim/publish up to 256 successive
  canonical audit items in one cycle even after stop was signaled during
  the first durable publish or failure transition.
- Notification candidate/fanout workers could finish an async effective
  policy lookup (or policy deferral) after stop and still begin a new
  candidate, source materialization, or fanout job operation.

The Comments cycle now accepts a shared stop receiver and checks it before
the initial dead-letter sweep and every new claim. Existing claimed work is
settled before checking again. Notifications now check stop at the
**post-policy admission boundary** before the next effectful owner call.
A focused deterministic Comments regression tests the per-claim admission
predicate across a stop during an in-flight operation.

These edits do not cancel a database transaction already started, repair
cross-process crash side effects, or change owner lease/receipt semantics.
The maintainer has not run tests, compilation, rustfmt, or runtime contention
checks for this audit batch.

## Server audit continuation — exited worker startup (2026-11-21)

PR #4772 was merged into `main` at
`356bffa3c1703b47d8edcaac86547bc120922295`. Its original branch is
closed, so this follow-up uses `agent/server-recovery-startup-health-20261121`
from that merge commit, rather than appending to an already merged PR.

Thirteen host-owned durable worker starters returned success when they found a
published handle, even when the underlying task had already exited. The same
false-positive existed in the contested-reservation and post-claim checks of
the seven individually claimed consumers; six other host workers use the
serialized runtime initializer.
That can make later bootstrap callers believe an inactive consumer was started,
while its durable work is still waiting for recovery. A shared runtime helper
now accepts only a live published handle, rejects a finished handle without
replacing or orphaning it, and leaves absence available to an exclusive
startup claim. A source regression tests absent, live, and finished states.

Scope: Notifications intake/candidate/fanout; Forum Search inbox/contract
consumer; Product Index refresh; Social Graph Index consumer; host Outbox
relay/retention, Auth retention, remote executor reaper, SEO bulk worker and
module transition watchdog. Owner-side
receipt/idempotency handling and external recovery are unchanged. Runtime
process restart remains necessary after an unexpected worker exit; automatic
in-process replacement is not implied. No Cargo tests, compilation, rustfmt,
or live concurrency checks were run, as requested by the maintainer.

## Server audit continuation — dispatcher readiness and cancellation (2026-11-21)

One accumulating draft PR: `agent/server-audit-continuation-20261121`,
based on current main `b4ec15e92e8c328bd7ba32ed50347911cbbfac89`,
after consolidated server fix PR #4771 merged as
`ec4914cac06b19fb352e429c39e4ce044f81774a`.

### Confirmed start-state and cleanup defects

- `spawn_module_event_dispatcher` previously returned `Ok(())` when it
  observed `ModuleEventDispatcherReservation` even if no worker handle
  existed. The first caller could still fail or panic, so external callers
  not using the host bootstrap mutex could treat an unstarted listener
  registry as successfully initialized.
- A published dispatcher handle was treated as success even while the
  required listener flag had not yet been stored, or after the worker
  terminated unexpectedly. Readiness must check both a live loop and a
  required-listener flag rather than the presence of a typed value alone.
- The dispatcher and three optional recovery worker starters manually
  removed their reservation only in selected error branches. A panic while
  constructing a dispatcher or a dependent worker could strand the
  reservation and prevent subsequent retry in the same runtime.
- `connect_runtime_workers_with_runtime` is also exposed directly, outside
  router bootstrap's `ServerBootstrapInitializationLock`. It holds an
  independent artifact-maintenance initialization mutex while spawning other
  host workers. Shutdown previously awaited only router-bootstrap admission
  before taking its handle snapshot, so a direct initializer already inside
  that independent mutex could publish a fresh handle *after* shutdown had
  taken its corresponding slot. The newly spawned task would then have no
  remaining host-owned `JoinHandle` to drain.

### Fix and tests

- Introduced `claim_module_dispatcher_start`, which returns idempotent
  success only for an already-published *running* dispatcher with a true
  `ModuleEventDispatcherRequirement`. A competing in-flight startup
  instead returns an explicit retryable initialization error. A finished
  registered worker is not reported as ready.
- Reused `ServerRuntimeStartReservation` for dispatcher startup, paid
  order label, marketplace financial and payment provider recovery.
  `Drop` releases a claim on ordinary error, dependency unavailability,
  or unwind; successful publication uses `commit()` to retain ownership.
- Added focused source tests proving contention is not readiness,
  cancellation/error and panic release a claim, and a handle without its
  required-listener flag is rejected. Existing startup tests cover
  missing EventRuntime and module listener registration.
- Shutdown now waits for any direct runtime-worker initializer under the
  existing `ModuleArtifactMaintenanceInitializationLock` after signaling
  `StopHandle` and taking the router-bootstrap mutex. Both initializers
  observe consistent lock ordering. The lock is released before `tokio::join!`
  so artifact-maintenance drain can reacquire it without deadlock. A
  regression simulates a direct initializer that registers an auth retention
  task *after* the stop signal and verifies shutdown still removes/drains
  the resulting handle after the initializer releases its mutex.

**Validation handoff:** The user owns tests. No Cargo checks, tests, formatting,
linters or live concurrency exercises have been run in this continuation.
Recommended: `cargo check -p rustok-server --lib`,
`cargo test -p rustok-server --lib services::module_event_dispatcher`, and
`cargo test -p rustok-server --lib services::app_lifecycle::tests`,
plus a multithreaded double-start/shutdown exercise. These guarantees
remain per-runtime; the patch does not implement process restart,
distributed leader election or broker exactly-once delivery.

## Installer HTTP admission cancellation follow-up (2026-11-21)

Branch: `agent/server-installer-admission-cancellation-20261121`, based on
`a9431fdefd08693eab7aa0aaab87169b877af239`.

The HTTP installer previously inserted a process-local `ActiveInstallerJobHandle`
then awaited `create_http_job` in the cancelable HTTP request future before
spawning its execution task. A dropped request during that await could leak the
local admission slot; if the database committed the `running` row before
cancellation, no executor task would run and the unique running-job fence would
block all later installation attempts.

- Request validation and all fallible synchronous preparation now precede the
  local reservation. After reservation there is **no await** before spawning.
- A single background task owns both the durable claim and the installer
  executor; the HTTP handler observes claim success/failure through a oneshot
  channel. Dropping the HTTP receiver cannot cancel either step. The guard
  releases the process-local slot when claim creation fails or the task exits.
- Durable unique-constraint losers still return HTTP 409; other database errors
  return redacted HTTP 500; a stopping host rejects new apply requests with 503.
- If shutdown is observed after the job claim commits but before executor
  entry, its background owner marks the still-unstarted job `failed` with the
  existing redacted error mapping, rather than launching new side effects.
- Regression tests cover early receiver cancellation, claim failure cleanup,
  shutdown after a durable claim but before execution, and stop-signal admission
  refusal.
- **No compilation, formatting, unit/integration tests, or migrations were run**;
  the maintainer will verify them. Check request disconnects during claim
  persistence, concurrent replica requests, and shutdown during apply.

Limit: this closes the HTTP cancellation gap **only**. A worker-process crash,
panicked claim after an ambiguous database commit, or hard runtime abort can
still leave a `running` row. Do not clear stale rows by age or restart another
installer based on an expired lock. Safe crash reconciliation needs durable
owner/attempt fencing, receipt inspection and validation of external effects.

## OAuth secret rotation lock and concurrent-success fence (2026-11-21)

Branch: `agent/server-oauth-secret-lock-boundary-20261121`, forked from
`a9431fdefd08693eab7aa0aaab87169b877af239` as a bounded auth-service
follow-up independent of the pending installer and auth-email PRs.

- Previously the OAuth administration provider acquired a database transaction
  and exclusive OAuth app lock, then generated two random tokens and awaited
  Argon2 hashing while holding that lock. This needlessly serialized token
  issuance, revocation and other app mutations behind expensive CPU work.
- Rotation now first reads the tenant-scoped app and checks its permissions and
  `can_rotate_secret()` state, then prepares the new plaintext and Argon2 hash
  **before** beginning a transaction. The commit stage acquires the existing
  backend-specific app mutation fence, rechecks every authoritative condition,
  and writes the secret hash in the same database transaction.
- A prepared rotation records the previous hash and rejects a commit when the
  current persisted hash changed during preparation (typed conflict, not a
  generic validation error). Two competing rotations
  against the same old secret can no longer both report successful issuance of
  their own new secret: the stale candidate must retry.
- A SQLite regression test prepares a rotation, commits a competing hash
  update, and verifies the stale rotation is rejected without overwriting the
  winner. The existing end-to-end guarded rotation test continues to cover
  tenant, delegated permissions and successful secret validation.
- **No cargo check, cargo test, formatter, lint, migrations or multi-replica
  verification were run**, as requested by the maintainer. Check PostgreSQL
  concurrent rotations against token issuance and revocation in deployment.

Limitations: the changed-hash comparison is a conflict fence only for the
preparation window. A deliberately **sequential** second rotation remains
permitted; secret rotation does not automatically revoke existing OAuth token
families. Database errors and failed commits remain redacted to callers.

## Module event dispatcher and recovery worker single-owner follow-up (2026-11-21)

Branch: `agent/server-module-worker-single-owner-20261121`, based on main
`a9431fdefd08693eab7aa0aaab87169b877af239`. This is a bounded
continuation of the server runtime audit, separate from pending auth-email,
installer and OAuth rotation PRs.

- The paid-order label, marketplace-financial and payment-provider event
  recovery workers previously checked for a stored handle and then spawned
  their own Tokio task before publishing a handle. Parallel composition could
  pass both checks and overwrite a handle, leaving the earlier task running
  without a shutdown owner. They now atomically reserve a distinct per-runtime
  worker key before spawning. Missing optional provider/runtime prerequisites
  release their reservation so later bootstrap retries may succeed.
- The module event dispatcher also previously used a non-atomic handle check.
  Its startup now reserves one dispatcher owner before assembling runtime
  extensions or starting child workers. A failed dispatcher launch releases
  its reservation and does not mark the required listener registry healthy.
- The dependent Product media, Commerce paid-label, marketplace-financial and
  Payment recovery workers start only after a successful dispatcher start.
  Their individual gates reject new starts after the process stop signal;
  Product media startup now also checks its existing handle before reserving.
- Tests cover concurrent Payment worker composition and stable handle identity,
  release on missing Fulfillment/financial runtime dependency, and refusal of
  new recovery work after shutdown. Existing dispatcher startup tests remain.
- **No formatting, build, Cargo tests, lints, migrations or multi-replica
  execution occurred here.** Run focused server lifecycle tests for each
  enabled feature and full/workflow host modes before merge.

Remaining risks: these are per-process reservations, not distributed worker
leases. External payment/shipping effects must remain idempotent and durable.
A process crash still requires replay/recovery from owner receipts. The
bootstrap lock serializes normal host initialization with shutdown, but callers
bypassing it should not treat this reservation as a universal host lifecycle
contract; short spawn/handle-publication windows require normal host ownership.

## OAuth app mutation and RFC 7009 serialization follow-up (2026-11-21)

Branch: `agent/server-oauth-app-mutation-fences-20261121`, forked from
main `a9431fdefd08693eab7aa0aaab87169b877af239`. Independent of
pending auth-email, installer, OAuth secret-rotation and worker-startup PRs.

### Confirmed defects and changes

- The manual OAuth app update handler read `can_edit()` and the app's current
  type outside a transaction, then persisted scopes/grants/redirect URIs and
  translations later. Another replica could change app ownership, revoke or
  alter security policy in between. The handler now begins a transaction,
  acquires the same app-row mutation lock used by OAuth token issuance and
  manifest sync, rechecks editability under that lock and commits the base
  update, translations and authoritative response projection together.
- Explicit app revocation similarly read `can_revoke()` before acquiring a
  mutation lock. Revocation now checks current tenant-scoped app ownership
  under the lock before deactivating the app and invalidating tokens, consents,
  and authorization codes in the same transaction. The deactivation update
  also matches tenant ID and fails closed if no row is changed.
- RFC 7009 token-family revocation now reserves the app mutation fence before
  looking up the presented token. It therefore serializes with authorization
  code and refresh-token issuance that use the app-then-subject lock order,
  rather than racing creation of a refresh successor after an unprotected read.
  The scoped unknown-token path still succeeds without an existence oracle.
- The family-revocation update changes only rows not previously revoked, so
  repeated requests do not rewrite the original `revoked_at` timestamp.
- The ordinary OAuth client-secret rotation CAS also checks the live
  `app_type` against the allowed set, so a configuration/manifest change during
  Argon2 preparation cannot authorize a secret on an unsupported app type.
  A SQLite regression proves the stale CAS is rejected without changing the
  stored secret hash.
- A new SQLite integration regression simulates transfer of a manual app to
  manifest-owned state and ensures manual edit/revocation are rejected without
  altering its active status and redirect configuration. The existing RFC 7009
  family replay test now asserts preserved revocation timestamps.

**Verification not performed** (per maintainer instruction): no compilation,
formatting, test execution, database migration or multi-replica exercise. Before
merge run scoped `rustok-server` compile and OAuth integration tests, plus
PostgreSQL interleaving tests for manifest synchronization, app edits,
revocation and concurrent refresh/revocation operations. This patch does not
claim durable multi-replica recovery or eliminate every possible stale-read
in other app/service entrypoints.

## OAuth replay revocation transaction fences (2026-11-21)

Branch: `agent/server-oauth-reuse-revocation-fencing-20261121`, forked from
main `a9431fdefd08693eab7aa0aaab87169b877af239`. This is a
standalone `apps/server` OAuth token-service continuation and intentionally
does not depend on pending app mutation and rotation PRs.

### Confirmed interleaving defects

- The OAuth refresh-token reuse detector previously read a revoked token and
  calculated its grace age without holding the app-row mutation fence. Its
  family-wide update then ran without a transaction. A concurrent refresh
  operation could issue a replacement after the replay detector had already
  marked the then-current family revoked, leaving a live descendant from a
  lineage the server considered compromised.
- A used OAuth authorization code observed by the preflight path similarly
  revoked its issued access/refresh token lineage using a standalone update
  instead of an app-locked transaction. A concurrent code/refresh exchange
  could therefore pass the same unprotected window before the replay result.

### Remediation and coverage

- The refresh reuse detector first looks up an exact revoked, client/tenant
  scoped token without locking the app, avoiding a database write-fence per
  unknown token. A positive candidate opens a transaction, takes the canonical
  tenant/app mutation lock (including for inactive apps), and **re-reads** the
  candidate with all scoped predicates inside the transaction. The replay-age
  decision and family update are based solely on this authoritative read;
  the transaction commits before the protocol returns `invalid_grant`. Grace
  and missing-record no-op paths explicitly roll back to release SQLite's
  reserved writer. Persistence and commit failures return `server_error`.
- The authorization-code replay wrapper now starts a transaction, reserves the
  same app-row fence used by code exchange and refresh rotation, revokes the
  code-bound lineage via the existing shared in-transaction service, and
  commits the security effect. The CAS-miss path that already held the app
  lock is unchanged, avoiding a nested transaction.
- Existing SQLite refresh-reuse failure/retry and code-family scope tests now
  persist an OAuth app and tenant, exercising the actual row-lock path rather
  than a token table in isolation. The code replay test checks revocation when
  the app is already inactive, injects a database update failure and verifies
  that the failure surfaces and does not mutate tokens, then retries and
  verifies the scoped successful revocation.

**Verification outstanding:** No tests, compilation, formatting, lint, or
multi-replica PostgreSQL interleaving tests were run (maintainer runs tests).
Run the targeted `services::oauth_token_service::tests`, crate-level check,
and deployment concurrency tests before integration. This still does not
provide recoverable refresh-token response replay after a lost successful
response, nor distributed exclusivity for unrelated writers outside the
canonical app mutation lock.

## OAuth authorization-code creation fence (2026-11-21)

Branch: `agent/server-oauth-code-issuance-fence-20261121`, forked from
main `a9431fdefd08693eab7aa0aaab87169b877af239`; independent of
the other open server audit PRs.

### Confirmed TOCTOU defect

All OAuth authorization routes (JSON, browser direct issue and consent
approval) previously preflighted a client, grant, scopes, redirect and sometimes
consent before calling `OAuthAppService::generate_authorization_code`. The
service inserted the code without checking any of those authorities inside a
transaction. A concurrent client revoke or consent revoke could commit first,
invalidate all pending codes, and then the stale authorization route could
create a new code **after** the invalidation. A subsequent client reactivation
or renewed consent could make that code unexpectedly redeemable.

### Implementation

- The authorization-code generator starts a transaction and obtains the
  canonical tenant-scoped OAuth app lock followed by the user lock. It checks
  current app/subject activity, `authorization_code` grant and supported app
  type, exact registered redirect URI, non-empty scopes within the current
  app ceiling, and active scope-covering third-party consent.
- It then creates and hashes a random code, inserts the pending code and
  commits before returning plaintext. Failed policy decisions never insert a
  code. This preserves the original `generate_authorization_code` API used
  by all three REST authorization paths and shares the lock order used by
  consent grant/revoke, app mutation and token exchange.
- Focused SQLite regressions exercise missing/revoked/regranted consent,
  app revocation, cross-tenant request, stale redirect/scopes and inactive
  subject, including no-code-insert assertions.
- The controller now maps an authoritative admission miss to
  `invalid_client`, a stale policy/redirect/scope to `invalid_request`, and a
  missing active subject/consent to `access_denied` rather than conflating
  every failed admission with a 500. Unexpected persistence errors still map
  to redacted `server_error` and are logged. A transport regression checks
  both OAuth error kinds and HTTP status codes, including detail redaction.

**Verification left to maintainer:** no compile, format, Cargo tests, lints
or live PostgreSQL multi-replica interleaving tests were executed. Run
`cargo check -p rustok-server --lib` and
`cargo test -p rustok-server --lib services::oauth_app::tests` in the
relevant feature profile before merge, plus end-to-end authorize/consent
browser flows and concurrent revoke/issue tests on PostgreSQL. Security
under real concurrency depends on all cooperating paths using the same
app-then-subject lock order. This does not bind an authorization-code
write to a live HTTP session generation after an already-authenticated
request begins; token exchange still checks subject/credential epochs.

## Background worker single-owner admission — Notifications, Search and Index (2026-11-21)

Branch: `agent/server-notification-worker-startup-ownership-20261121`,
forked from main `a9431fdefd08693eab7aa0aaab87169b877af239`;
independent of the module dispatcher and OAuth draft branches.

### Confirmed defects

- Candidate, fanout and outbox-intake startup each checked
  `shared_contains::<...WorkerHandle>()` before dependency construction
  and later executed an unconditional `shared_insert(...tokio::spawn(...))`.
  Concurrent callers could both pass the initial check, launch duplicate
  tasks and overwrite the context's sole JoinHandle, leaving one task
  invisible to the normal drain. In Outbox intake, two callers could also
  concurrently register consumer event types and backfill receipts before
  any handle was published.
- Candidate and fanout worker startup had no admission-time check of the
  shared `StopHandle`. A late caller could spawn a worker while the server
  already had its shutdown signal set.
- Forum Search inbox, Forum Search typed contract consumer, Product Index
  refresh, and Social Graph Index consumer showed the same check-then-publish
  race. The three broker-backed consumers also opened persistent Iggy
  consumer groups asynchronously **before** publishing handles; concurrent
  starters could open two groups and lose a JoinHandle. All four also needed
  a stop check between preparation and task admission.
- A second caller observing an in-progress reservation initially returned
  `Ok(())`, even if the first caller had not yet finished and could still
  fail. That was a false startup-success signal for external bootstrap
  callers not coordinated by `ServerBootstrapInitializationLock`.

### Changes

- Added a typed `ServerRuntimeStartReservation<'_, T>` in
  `server_runtime_context.rs`: it takes a process-local atomic
  `shared_insert_if_absent` claim, releases it on startup error or future
  cancellation via `Drop`, and retains the claim after handle publication
  through `commit`.
- Candidate and fanout reserve after their readiness/dependency checks,
  before constructing/spawning their workers. If construction fails or
  shutdown begins during preparation, the claim is removed so a retry can
  succeed. Only the one admitted caller publishes a JoinHandle.
- Outbox intake reserves **before** async registration and full legacy
  receipt backfill; an aborted backfill cannot strand its local startup
  reservation. Successful backfill followed by handle publication retains
  the claim. Existing durable receipt idempotence remains unchanged.
- Forum Search inbox reserves immediately before worker admission. Its
  unsupported-backend and missing-source early exits remain no-ops without a
  claim, while a shutdown before publication aborts startup.
- Forum Search contract, Product Index refresh and Social Graph Index now
  reserve their distinct worker marker **before** the async persistent
  consumer open. This prevents duplicate group open and ensures cancellation
  during broker I/O automatically releases the reservation. Validate
  configuration and check shared `StopHandle` before/after broker open, before
  recording worker-start telemetry and publishing the task.
- Each of the seven callers now distinguishes a completed worker (idempotent
  success) from another starter still working (retryable `Error::Message`).
  The latter cannot be reported as success until the winner actually
  publishes a handle, and never overrides an existing one.
- The reservation constructor is deliberately lazy (`bool::then`, not
  `then_some`), because a losing contender must **not** construct a temporary
  drop guard capable of deleting the winning claim.
- Tests cover failed-start claim release, retained successful ownership,
  concurrent contention by 12 threads and cancellation by dropping an
  in-flight async startup future before it completes.

**Handoff:** No Cargo tests, rustfmt, clippy, compilation, migrations or
multi-process checks were run per maintainer preference. Run
`cargo check -p rustok-server --lib` and targeted
`services::server_runtime_context::tests`,
`services::notification_candidate_worker`,
`services::notification_fanout_worker`,
`services::notification_outbox_intake_worker`,
`services::forum_search_inbox_worker`,
`services::product_index_refresh_worker`, and
`services::social_graph_index_worker` tests. Validate lifecycle
shutdown/backfill interleavings and network consumer-open cancellation with a
real DB, Iggy broker and two concurrent starter calls. This reserves **one worker per runtime context**, not one per
deployment. It does not add durable cross-replica worker leader election.

## Worker recovery after DLQ publication failure and cooperative shutdown (2026-11-21)

Branch: `agent/server-social-graph-poison-recovery-20261121`, based on main
`a9431fdefd08693eab7aa0aaab87169b877af239`; independent of the
open startup ownership and OAuth branches.

### Defects confirmed in `apps/server`

- `social_graph_index_worker.rs` ended the raw poison DLQ publish retry loop
  with an `Err(_error)` match arm while its formatted error referenced
  `{error}`. This is an unresolved Rust identifier in a compiled function
  body and prevents the corresponding module from compiling. The raw broker
  error also should not become a general worker error surface because its
  text is uncontrolled transport data.
- `marketplace_financial_worker.rs` performed three distinct durable
  reconciliation stages sequentially on each timer tick: adapt provider
  reversals, sweep reversal events, and sweep paid-order events. After
  shutdown began during stage one or two, the worker could still admit later
  stages before checking the stop channel again. This lengthened shutdown and
  admitted avoidable external recovery work after host stop.

### Remediation

- Replaced the invalid raw-poison terminal error reference with a stable,
  redacted diagnostic containing the retry attempt count and the crucial
  guarantee that the source broker offset remains uncommitted. The existing
  release-claim/lease and receipt publication logic is preserved; on
  redelivery the worker can reclaim or recognize its durable receipt. Added
  a focused regression for the failure diagnostic.
- Factored marketplace financial reconciliation sequencing into
  `run_recovery_phases`. Each phase runs to completion (a durable operation
  is not cancelled mid-write), but the stop receiver is checked before the
  initial phase and after every phase; an observed stop prevents admission
  of the following phase. The outer worker exits rather than sleeping for
  another tick. A deterministic asynchronous test covers stop before any
  phase, after phase one, after phase two, after phase three, and successful
  all-three completion without stop.

### Verification and limits

**No compilation, tests, formatting, lint or live Iggy/database failover
exercises were run**, per maintainer preference. Suggested commands:
```sh
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::social_graph_index_worker::tests
cargo test -p rustok-server --lib services::marketplace_financial_worker::tests
```
Also exercise a real broker failure at the last DLQ attempt and a process
stop between financial phases with a real database. Crash between Iggy
publication and durable receipt publication still has an inherent
at-least-once retry window (dependent on the broker's deterministic ID
deduplication); this patch does **not** claim transactional exactly-once
delivery across broker and database.

## Server runtime follow-up (2026-11-21)

This bounded continuation follows merged PR #4759 and stays on
`agent/server-runtime-lifecycle-followup-20261121`.
The pass examined remaining host services, controller request boundaries,
middleware admission, transaction outcomes, and cooperative shutdown of
registered background workers; it is **not** a claim that every server path
has been exhaustively verified or that the full-stack audit gates are closed.

- Generic settings writes now reject both missing and nil actor UUIDs before
  opening a transaction. This preserves the documented non-nil actor identity
  for persisted settings rows and their transactional outbox events.
- CSP reporting batches count only actual CSP violations against the bounded
  processing limit, so unrelated Reporting API entry types cannot suppress
  CSP reports that appear later in the same batch.
- The search-backend readiness TCP target now parses URL authorities instead
  of splitting strings at slashes. Query strings/fragments no longer become
  hostname bytes; invalid ports and schemes fail consistently, with legacy
  bare `host:port` configuration retained.
- Observability authentication rejects **all** duplicate Authorization fields,
  including identical copies; this avoids intermediary-dependent token parsing.
- Focused regression tests were added for each change. **Cargo tests, format,
  integration, and multi-replica concurrency tests were not run** in the
  GitHub-API-only editing environment, per maintainer request.

### Continuation: retention, tenant parsing, and worker bootstrap

- Auth-artifact retention now deletes sessions, invite consumptions/invites,
  OAuth tokens, and OAuth authorization codes inside a **single transaction**.
  An error in any leg rolls back preceding deletes; counts are returned only
  after commit. A SQLite regression deliberately removes the second table
  after inserting an expired session and checks that the first delete rolls
  back. This changes transaction locking behavior, so verify with PostgreSQL
  under concurrent auth traffic before production rollout.
- The optional Notifications Outbox intake backfill checks the process stop
  signal before registration, after registration, before and after each page,
  and before each receipt. An interrupted bootstrap exits instead of draining
  an unbounded historical receipt backlog while host shutdown waits for the
  bootstrap lock. Outbox registration and acknowledgement remain durable and
  idempotent; no in-flight SQL operation is forcibly cancelled. A watch-signal
  regression test checks refusal after shutdown. This does not make initial
  backfill bounded in **normal** startup, and does not replace durable recovery.
- Tenant subdomain matching now checks UTF-8 slice boundaries before comparing
  configured base-domain suffixes. Malformed non-ASCII input cannot panic the
  handler, even when a caller bypasses the ordinary host validator. A test
  exercises a multibyte boundary.
- These changes were committed to PR #4760 (subsequently merged into
  `main`); no tests, builds, linters, format checks, or migrations were
  executed in that GitHub-only pass.

### Auth email task shutdown and locale follow-up (after merge of #4760)

The original host ran password-reset and verification email tasks as detached
Tokio tasks whose JoinHandles were dropped by HTTP callers. A best-effort stop
check before spawning did not make accepted sends part of the host drain.

- `AuthEmailTaskRegistry` now tracks active sends independently of returned
  JoinHandles. Task admission and registry closure use the same mutex, preventing
  a spawn racing with shutdown from escaping the task snapshot. Completion is
  tracked through a drop guard that also runs when cancellation occurs before
  the first poll. The host waits up to five seconds for sends and then aborts
  and observes remaining tasks within another bounded second.
- The panic boundary encloses future construction as well as its subsequent
  polling; a synchronous factory panic cannot escape as an unobserved join
  error. Added focused regressions for dropped caller handles, cancellation,
  panics, and the host shutdown integration point.
- Shared SMTP transport caching now keeps the first inserted pool under
  concurrent sender construction instead of overwriting a valid connection
  pool. Shutdown-skipped sends contribute to the existing skipped metric.
- Locale selection now matches the first language subtag (including uppercase
  and region variants), so arbitrary prefixes such as `ruby` or `russian`
  no longer select Russian templates.
- The code additions that preceded the merge are in #4760. The new locale,
  shutdown-skip telemetry and live documentation reconciliation are in
  `agent/server-auth-mail-lifecycle-followup-20261121`, based on main
  `c5000ee004c65e07f78083da242ed9c5be6e79b2`.
- **No tests, lint, formatter, builds, or integration suites were run here.**
  Maintainer should run focused auth-email unit/integration coverage, server
  compilation and format checks, and real SMTP shutdown scenarios.

This is only process-local graceful draining. Durable email delivery requires
an owner-level persisted request/outbox and replay contract; a hard process
kill, transport failure, or host timeout can still lose an email. It is not
safe to claim at-least-once delivery from this in-memory registry alone.

Unclosed owner-level work remains, including durable delivery/retry for
password-reset and verification email tasks, crash reconciliation of HTTP
installer jobs, and stronger lifecycle fences across process and replica
boundaries. This change does not assert those contracts are solved.

## Focus

Strengthen `apps/server` as the central backend runtime with formal API contracts, predictable operational diagnostics, and hardened security gates.

## Bounded authentication and OAuth follow-up (2026-11-17)

This ordinary server development pass is not an activation or completion of
the full-stack audit ledger. Work is isolated in
`fix/server-deep-review-2026-11-17` (PR #4742), based on
`9b32aacabc3c032a009eb9c18609547242766a29`.

- Unknown-email timing equalization now propagates both dummy Argon2
  initialization failures and verification failures; no empty-hash fallback.
- Password-reset credential CAS rejects accounts deactivated while the reset
  password was being hashed. Password changes reject expired current sessions.
- In-place first-party refresh/session lifecycle enforces the original 90-day
  absolute deadline across fresh issuance, refresh, active session checks,
  listing and password-change admission, including legacy rows.
  Follow-up verification corrected unchecked chrono arithmetic near its
  representable bounds and binds browser cookie Max-Age to the actual persisted
  refresh expiry (including near-90-day rotations), not a full fresh TTL.
- OAuth refresh `scope` can only retain or reduce the existing grant, never
  silently grant more than requested. Cookie-session refresh/logout reject
  duplicate same-name cookies rather than choosing an attacker-influenced order.
  OAuth refresh-family absolute deadlines also use checked Chrono arithmetic
  for both family admission and token issuance. Invalid persisted timestamps
  produce a typed OAuth error instead of a request panic.
- GraphQL host preflight now applies field, tenant, module, storefront, forum,
  dashboard and legacy-mutation policies to only the operation selected for
  execution. Module and legacy-mutation fragment walkers now terminate on
  cyclic fragment graphs while preserving detection of reachable protected fields.
- Channel REST authorization now reads the authenticated token's effective
  permissions (including OAuth scope restrictions), not persisted grants that
  could widen authority beyond the token. It also rejects tenant mismatches.
- DLQ listing/replay treats a top-level event-envelope tenant as authoritative;
  nested legacy tenant fallback applies only when the top-level key is absent.
  Replay repeats the tenant predicate within its atomic status update.
- The GraphQL selected-operation helper reads the parser's operation-name map
  keys, not a nonexistent field on the operation AST.
- Blank `OTEL_SERVICE_NAME` no longer masks a usable `RUSTOK_SERVICE_NAME`.
- Focused tests cover the added behavioral branches. Cargo, formatting,
  SQLite/PostgreSQL concurrency and real-HTTP tests are **not run** in this
  GitHub-API-only editing environment; the maintainer must run focused checks
  before merge. Broader server inspection is unfinished.

Additional follow-up: OAuth refresh-token reuse detection now propagates
database read/write failures as `server_error` (instead of reporting only
`invalid_grant`) and a fault-injection test covers retry after recovery.
Family-wide revocation is additionally scoped to the original OAuth subject,
so even malformed cross-user family IDs cannot revoke another user's tokens
inside one app/tenant.
First-party logout also accepts the immediately preceding rotated cookie
under the session row lock, revoking its live successor after a lost refresh
response. Refresh-reuse detection now takes that same database-specific row
lock before evaluating the grace window or revoking the session, preventing a
stale ActiveModel snapshot from overwriting a concurrent token rotation.
Focused regression tests cover the in-grace race, post-grace revocation, and
persistence-failure rollback. Session-id logout also acquires the session row
lock (or SQLite write fence) before converting the row into an ActiveModel
update; its regression test exercises the actual logout service and repeated
idempotent logout. The 30-second grace window is not an idempotent retry
protocol: the old token is rejected without revoking the current session, but
a lost successful response still cannot be recovered.

Remaining auth contract gaps requiring owner-level design and verification:
the in-place first-party refresh session stores only the immediately previous
digest, so replay/logout of still older generations cannot identify or revoke
the live lineage. OAuth still has no durable retry/incident reconciliation if
the caller does not retry after a revocation outage. These are not closed by
this pass and must not be reported as remediated.


## Server Engineering Audit Continuation (2026-10-28)

This is **bounded remediation**, not completion of the exhaustive FS-01
six-dimension audit. The reviewed baseline was
`999c9b2804febfe67a3bef7ef937e891d6aa8657` on `main`.

### Confirmed changes under review

- HTTP installer admission fails closed on a durable-job lookup error.
  A new append-only migration enforces at most one `running`
  `install_http_jobs` record across replicas through a database
  partial unique index on PostgreSQL/SQLite. Concurrent losers receive
  a stable HTTP 409. Existing duplicate `running` rows intentionally
  block migration until their side effects and receipts are reconciled.
  Public installer read errors no longer expose SQL/host paths.
- Manifest OAuth synchronization no longer silently ignores orphan-app
  deactivation failures. Each tenant's orphan deactivations and matching
  refresh-token revocations commit atomically; SQL failures roll back
  those deactivations. Newly created/updated desired apps still execute
  before the orphan transaction and **the complete reconciliation is
  not yet all-or-nothing**.
- REST user listing now has a deterministic tie-breaker for equal
  `created_at` values.

### Unclosed architectural and runtime gates

- **Installer:** the HTTP job still uses an in-process `tokio::spawn`.
  A process crash can strand a `running` job. HTTP now assigns a stable
  session ID before spawning and stores it on the job row, and the executor
  creates that session with the same ID. The executor still has no resume entry
  point; the redacted session plan and receipts do not prove whether an
  unreceipted external effect completed. A safe recovery/fencing design must
  reconcile immutable receipts and actual side effects before taking over; do
  not automatically delete, expire, or replay `running` rows. Add a multi-replica
  PostgreSQL contention test and crash/restart recovery evidence. PostgreSQL
  support for the new index must be verified before deployment.
- **OAuth:** implement a single versioned, serialized per-tenant
  reconciliation transaction that covers all upserts and deactivations,
  with explicit revocation fences for previously minted access tokens.
  Owner/transport authorization semantics must be preserved.
- **Build visibility:** platform-wide build history and unscoped
  `BuildEventHub` broadcasts are currently exposed behind tenant
  `modules:read/list/manage` grants; decide and document the
  authoritative host-global-vs-tenant visibility contract in an ADR
  before altering clients, event schemas and WebSocket behavior.
- **Email:** replace detached password-reset/verification send tasks
  with confidential, durable, idempotent delivery and retry evidence
  without enabling user enumeration.
- **GraphQL:** owner-view UUID projection now fails with a sanitized GraphQL
  error rather than panicking. The added conversion and transport tests still
  need execution with the project toolchain.
- **Build execution:** dry-run no longer claims a build. A live executor now
  attempts to record observed post-claim errors as failures and uses guarded
  status writes, but failed recording, process loss or task cancellation can
  still strand it. Implement durable fenced recovery, global admission and
  transactional notification delivery before claiming crash-safe execution or
  progress streams; the added Rust tests still require execution.
  New build receipts now preserve the inserted execution plan and reason;
  historical receipts retain their original response. Successful-build reuse
  across composition revisions and repeated `BuildRequested` notifications
  still require a coherent owner/event reconciliation design.

Required scoped verification before accepting these changes:
`cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`,
`cargo test -p rustok-server --lib`,
`cargo test -p rustok-migrations --lib`, and targeted PostgreSQL
schema/admission and OAuth failure-injection tests. None is claimed
passed without execution evidence.

## Search metrics empty-index correction (2026-11-02)

The SQLite search metrics query now normalizes empty-set public and stale
counts to zero, matching the existing PostgreSQL aggregate behavior. Previously
both values were `NULL`, causing a successful empty-index query to be reported
as a failed metrics collection. SQLite search lag calculations now round
integer millisecond differences instead of floating-point Julian-day arithmetic,
which avoids off-by-one results at whole and half-second boundaries. SQLite's
date parser has millisecond precision, unlike PostgreSQL's microseconds; the
readiness and Prometheus queries use the same calculation and compare parsed
instants rather than timestamp text. Search and outbox readiness also compare
signed lag against the full `u64` configuration range without wrapping large thresholds.
Focused Rust tests cover the changed branches but require execution with the
workspace's Rust toolchain and dependencies. This correction does not make the
existing lag or bootstrap metrics proof of source freshness or completeness;
that requires owner-backed reconciliation or durable checkpoints.

## WebSocket Authority Continuation (2026-11-02)

The build WebSocket now shares the GraphQL connection's live authority lease:
initial bearer identity is re-resolved against tenant, credential and current
RBAC state periodically and before event delivery, with a bounded lookup and an
expiry deadline. Outbound writes are bounded to avoid a stalled peer holding
an expired connection indefinitely. The process-local build hub uses atomic
singleton admission. The GraphQL init-receipt deadline is rechecked after it
fires so it does not interrupt an already received initialization; the separate
validation phase has a 15-second bound.
The global build-visibility decision and durable multi-process progress remain
open; this change does not resolve those boundaries. SQLite lease and socket
regression tests and a concurrent hub test are included but require execution
in a Rust-capable environment, alongside GraphQL WebSocket and multi-replica
integration verification.

### Module-work scheduler liveness (verification pending)

Readiness and runtime-worker metrics now include the shared module-work scheduler
when module composition registers scheduled work. Until registrations are inspected,
worker hosts treat the task as required; a confirmed empty set is optional. Surface
hosts do not require a local scheduler. These checks cover only task liveness, not
work progress or backlog. Module-work startup is serialized per runtime context;
concurrent initializers cannot register and spawn duplicate tracked schedulers.
An error or cancellation releases the initialization lock so a later attempt may
retry. Shutdown publishes a stop signal even if startup has not created it yet,
prevents startup after registration completes, and waits up to its shared drain
deadline for an ongoing initialization. This is process-local, not fleet-wide
ownership; registration callbacks retried after error or cancellation must handle
any external side effects they performed before failing. A stuck callback can
outlive the drain deadline. This does not serialize the whole bootstrap or other
worker/dispatcher startup against shutdown. Concurrency, error/cancellation retry,
liveness and shutdown tests are added but require execution with the project toolchain.

### Axum startup failure cleanup (verification pending)

The Axum host now invokes its bounded worker shutdown for ordinary initialization,
router composition, endpoint resolution, listener bind, and serving errors as well
as successful serving, preserving the original result. Shutdown also explicitly
aborts the channel/RBAC invalidation listeners, RBAC generation watchdog, and
in-memory event forwarder even if other handle clones remain. Aborts are requested,
not joined. An RAII guard schedules the bounded shutdown when the host lifecycle
future is cancelled or unwinds, but that cleanup is detached: the cancelling caller
cannot await confirmation, and runtime teardown/process abort can still prevent it
from completing. Not every detached or stop-signal-only task is tracked and drained;
an expired drain deadline can leave other work running. Rate-limit cleanup loops
and the Comments TCP listener are now stored in runtime-owned registries and
included in the bounded host drain. Direct users of the lower-level bootstrap APIs
remain responsible for shutdown. The host, listener, rate-limit, and supervisor
regression tests added in the latest source continuation have not been compiled or
executed.

The pre-existing RBAC source-guard mismatch exposed a rejected-read fallback in
the owner resolver. The owner now returns a typed consistency error after
bounded retries and the server maps it to a generic internal error rather than
using a potentially superseded grant or fabricating a denial. A deterministic
interleaving test and adapter tests are added but have not run. This is a local
cache-fill check, not a transaction or cross-replica revocation fence. See the
[owner decision](../../../DECISIONS/2026-11-02-rbac-cache-resolution-consistency.md).

After a matching request-snapshot invalidation, `RbacService` single/any/all,
permission-list and role lookups and the owner-policy adapter now fail unavailable
rather than fall back to unscoped persisted grants. Full-cache invalidation also
expires a snapshot in the calling task. This is an intentional availability
tradeoff, and a preceding mutation may have committed; clients must not blindly
replay it. The added unit and database regression tests have not been compiled or
executed. See the [request-snapshot decision](../../../DECISIONS/2026-11-02-invalidated-rbac-request-scope.md).

The GraphQL security extension now rejects new normal root resolvers when the
matching task-local snapshot is invalid, even when the owner reads the original
`AuthContext`. After a grant commit the generation-aware role writer clears
the whole local cache and expires the calling snapshot, including for an
unrelated role; the retained legacy hook expires it before a fallible membership
lookup. Added GraphQL sequencing and failed-lookup regression tests are not yet
compiled or run. The hook permits nested fields within a root already admitted, cannot stop
already-started fields, and does not reject a missing task-local scope. State does
not automatically follow spawned tasks or invalidate other requests/replicas.
Actual async-graphql 7.2.1 execution and transport behavior require integration
verification before claiming this boundary is protected end to end.

The owner GraphQL `update_role` now reserves the durable RBAC invalidation
generation in the same transaction as a supplied grant replacement. After commit
the server clears the local cache and attempts namespace-wide generation fan-out;
watchdogs can reconcile a missed fast notification. Added owner and server tests
have not been compiled or run. The durable marker is not an outbox or an immediate
revocation fence, and cancellation, unhealthy watchers and active request
snapshots remain limitations. PostgreSQL and multi-replica verification are open.
See the [grant-invalidation decision](../../../DECISIONS/2026-11-02-rbac-graphql-grant-invalidation.md).

### GraphQL operation admission ordering (verification pending)

Previously, the incoming-socket task tracked new operations before the GraphQL
protocol stream consumed cancellations, so rapidly cancelled subscriptions or
reused ids could be rejected as duplicates or overflow. Admission now occurs
when the protocol stream consumes each queued message; the host handles an
engine `complete` before polling the engine again. Slots are not eagerly
released by incoming cancellation, so an old completion cannot release a new
operation with the same id under that protocol ordering. On review, forwarding
the engine's cancellation acknowledgement was found unsafe for immediate ID
reuse on `graphql-transport-ws`: a client could treat it as the completion of
the new operation. That acknowledgement is now suppressed for the modern
protocol, but forwarded for legacy `graphql-ws`, whose documented flow expects
it. This cannot retract messages already in flight before cancellation; legacy
clients in particular must not assume an old completion identifies a reused ID's
new operation.

Regression tests exercise both supported subprotocols, queued cancellations,
natural completion, id reuse, duplicate ids, and the 32-operation limit. They
have not been compiled or run here. The fix depends on the locked async-graphql
protocol engine yielding a server `complete` for an active cancellation before
consuming a subsequent operation: the public upstream source near the 7.2.1
release shows this behavior, but the exact locked crate was not available for
verification. Run the tests against that crate and re-run them when upgrading it;
also test end-to-end over the actual socket with delayed writes and revocation.
Lease revalidation before polling and output is not a transaction fence for an
already running GraphQL operation: a mutation may commit after its authority
changes but before its result is revalidated and suppressed. Security-sensitive
mutations requiring immediate revocation must recheck authority at their own
commit boundary; the WebSocket transport alone cannot provide that guarantee.
`cargo test -p rustok-server --lib graphql_ws_admission --locked --offline`
could not reach compilation because the pinned Athanor git dependency was not
cached. A standalone Rust harness using the actual admission adapter with mocked
stream/channel/tracker types passed two ordering and overflow checks; it is not
an integration test or a type-check of the server against its real dependencies.

## Server P1 Security and Readiness Remediation (2026-11-01)

This is bounded remediation of the five P1 findings from the `apps/server`
logic review, not completion of the exhaustive FS-01 audit.

### Confirmed changes under review

- Invite acceptance now checks `features.invites_enabled` in the canonical
  `AuthLifecycleService::accept_invite_once_runtime` service before token
  decode. REST middleware, REST handlers, and GraphQL/native providers share
  the same gate; disabled invites remain indistinguishable from invalid tokens.
- Service-token permission snapshots now pass through the same tenant module
  availability filter as direct-user snapshots before role inference. Flex
  REST handlers call the owner-side `flex::rest::ensure_module_enabled` gate,
  and Flex GraphQL `require_access` calls the shared GraphQL module gate.
- Password reset, self-service password change, and admin password updates now
  write `users.password_changed_at` and revoke live delegated OAuth refresh
  tokens in the same transaction. Delegated access-token validation, refresh
  validation, and authorization-code exchange reject credentials issued at or
  before the epoch. A nullable additive migration owns the schema change.
- CORS preflight now allows the session-request, idempotency, host-authority,
  installer setup, and observability token headers already consumed by the
  server. A real preflight test replaces the previous Debug-only assertion for
  this contract.
- Readiness only requires local background-worker handles in `full` and
  `worker` host modes. API/SSR surface modes emit a non-critical worker-host
  mode check instead of failing readiness for workers they do not run.

### Worker liveness follow-up (partial)

Readiness and worker metrics now share local task snapshots for auth retention,
module dispatch and lifecycle maintenance, and feature-gated payment, commerce,
product and workflow workers. Dispatcher composition explicitly records whether
it has listeners, so an uninitialized registration fails closed while a confirmed
empty listener set does not require a task. Host modes without local workers do
not require these handles. These are liveness checks, not proof of successful
processing; progress/backlog observability and coverage of other background
workers remain open. Rust compilation and the added tests require a toolchain.

### Verification status

The sandbox has no project-compatible Rust toolchain, PostgreSQL, Redis, or Docker.
A temporary Rust/Cargo 1.88 was obtained for metadata and formatting inspection,
but the workspace requires Rust 1.96. Offline server checking stops before
compilation because the Athanor git dependency is not cached; registry access is
also restricted. `git diff --check` and diff invariants pass. The architecture
snapshot guard currently reports the existing `rustok-server -> flex` dependency
against its allow-list; the relevant manifests and guard rules are unchanged by
this patch. Rust compilation, crate tests,
migration smoke, and infrastructure-backed tests remain required before merge:
`cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`,
`cargo test -p rustok-server --lib`,
`cargo check -p flex --lib`,
`cargo test -p flex --lib`,
`cargo check -p rustok-migrations --lib`,
`cargo test -p rustok-migrations --lib`, and
`scripts/verify/verify-migration-smoke.sh`.
No Rust or PostgreSQL verification is claimed passed without execution evidence.

## Module Platform Handoff

The server is the host and transport composition layer for the module platform;
it is not the owner of module marketplace/control-plane business logic. The
canonical sequence is maintained in the
[module-platform implementation plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

Server work for that plan is:

- mount the `rustok-modules` facade through authenticated tenant/actor contexts;
- expose static module lifecycle toggles only through the owner typed command:
  GraphQL derives tenant, actor, and `modules:manage`, requires a UUID
  idempotency key plus a non-negative aggregate revision, and leaves journal
  correlation/replay/no-op receipt handling to the owner. Static toggles,
  normalized settings, retry, and compensation share the owner lifecycle
  aggregate and its fail-closed execution claim;
- expose artifact tenant lifecycle only through the owner-issued GraphQL snapshot
  and revision-CAS mutation; transport derives tenant/actor/permission and does
  not read lifecycle, admission, or outbox tables directly;
- supply database, OCI, trust, events, audit, clock, and other infrastructure
  adapters; module-build scheduling and execution run in their separate
  dispatcher and worker deployments. The server queues and streams build
  events and requests builds through `PlatformCompositionBuildService`, but it
  does not own a build executor. `services/build_executor.rs` was an
  installer-era wrapper that no longer had a caller after the release
  activation hook moved into the owner; it also hard-wired
  `NoopBuildEventPublisher`, so keeping it would have advertised a build path
  that silently published nothing. It is removed, and the only remaining
  `BuildExecutionService` composition is the CLI worker
  (`crates/utils/rustok-cli-platform/src/rebuild.rs`);
- keep MCP stdio out of the host. `DbBackedMcpRuntimeBridge` exposed
  `bind_stdio_config`, an unused helper that implied the host could assemble a
  stdio MCP session; the module plan states generic stdio is a library/CLI
  surface and that tenant-owned tools are reachable only through the
  authenticated JSON/SSE remote transport. The helper had no caller and is
  removed, so the host has exactly one MCP path (remote bootstrap + tool
  transport) and no second, unaudited composition to drift from it;
- obtain every control-plane service through `ModuleControlPlane` instead of
  constructing owner types in the host. The RBAC permission-ownership map is the
  one host adapter that previously built `ModuleDefinitionCatalog` directly
  (`services/rbac_module_availability.rs`); it now derives the same catalog from
  `ModuleControlPlane::new(db).catalog(registry)`. The map is memoized in the
  runtime context, so the added control-plane handle costs one construction per
  process;
- migrate platform composition, build enqueue, registry governance, effective
  policy, GraphQL, and native adapters to owner operations;
- keep production release admission, desired/observed rollout, activation, and
  recovery exclusively in `rustok-modules`; the server exposes only narrow
  authenticated transports and deployment-agent observations, never another
  release head or activation side-effect hook;
- adapt typed manifests and bootstrap-file loading at the host boundary while
  `SeaOrmModuleCompositionService` owns canonical active-snapshot reads and
  bootstrap persistence, revision-CAS updates, and the combined CAS/build
  transaction; the server's build adapter receives the owner transaction and
  publishes the build notification only after commit;
- split compile-time Core/static implementation registration from the durable
  artifact-aware definition catalog and runtime dispatcher;
- keep the static registry boot-owned in `ServerRuntimeContext`; request guards
  consume that injected adapter and fail closed instead of constructing a
  registry per request; installer execution receives the same boot-owned
  registry explicitly and does not construct a second topology;
- supply platform content-addressed artifact storage, transactional outbox, and
  multi-node reconciliation adapters;
- compose artifact and Alloy Rhai through one shared mTLS-authenticated isolated
  worker client, require exact readiness at startup, and never register an
  in-process Rhai fallback; keep Wasmtime placement explicit;
- preserve transactional and transport parity guarantees during cutover;
- delete replaced service business logic, error taxonomies, direct writes, and
  runtime Cargo execution references; trusted static distribution builds remain
  installer/CLI operations and never start from server runtime workers;
- keep only Core/bootstrap and explicitly promoted native modules in static
  host composition.

The server must never compile untrusted module source, load marketplace native
libraries, or modify its source/Cargo graph during runtime installation. It
must also never fetch an external OCI payload for every execution, grant an
artifact raw infrastructure clients, or require an artifact-only module to
implement `RusToKModule` in process.

## Host-global authority composition

Process-wide Events, Iggy, System and Settings resources are not tenant-owned.
The server must not infer authority over them from tenant roles, permissions,
OAuth applications, OAuth scopes, app metadata, a default tenant or magic UUID.

The implementation merged in PR #2735 as
`1ce83819b077ef6e0df009fd5675f556315ef63a` uses a dedicated host-owned
credential boundary:

- callers present `X-RusTok-Host-Token` only over HTTP/native requests;
- the deployment stores only SHA-256 digests, non-nil operator audit ids and
  explicit `read`/`manage` levels in `RUSTOK_HOST_AUTHORITY_CREDENTIALS`;
- parsing is bounded, duplicate token hashes are rejected and digest comparison
  is constant-time;
- Axum middleware removes and authenticates the raw header once before
  downstream dispatch;
- native Events and Iggy server functions receive only typed
  `HostAuthorityContext` request extensions;
- HTTP GraphQL consumes the same typed authority from a request-task-local scope
  and never re-reads the raw header or credential configuration;
- GraphQL WebSocket remains fail-closed and does not inherit host authority;
- Iggy native/GraphQL mutation additionally requires ordinary authenticated
  tenant context equal to the routed tenant because encrypted connector secrets
  remain tenant-owned; the mutation audit actor remains the host operator;
- overlap rotation and revocation are deployment configuration operations, not
  tenant OAuth/RBAC writes.

The operator runbook is
[`host-authority.md`](./host-authority.md). Superseded PR #2726 is historical
staging only. Issue #2680 remains open until same-SHA source/unit/compile
evidence and retained live denial, admission, rotation, revocation and
multi-replica parity evidence exist.

## RBAC artifact permission control-plane composition

Artifact role-permission grants and role/permission metadata are tenant RBAC
control-plane state. Server and native adapters may authenticate and map
transport errors, but they must not define a second principal policy or treat an
effective permission as sufficient proof of principal eligibility.

PR #2747 merged the owner-provided `RbacControlPlanePrincipal` policy as
`75b67f877eb405abe4e6761a16d6b7ece98bc103` for GraphQL, REST and native
RBAC Admin. The current cycle replaces owner-side reconstruction from OAuth
metadata with one explicit shared principal kind:

- `rustok-api::AuthPrincipalKind` is host-neutral and distinguishes direct user,
  OAuth delegated user and service principal;
- the access-token resolver validates the token and classifies the claims once,
  storing the typed result on `CurrentUser`;
- HTTP/native middleware and GraphQL HTTP/WebSocket composition only propagate
  `CurrentUser.principal_kind` into `AuthPrincipalContext`;
- unknown or inconsistent grant/client/session shapes fail closed in the auth
  resolver before request data reaches module policy;
- GraphQL role reads/writes, REST artifact-role permission writes and native
  RBAC Admin require typed principal context before effective permission checks;
- only `DirectUser` with authenticated/routed tenant equality may enter RBAC
  control-plane operations;
- delegated users and services remain denied even when their effective scoped
  permissions include `modules:manage`, `users:manage` or `settings:read`;
- the owner principal contains only typed kind plus tenant id and has no fallback
  to `client_id`, `grant_type` or `session_id`;
- the durable REST operation actor remains `AuthContext.user_id`; request
  payloads cannot supply or replace the audit identity;
- the owner crate remains independent of Axum and the `rustok-api/server` feature.

The contract is documented in
`crates/modules/rustok-rbac/docs/explicit-principal-kind.md`. Same-SHA API/auth-resolver/
RBAC/Admin/server compilation, focused tests, source verifiers and live negative
transport evidence remain required before this correction is verified.

The default `rustok-rbac` crate compiled on exact PR #2747 head
`3cf4b3a44980ca257f7f53849e905673141db289` inside Rust-host workflow run
`30650883159`. That workflow then reproduced issue #2740 before building
`rustok-server`; it is historical evidence only for the pre-typed revision.

## RBAC durable invalidation observability composition

The server owns the process-level watchdog that reconciles the authoritative
RBAC database generation with the generation applied to local permission
snapshots. The cycle-001 RBAC source slice instruments that existing worker; it
does not create a second counter, cache authority, listener or recovery path; it remains the only recovery path.

The host adapter now reports through the canonical `rustok-telemetry` registry:

- durable and locally applied generation;
- signed durable-minus-applied lag, including a negative regression signal;
- watchdog running state and bounded restart reasons;
- durable-generation database read failures;
- bounded recovery and process-wide permission snapshot clear reasons.

The worker records positive lag before catch-up, zero after a successful applied
checkpoint, and preserves negative lag when the database regresses below the
monotonic process checkpoint. Recovery still clears permission snapshots through
the existing owner/runtime path. No metric contains tenant, user, role,
permission, session, OAuth client or cache-key labels.

Alert thresholds and Redis outage/restart, missed PubSub, generation regression
and canonical role-repair procedures are owned in
`crates/modules/rustok-rbac/docs/README.md`. Source guard
`scripts/verify/verify-rbac-invalidation-observability.mjs` locks registry,
worker, documentation and cursor synchronization. Compilation, test execution,
two-replica Redis recovery and one complete authorization incident trace remain
required evidence.

## Improvements

### Architecture debt

- Reduce coupling between HTTP/GraphQL layers and modular business logic through stricter service boundaries.
- Unify module lifecycle (bootstrap, readiness, graceful shutdown).
- Reduce transport/auth configuration duplication across subsystems.

### API/UI contracts

- Finalize a unified error contract for REST and GraphQL (codes, machine-readable fields, correlation id).
- Stabilize tenant-aware header and auth claim contracts for all frontend clients.
- Expand public API change versioning via changelog/contract notes.
- Bring MCP management surface (`/api/mcp/*`, GraphQL `mcp*`) to platform-grade: persisted clients/tokens/policies/audit, session-start runtime binding, live binding Alloy scaffold tools to persisted draft store and persisted Alloy scaffold drafts already exist; server-owned remote MCP transport bootstrap (`POST /api/mcp/runtime/bootstrap`) added as primary token-to-runtime-binding handshake; remote JSON/SSE transport for core registry tools (`POST /api/mcp/runtime/tools/call`, `POST /api/mcp/runtime/tools/stream`) added with persisted binding, policy enforcement and audit trail; remote JSON/SSE transport also extended to Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`) via server-owned persisted draft store; next step — surface these remote MCP operations in admin UI.

### Observability

- Align metric coverage across all critical endpoints and background event processing.
- Add end-to-end tracing: gateway -> handlers -> modules -> outbox/transport.
- Build SLO dashboards for latency/error budget and health per module.
- Execute and retain RBAC durable-generation metric, lag, worker-restart and full-clear evidence across Redis outage/restart and missed-publication recovery.

### Security

- Strengthen RBAC enforcement checks at middleware and service layer levels.
- Introduce regular security review for sensitive endpoints (auth, tenant, admin operations).
- Expand security event audit (login, privilege changes, tenant boundary violations).
- Retain host-global operator credential denial/admission, rotation and replica-parity evidence without moving credential ownership into tenant OAuth or RBAC.
- Keep role/permission control-plane principal admission owner-defined, typed and ahead of effective permission checks on every transport.

### Test coverage

- Increase integration test share for module scenarios with real DB/migrations.
- Add contract tests for API response stability for frontends.
- Include negative tests for RBAC/tenant isolation and failure-mode tests for event transport.
- Add live HTTP/native host-authority tests for no header, wrong token, read/manage hierarchy, audit identity, Iggy authenticated/resolved tenant ownership, rotation/revocation and WebSocket denial.
- Retain REST, GraphQL and native RBAC tests/verifiers for delegated/service denial, missing typed context, tenant mismatch, missing permission and trusted actor propagation.
- Run the targeted API/auth-resolver/RBAC/Admin/server checks and focused principal-kind and invalidation source verifiers before the complete Wave 2 server sweep.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `pending`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `cross-owner Tenant trust sweep for host-global Events/Iggy/System/Settings authority; active core/rbac review of access-token claim classification, typed propagation across HTTP/native middleware and GraphQL HTTP/WebSocket, REST artifact-role permission writes and native RBAC Admin; canonical durable-generation watchdog and process telemetry registry`
- Findings: `P0=3, P1=4, P2=1, P3=0` (two host-global P0 findings and one raw-credential-lifetime P1 belong to the Tenant/Events interaction; the third P0 is the historical RBAC REST invalid principal grant; the second P1 is historical native role-metadata admission inconsistency; the third P1 is missing invalidation observability; the fourth P1 is repeated owner inference from grant/client/session metadata; the existing P2 is the earlier module-control-plane construction finding)
- Fixed in this pass: `PR #2735 merged the host-owned operator credential boundary as 1ce83819b077ef6e0df009fd5675f556315ef63a. PR #2747 merged one owner control-plane policy as 75b67f877eb405abe4e6761a16d6b7ece98bc103. Current core/rbac source work adds canonical invalidation observability and introduces host-neutral AuthPrincipalKind plus mandatory AuthPrincipalContext; the access-token resolver is the single classifier, HTTP/native and GraphQL are propagation-only, every RBAC transport consumes the typed context and the owner no longer receives client_id, grant_type or session_id.`
- Remaining risks or blockers: `the complete apps/server Wave 2 inspection has not started; the RBAC corrections lack same-SHA format, rustok-api default/server, telemetry, all-feature owner, admin SSR/server compilation, focused Rust/source verifier tests and live transport evidence; PostgreSQL concurrency, two-replica Redis recovery, one complete authorization incident trace and module-owned management-flow evidence remain absent; host-authority source/unit/compile and live denial/admission/rotation/revocation/replica evidence also remain pending; issues #2680 and #2740 remain open`
- Evidence: `source audit confirms the auth resolver maps validated claims into the closed typed enum and stores it on CurrentUser; middleware and GraphQL HTTP/WebSocket only propagate that value; RBAC GraphQL, REST and native adapters require the typed request context; owner policy contains only kind plus tenant and has no grant/client/session fallback. Source guards cover the single-source typed contract and admission order. No command execution is claimed. Historical PR #2747 head 3cf4b3a44980ca257f7f53849e905673141db289 compiled only default rustok-rbac before issue #2740 stopped the workflow.`
- Next action: `continue the core/rbac cursor: run targeted API/auth-resolver/RBAC Admin/server checks and principal-kind, tenant and invalidation source verifiers on one revision, then retain PostgreSQL concurrency and multi-replica Redis recovery evidence; leave the complete server composition audit for its Wave 2 cursor visit`
- Resume command: `cargo fmt --all -- --check && cargo check -p rustok-api && cargo check -p rustok-api --features server && cargo check -p rustok-rbac --all-features && cargo check -p rustok-rbac-admin --features ssr && cargo check -p rustok-server --lib && cargo test -p rustok-api authenticated_facts_classify_fail_closed && cargo test -p rustok-server --lib token_claim_classifier_returns_explicit_principal_kinds && cargo test -p rustok-rbac --all-features && cargo test -p rustok-server --test rbac_artifact_permission_control_plane_guard && node scripts/verify/verify-rbac-explicit-principal-kind.mjs && node scripts/verify/verify-rbac-admin-tenant-scope.mjs && node scripts/verify/verify-rbac-invalidation-observability.mjs`

## P2 remediation batch (2026-11-01)

This batch continues the server review after the P1 fixes. It is implemented
in the working tree but has not been compiled or exercised against PostgreSQL,
Redis, or a running host in this environment.

### OAuth replay and lineage scope

- `oauth_tokens.authorization_code_id` is an additive nullable column owned by
  `rustok-auth` (`m20261101_000002_add_oauth_token_authorization_code`). It links
  delegated refresh-token lineages to the authorization code that issued them.
  Service-client tokens and pre-migration rows keep NULL.
- Authorization-code exchange now detects replay both before the CAS and on a
  CAS miss. A replay revokes the delegated refresh lineage bound to the same
  code, client, tenant, and subject. The CAS-miss revocation is committed before
  `invalid_grant` is returned.
- Refresh rotation CAS misses run the existing reuse detector. Refresh-family
  root lookup and reuse revocation are scoped by client, tenant, and subject;
  a missing family root fails closed instead of resetting the absolute cap.
- Stateless OAuth access tokens are not extended with a family claim in this
  batch. Their exposure after lineage revocation remains bounded by the
  15-minute user access-token TTL; refresh reuse is denied immediately.

### HTTP edge labels and upload deadlines

- Metrics path labels come from the routed `MatchedPath` captured by a route
  layer. Unmatched requests use fixed `:not_found` / `:unmatched` buckets
  regardless of status; raw URL segments are no longer sanitized into an
  unbounded label vocabulary.
- Extended upload timeouts are limited to the Media owner streaming upload
  route and the registry artifact publication route. Bounded JSON artifact
  dispatch and client-controlled Content-Type alone no longer receive the
  upload deadline.

### Environment and log contracts

- `common::settings::resolve_environment_name` is the canonical selector for
  config loading and production gates. `RUSTOK_ENV`, `RUST_ENV`, and `APP_ENV`
  must agree when present; conflicting aliases fail startup, and earlier
  production checks fail closed.
- Auth entry-point debug logs, disabled email-sender logs, superadmin bootstrap
  logs, and comments schedule-audit ambiguity warnings no longer emit raw
  email addresses, recipients, or claim tokens.

### Verification still required

- `cargo check -p rustok-server`
- `cargo test -p rustok-server oauth_token_service`
- `cargo test -p rustok-server http_stack`
- `cargo test -p rustok-server common::settings`
- `cargo test -p rustok-auth migrations`
- `scripts/verify/verify-migration-smoke.sh` against PostgreSQL
- `scripts/verify/verify-remediation-gate.py` with Cargo available

## P3 runtime-boundary remediation batch (2026-11-01)

This batch continues the bounded server review after P1/P2. It is implemented
in the working tree but has not been compiled or exercised against PostgreSQL,
Redis, or a running host in this environment.

### Module event dispatcher lifecycle

- `spawn_module_event_dispatcher` now starts the module-owned event dispatcher
  only in background-worker profiles (`full` and `worker`). API, admin SSR,
  storefront SSR, and registry-only profiles still compose module runtime
  extensions and individually gated helper workers, but do not start the local
  dispatcher receive loop.
- Sequential dispatcher startup checks for a host-owned
  `ModuleEventDispatcherHandle` in `ServerRuntimeContext`; this check is not
  atomic against concurrent bootstrap. `shutdown_runtime_workers` requests a
  cooperative stop and joins the dispatcher with a bounded timeout, falling back
  to abort if the drain exceeds that deadline. This does not establish durable
  delivery or prevent a concurrent startup/shutdown race.

### Platform settings write serialization

- `SettingsService::update` now reads the existing row, preserves secrets,
  validates, and writes inside one transaction. The update is a CAS against
  `schema_version`; a lost race rolls back and retries with a fresh snapshot.
- First-write races are retried when the unique `(tenant_id, category)` index
  rejects a concurrent insert. After a bounded number of attempts the service
  returns `SettingsError::WriteConflict` instead of silently overwriting.
- GraphQL maps the conflict to a stable retryable client signal. The public
  input/payload still does not expose `schema_version`; callers that need
  end-to-end optimistic concurrency must add an expected-version field in a
  separate API contract change.

### GraphQL channel propagation

- HTTP GraphQL requests now insert the middleware-resolved `ChannelContext`
  into async-graphql request data, so owner resolvers that build `PortContext`
  from `ctx.data::<ChannelContext>()` receive the same channel as REST.
- GraphQL WebSocket upgrades carry the handshake-resolved channel into
  `connection_init` data. The channel tenant must match the tenant resolved
  from `connection_init.tenantSlug`; mismatches fail before authentication
  lease creation. `RequestContext` channel fields are populated for both
  transports instead of being hardcoded to `None` on WebSocket.

### Redis rate-limit connection reuse

- The Redis rate limiter now lazily creates one reconnecting
  `ConnectionManager` per limiter and shares it across clones, namespaces,
  limit checks, and health probes. The existing two-second Redis operation
  timeout still bounds connection acquisition and commands.
- This removes per-request connection churn while preserving the distributed
  Lua INCR/EXPIRE/TTL semantics and the memory-backend fallback.

### RBAC availability fail-closed presentation

- When tenant module-availability resolution fails, the host provider no
  longer returns `None` (which the RBAC owner interprets as “all permissions
  are platform-core”). It returns a snapshot with known permission ownership
  and an empty enabled-module set, so module-owned permissions are presented
  as dormant and newly dormant grants are rejected.
- Hosts without a module registry still return `None` through the existing
  handle construction path; this change only affects resolution failures on
  hosts that do have a registry.

### Verification still required

- `cargo check -p rustok-server`
- `cargo test -p rustok-server module_event_dispatcher`
- `cargo test -p rustok-server app_lifecycle`
- `cargo test -p rustok-server settings_service`
- `cargo test -p rustok-server graphql::settings`
- `cargo test -p rustok-server controllers::graphql`
- `cargo test -p rustok-server rate_limit`
- `cargo test -p rustok-server rbac_module_availability`
- PostgreSQL concurrent settings-update and unique-insert contention tests
- Redis outage/reconnect and rate-limit contention tests
- `scripts/verify/verify-remediation-gate.py` with Cargo available


## P4 protocol-boundary remediation batch (2026-11-01)

Scope: settings write-conflict taxonomy, RFC 7009 refresh-family revocation,
and GraphQL WebSocket operation/auth lifetime bounds. This batch does not
introduce a client-visible settings version contract and does not add a
cooperative dispatcher drain; those remain separate follow-ups.

### Implemented

- `rustok-api` GraphQL error taxonomy now includes `ErrorCode::Conflict`
  (`CONFLICT`) and `GraphQLError::conflict`. Platform settings
  `WriteConflict` maps to that extension code instead of `BAD_USER_INPUT`,
  so clients can distinguish retryable optimistic-concurrency conflicts from
  invalid input. Regression tests assert the extension code in both the API
  crate and the server settings mutation mapper.
- Settings unique-violation detection uses SeaORM's portable
  `DbErr::sql_err()` / `SqlErr::UniqueConstraintViolation` classifier instead
  of matching only one runtime-error shape.
- RFC 7009 revocation is transactional and family-scoped. The service reads
  the token by hash inside the transaction, coalesces `family_id` to the row
  id for pre-lineage rows, and revokes the whole lineage with one
  `update_many` scoped by client, tenant, and user (or `user_id IS NULL` for
  service tokens). Unknown hashes still succeed without revealing existence.
  The controller uses the security-only OAuth client lookup so presentation
  translation hydration cannot decide credential validity. SQLite regression
  tests cover unknown-token success, whole-family idempotent revocation, and
  cross-client/cross-tenant isolation.
- GraphQL WebSocket hosts admit operations as the protocol engine consumes
  queued input: at most 32 active ids, ids must be non-empty and at most
  128 bytes, duplicate ids close with 4409, overflow with 4429, and invalid
  ids with 4400. Slots are released when the server emits `complete`.
  `CurrentUser` carries the signed access-token expiry; the WS lease closes
  at that expiry and revalidates authorization every 30 seconds while idle.
  Pure tracker tests cover cap/duplicate/release/id validation.

### Known limitations / pending implementation

- OAuth access JWTs are registered credentials. RFC 7009 and security-family revocation
  revoke access and refresh rows together, fencing already-issued access JWTs before TTL
- A shorter service-token TTL policy and an OAuth code-reuse domain event
  remain separate contract changes.
- PostgreSQL/Redis fault-injection checks remain a deployment gate.

### Verification status

Written without a local Rust toolchain; not compiled or tested in this
sandbox. Run before merge:

```bash
cargo check -p rustok-api
cargo test -p rustok-api --lib
cargo check -p rustok-server
cargo test -p rustok-server --lib graphql::settings
cargo test -p rustok-server --lib services::oauth_app
cargo test -p rustok-server --lib controllers::graphql
cargo test -p rustok-server --lib extractors::auth
```


## Runtime task drain completion (2026-11-12)

Scope: close confirmed shutdown gaps in runtime-owned tasks that subscribe to
the host stop signal or have an explicit runtime handle. This is not a
repository-wide detached-task audit; test-only spawns, supervisors whose
abort-on-drop ownership already bounds lifetime, and unrelated service
lifecycle work are outside this batch.

- Host shutdown now takes and joins the physical Event DLQ duplicate-alert
  observer and its observability projection, Product channel reconciliation,
  Product Index refresh, and Social Graph Index consumer/position/poison
  observer handles, and the Comments schedule-audit handoff worker after
  broadcasting stop. Both Forum Search workers (inbox and typed-contract
  consumer) are drained too. Tenant cache-generation listeners now retain every
  spawned local/Redis/reconciliation task for the same drain. Tenant locale
  listener tasks, previously cancelled only when their abort-on-drop handle was
  dropped, are also explicitly cancelled and joined by host shutdown. The same
  explicit drain also covers Channel/RBAC invalidation listeners, the RBAC
  generation watchdog, Event forwarder, Cache Redis status monitor,
  field-definition cache invalidation/reconciliation workers, and the
  feature-gated SEO redirect-cache reconciler. Their abort-on-drop wrappers
  remain as a fallback.
- Module-worker drains now retain abort-on-drop `JoinHandle`s through timeout;
  if a worker exceeds the deadline, shutdown aborts it and awaits cancellation
  instead of dropping the join future and detaching the task. If the host drain
  itself is cancelled, the local guards still abort owned tasks.
- The Forum Search typed-contract consumer handle is included alongside the
  inbox worker; it was previously absent from the host drain.
- The Outbox relay supervisor uses a `JoinSet` for its inner processing task:
  graceful shutdown now awaits its current pass and joins it, and dropping the supervisor still aborts children
  instead of detaching them.
- Each drain is bounded to five seconds. If a task does not cooperate, shutdown
  aborts it and awaits the join result rather than dropping the JoinHandle and
  leaving the task detached.
- Added a regression test for the timeout abort-and-join fallback. Existing
  signal/handle shutdown tests continue to cover the previously fixed listener
  and rate-limit cleanup tasks.
- The one-shot installer HTTP execution task remains outside this worker-drain
  batch. Its durable `running` row is protected by a cross-replica unique index,
  so stale-job recovery/shutdown handling needs a separate ownership and
  recovery design; it must not be cleared by a blind startup sweep.
- Cargo is unavailable in this environment, so compilation and Rust tests remain
  unverified; `git diff --check` is the available whitespace check.

## P5 dispatcher cooperative-drain batch (2026-11-01)

Scope: close the remaining dispatcher shutdown gap from P3. The core
`EventDispatcher` now owns a cooperative stop signal and a real in-flight
task boundary; host shutdown uses it instead of aborting the receive loop
as the primary path.

### Implemented

- `RunningDispatcher` carries a `watch` stop signal. `request_stop` stops
  admission of new events; the receive loop then drains its `JoinSet` of
  admitted dispatch tasks before exiting. `stop` is now an async
  request-stop-plus-join operation, while `abort`/`abort_handle` remain the
  emergency cancellation path.
- Concurrent handler tasks are no longer detached from the dispatch task.
  `dispatch_to_handlers` owns a handler `JoinSet`, waits for all matching
  handlers, then releases backpressure and records dispatch latency. This
  makes dispatcher drain observable at the handler boundary and prevents
  finished-task retention in the receive-loop `JoinSet`.
- Host shutdown calls `ModuleEventDispatcherHandle::request_stop`, joins
  under the existing five-second module-worker deadline, and aborts through
  an `AbortHandle` only if the drain times out.
- Existing core/blog/commerce dispatcher callers were updated to the async
  `stop` contract. Regression tests cover cooperative drain of an in-flight
  blocking handler, host-handle cooperative join, and the existing abort
  fallback.

### Known limitations / pending implementation

- Drain is bounded by the host deadline; a handler that ignores cancellation
  and blocks past the deadline is still aborted with the dispatcher task.
  Durable recovery remains the responsibility of the outbox/worker contracts.
- Client-visible expected-version settings CAS and stale source-guard
  stabilization remain separate follow-ups.

### Verification status

Written without a local Rust toolchain; not compiled or tested in this
sandbox. Run before merge:

```bash
cargo check -p rustok-core
cargo test -p rustok-core --lib events::handler
cargo check -p rustok-server
cargo test -p rustok-server --lib services::module_event_dispatcher
cargo test -p rustok-blog --test comment_projection_postgres_test
cargo test -p rustok-blog --test comment_projection_dispatcher_duplicate_postgres_test
cargo test -p rustok-commerce --test integration
```

## P6 — Client-visible optimistic concurrency for platform settings

Internal compare-and-swap alone still lets two admins load the same settings page and silently
overwrite each other. The generic settings write path now exposes the persisted revision to every
current admin client:

- `SettingsService` returns `SettingsSnapshot { settings, schema_version }` from `get_snapshot` /
  `get_all_snapshots`. A missing row is revision `0`; the cache category still reports the
  deployment-resolved document but carries the persisted row's revision.
- `update_if_current(..., expected_schema_version)` checks the expected revision inside the same
  transaction that reads, validates, preserves secrets, and writes the row. A mismatch rolls back
  and returns `WriteConflict`; negative expectations are validation errors. The bounded internal
  retry loop remains only for the internal/bootstrap `update` wrapper.
- GraphQL `PlatformSettingsPayload` and `UpdatePlatformSettingsPayload` expose `schemaVersion`.
  `UpdatePlatformSettingsInput` requires `expectedSchemaVersion`; stale writes map to the shared
  `CONFLICT` error so clients can re-read and retry rather than assuming success.
- `rustok_cache::CacheSettingsAdminPort` now reads/updates `CacheSettingsSnapshot` and requires an
  expected revision. The host adapter delegates to `SettingsService`, and the cache admin Leptos
  transport/UI keeps the loaded revision and advances it only after a successful save.
- `apps/admin` email settings and the Next.js email/cache admin packages were updated in lockstep:
  queries/mutations select `schemaVersion`, forms store the loaded revision, and saves send
  `expectedSchemaVersion`. There is no client-side blind-write fallback.

Regression coverage:

- SQLite service test: revision `0` insert, sequential revision increments, stale expected revision
  conflict, stored settings unchanged after conflict, and negative revision rejection.
- Cache host adapter tests assert revision increments and that the invalid-mode update uses the
  loaded revision.
- Cache port recording test asserts a stale update is rejected before recording a write.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only:
tree-sitter parse of changed Rust files (two pre-existing unrelated ERROR nodes), `git diff --check`,
remediation diff invariants, the runtime-context/auth-admin-tenant-scope guards, and the updated
server platform-settings tenant-scope guard (now asserting the snapshot/CAS GraphQL contract).

## P7 — OAuth deployment gate and authorization-code retention

The `features.oauth_enabled` setting previously deserialized but did not affect runtime composition:
OAuth endpoints and discovery metadata were mounted even when the feature was disabled. The host now
treats the flag as a real fail-closed composition gate:

- `build_application_router` mounts `controllers::oauth` and `controllers::oauth_metadata` only when
  `settings.features.oauth_enabled` is true.
- OpenAPI prunes `/api/oauth/*` when the gate is closed, so the published contract matches the mounted
  surface.
- Router tests assert closed-gate 404s for token/revocation/discovery paths and an open-gate mounted
  token endpoint. An OpenAPI test asserts pruning and restoration.

Auth retention also now purges expired `oauth_authorization_codes`. A used code is the replay anchor
for the delegated refresh lineage it issued, so codes are retained on the same 120-day outer window as
OAuth token families rather than the shorter 30-day session/invite window. The retention worker reports
the new per-table count, and a SQLite regression test proves old codes are removed, recent used codes
survive, and a second purge is idempotent.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only:
tree-sitter parse of changed Rust files (two pre-existing unrelated ERROR nodes), `git diff --check`,
remediation diff invariants, and the existing runtime-context/auth-admin-tenant-scope/platform-settings
guards.


## P8 — OAuth access-token revocation fence

OAuth access JWTs now carry a `jti` bound to a durable `oauth_tokens` access row
(`token_kind = access`). The extractor requires an active, hash-bound, client/tenant/grant/
subject-scoped row, so RFC 7009, app/consent revocation, credential change, code replay, and
refresh reuse fence already-issued access JWTs before their signed TTL. Delegated rows capture
`consent_granted_at`; consent revoke/re-grant cannot resurrect older credentials. Refresh-only
queries filter `token_kind = refresh`, and admin active-token counts remain refresh-lineage counts.

Regression coverage:

- SQLite RFC 7009 test revokes an access registration and its refresh family by access hash.
- Extractor tests reject unregistered and revoked OAuth access registrations.
- Existing refresh rotation / code replay tests now include access rows and the new columns.

Verification limits: no Rust toolchain or database runtime is available in this workspace, so
`cargo check` / `cargo test` and the remediation gate remain pending. Static verification only.

## PR integration and verification handoff (2026-11-02)

The P1–P8 changes were prepared from base
`0aaf455a45556f4e39a5ed66b8a1184369e5e93f`. Before opening the draft PR, `main` was
refreshed to `a418bbbed64e53cdf841b87dada7ae5799770cd2` and merged into the task branch.
The integration preserves the upstream runtime/readiness changes, ADR registry repairs,
and admin contract tests. No audit-ledger advancement is part of the task delta.

The upstream cache/email admin fixtures now exercise the versioned settings contract:
they submit `expectedSchemaVersion`, retain the returned revision when normalizing invalid
JSON, propagate conflicts, and reject a missing revision instead of inventing a CAS baseline.
The settings source guard checks both transactional event publication and the CAS entrypoint.

Executed local evidence:

- `node --test apps/next-admin/packages/cache/src/cache-admin.test.mjs apps/next-admin/packages/email/src/email-admin.test.mjs` — 19 passed, 0 failed.
- `npm run verify:docs` — passed.
- `node scripts/verify/verify-adrs.mjs` — passed for 164 decisions.
- Runtime-context, auth-admin tenant-scope/boundary, explicit RBAC principal-kind, and
  platform-settings tenant-scope source guards — passed.
- Task delta against refreshed `main`: `git diff --check` and remediation diff invariants — passed.

Still blocked locally: cargo check/test/clippy/format, Rust crate contract tests, full Rust
remediation/architecture gates, admin typechecks/builds, and PostgreSQL/Redis fault injection.
The environment has no Cargo/Rust toolchain, PostgreSQL, Redis, Docker, or installed frontend
dependencies. These are CI/maintainer gates, not claimed passing results. The PR remains a
draft until the affected crate and integration checks have actually run.

Deferred work remains the service-token TTL policy, the cross-crate OAuth code-reuse domain
event, deduplication of `RbacService::record_claim_role_mismatch()`, and database/Redis
failure-path evidence. The historical source-guard/ADR failures recorded in earlier batches
were resolved by the refreshed `main`; the current runs above pass.


## P9 — Empty module listener registry routing (2026-11-18)

A worker host must bind the module listener registry even when the composed
module set registers zero handlers. Previously the host treated an empty set as
a reason not to start the dispatcher. That left the configured transport unable
to distinguish an intentionally empty registry from a dispatcher that had not
initialized: remote-only sealed families were refused before reaching Iggy, and
the single-node outbox target had no local receiver for root events.

### Implemented

- Worker hosts now start the dispatcher for both populated and empty listener
  sets. The empty registry is explicit and remains part of readiness; it is not
  represented as a missing worker.
- Required local sealed families still fail closed when no matching handler
  exists. Remote-only families can be sent to the configured primary when the
  registry is explicitly empty.
- Added regression coverage for empty-registry startup and remote-only delivery.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::module_event_dispatcher
cargo test -p rustok-server --lib services::event_transport_factory
```



## P10 — Unique notification lease-owner identities (2026-11-18)

The server adapters previously derived the distributed lease owner for
notification fanout and candidate work from a process-local atomic counter.
Every replica therefore started with the same owner string (for example,
`notification-fanout-1`). The owner services fence completion by comparing
that string and the lease expiry. If a slow worker outlived its lease and another
replica reclaimed the row with the same owner string, the stale worker could
mistake the new lease for its own and commit after the reclaim.

### Implemented

- Fanout and candidate lease-owner IDs now include a fresh UUID for each worker
  instance. The local atomic instance ID remains only for host-local handle
  identity and diagnostics.
- Added regression tests proving that two worker instances with the same local
  counter value receive different lease-owner IDs and remain within the owner
  contract's length limit.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::notification_fanout_worker
cargo test -p rustok-server --lib services::notification_candidate_worker
```



## P11 — Comments worker readiness visibility (2026-11-18)

The Comments TCP listener and schedule-audit handoff worker already had bounded
shutdown drains, but neither appeared in the runtime worker snapshots. A task
could therefore terminate unexpectedly while the HTTP health endpoint remained
green.

### Implemented

- The optional Comments TCP listener and schedule-audit handoff worker are now
  included in readiness whenever their handles are present.
- Added a lifecycle regression test that verifies both workers are required
  while running and are removed by the normal shutdown drain.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib --features mod-comments
cargo test -p rustok-server --lib services::app_lifecycle::lifecycle_tests::comments_listener_and_audit_handoff_workers_are_required_when_started
```



## P12 — Notification intake versus Outbox retention (resolved by P18)

The original failure boundary was real: Notifications persisted accepted
receipts and permanent rejections in owner tables while Outbox pruned dispatched
`sys_events` rows solely by age. A stalled intake worker could therefore lose
its source envelope before recording a terminal outcome.

P18 resolves the cross-owner boundary with Outbox-owned durable consumer
registrations and per-event acknowledgements. The Outbox owner does not query
notification-owned tables, and Notifications does not mutate relay state.
See the accepted [consumer-retention decision](../../../DECISIONS/2026-11-18-outbox-consumer-retention-acknowledgements.md).

The guarantee applies to rows that still exist when the consumer registration
is committed. Already-pruned historical events cannot be reconstructed; old
terminal notification receipts are backfilled before retention starts.



## P13 — Acknowledge root-event projections before outbox completion (2026-11-18)

The sealed contract path already awaited matching module handlers, but root
`DomainEvent` delivery only broadcast into an in-memory channel. The outbox
relay could mark a row dispatched before root projections completed; handler
errors after the configured retry budget were logged and swallowed. In the
Iggy profile, a local root-projection failure was also swallowed after remote
acceptance, leaving no durable local retry.

### Implemented

- Added an acknowledged root-delivery path through the existing explicit module
  listener registry. It waits for every matching handler and propagates handler,
  capacity, and shutdown failures to the outbox relay.
- The single-node `outbox` profile now uses the acknowledged local target.
  The `outbox_iggy` profile also returns local projection failures so the
  canonical outbox envelope is retried unchanged.
- An unavailable local registry is rejected before irreversible remote
  publication. A projection failure after remote acceptance may still cause
  the same envelope to be published again; remote consumers must be idempotent.
- Added regression coverage for handler completion, projection failure,
  explicit empty registries, primary refusal, and retry with the same event ID.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::contract_delivery
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_transport_factory
```



## P14 — Retry and graceful shutdown for the in-process event forwarder (2026-11-18)

The server event forwarder previously consumed an in-memory event and only
logged a transport error. A transient database failure therefore lost the
event before it reached the durable outbox. Backpressure accounting also had
no release path for forwarded events or lagged messages, and the configured
queue depth could exceed the Tokio broadcast ring capacity, allowing the ring
to drop events before backpressure rejected new publishers. Shutdown aborted
the forwarder immediately, discarding queued or in-flight events.

### Implemented

- The forwarder retries the same envelope with bounded exponential delay until
  the transport accepts it; transport panics are caught and retried as well.
- Root OutboxTransport publication is idempotent by envelope ID and verifies
  exact canonical facts before accepting a duplicate, covering ambiguous
  database commit responses.
- Backpressure permits are released after persistence, for lagged messages,
  and when queued messages are discarded during task teardown.
- The effective backpressure depth is clamped to the broadcast capacity, and
  permit acquisition uses compare-and-swap so concurrent publishers cannot
  exceed the configured critical depth.
- Shutdown signals the forwarder to drain accepted messages and allows up to
  30 seconds before forced abort. P17 orders this dedicated drain after producer
  workers have joined.
- Added regression coverage for transient transport failure, shutdown draining,
  permit release, concurrent backpressure acquisition, and idempotent outbox
  retries.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::backpressure
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_bus
cargo test -p rustok-server --lib services::app_lifecycle
cargo test -p rustok-outbox --test sqlite_transport_regression_test
```



## P15 — Event forwarder readiness visibility (2026-11-18)

The event forwarder was supervised and drained during shutdown, but it was not
included in the runtime worker readiness snapshot. If its supervisor exited,
the HTTP host could remain green even though the in-memory-to-outbox bridge was
no longer running.

### Implemented

- A started event forwarder is now a required worker in local runtime readiness.
- Readiness reports it as finished if its supervisor task has terminated.
- Added a lifecycle regression assertion for the forwarder's required/live
  snapshot state.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::app_lifecycle::tests::shutdown_aborts_invalidation_and_event_supervisors_even_with_live_clones
```



## P16 — Cross-cutting event-delivery invariants documented (2026-11-18)

Codified the recurring failure patterns found in the server audit in
`docs/standards/logic-hygiene-patterns.md`: replica-unique lease ownership,
atomic backpressure reservation, durable idempotent forwarding, acknowledged
projection delivery, and cooperative shutdown. Each invariant now includes a
specific regression check so future changes can preserve the same guarantees.


## P17 — Close event-forwarder admission and shutdown races (2026-11-18)

The P14 bridge still had two loss windows: with the default
`backpressure.enabled = false`, the Tokio broadcast ring could overwrite
accepted events while the forwarder retried a failed outbox write; and the
shared stop signal let the forwarder drain concurrently with producers that
were still completing in-flight work. A third startup race allowed the first
publisher to run before the spawned forwarder had subscribed.

### Implemented

- Install a hard admission controller for the server-owned EventBus-to-Outbox
  forwarder bus, including when configurable backpressure is disabled. The hard
  depth equals the broadcast capacity; enabling backpressure adds configurable
  early thresholds and a possibly smaller depth.
- Honor configured `channel_capacity` even when EventBus is requested before
  EventRuntime has been composed.
- Subscribe the initial forwarder receiver synchronously before returning the
  EventBus to its first publisher; supervised restarts create a fresh receiver.
- Give the forwarder a dedicated stop signal. The global stop first quiesces
  producers; the forwarder is stopped and drained only after all other
  registered worker drains have joined.
- Add regressions for default hard admission, first-publish subscription, and
  producer-first shutdown draining.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::event_bus
cargo test -p rustok-server --lib services::app_lifecycle
```


## P18 — Retain Outbox events until required consumers acknowledge (2026-11-18)

The Notifications intake worker previously wrote terminal outcomes into
notification-owned tables, while Outbox retention deleted dispatched source
events by age alone. A prolonged intake outage could therefore remove an event
before its notification receipt or permanent rejection existed.

### Implemented

- Added Outbox-owned `sys_event_consumers` registrations and
  `sys_event_consumer_receipts` acknowledgements. Retention now deletes a
  dispatched event only after every registered consumer for its event type has
  acknowledged the canonical event ID.
- Added an append-only platform migration and shared standalone schema helper.
  Receipt rows cascade with the canonical Outbox event.
- Notifications registers its supported event types before the server starts
  runtime workers and retention. A neutral receipt port commits the Outbox
  acknowledgement in the same transaction as the accepted intake receipt or
  permanent rejection; retryable failures create neither terminal receipt nor
  acknowledgement.
- Added bounded startup backfill for existing terminal Notifications receipts
  before retention starts. Already-pruned IDs are harmless because the owner
  terminal receipt is already durable.
- Added regression coverage for multi-consumer retention holds, idempotent
  acknowledgements, and atomic notification receipt/Outbox acknowledgement.
- Registration is durable and additive. Disabling intake does not silently
  release its retention obligation; unacknowledged events remain stored until
  intake recovers or an operator explicitly retires the consumer after drain.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-outbox-storage --lib
cargo test -p rustok-outbox-storage --lib consumers
cargo check -p rustok-outbox --lib
cargo test -p rustok-outbox --lib retention
cargo check -p rustok-notifications --all-targets
cargo test -p rustok-notifications --test outbox_intake_sqlite
cargo check -p rustok-server --lib --features mod-notifications
cargo test -p rustok-server --lib services::notification_outbox_intake_worker
cargo test -p rustok-migrations
node scripts/verify/verify-notifications-outbox-intake.mjs
node scripts/verify/verify-outbox-retention-and-event-metrics.mjs
node scripts/verify/verify-migration-backfill-contracts.mjs
```

PostgreSQL migration and concurrent retention/acknowledgement behavior remain
maintainer/CI gates.


## P19 — Release backpressure reservations on dispatcher lag (2026-11-18)

The module dispatcher handled `broadcast::RecvError::Lagged` by advancing the
receiver and continuing, but did not release the backpressure reservations for
the skipped envelopes. Under a slow handler, the broadcast ring can overwrite
events; those accepted-but-lost messages therefore remained counted forever and
could eventually make the backpressure controller reject healthy publishers.

### Implemented

- Release one EventBus backpressure reservation for every skipped envelope when
  the module dispatcher observes broadcast lag.
- Clamp dispatcher semaphore capacities in the legacy `start()` path so
  externally supplied oversized `DispatcherConfig` values cannot panic inside
  the spawned task.
- Use saturating retry-attempt arithmetic so `retry_count = usize::MAX` cannot
  overflow before handler execution.
- Added a regression test proving lagged reservations return the controller to
  zero after the retained event completes.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo test -p rustok-core --lib events::handler
```



## P20 — Surface core runtime worker failures in readiness (2026-11-18)

The readiness review identified the Outbox relay, Outbox retention,
remote-executor reaper, and SEO workers as critical runtime obligations.
The health controller already owned explicit, configuration-aware checks for
these workers. Mirroring them in `additional_runtime_worker_snapshots` was
redundant and emitted duplicate Outbox/remote-executor checks plus a second,
non-canonical SEO worker label. P21 records and corrects that implementation
mistake; the explicit checks in `health.rs` remain authoritative.

### Implemented

- Kept the existing explicit checks for Outbox relay/retention,
  remote-executor reaper, and each enabled SEO worker as the single source of
  readiness truth.
- Kept the event-forwarder snapshot in the additional worker list because it
  has no separate explicit check; it becomes required when the EventBus
  forwarder handle is registered.
- Added a regression test that rejects duplicate readiness check names and
  asserts one canonical check for each core worker.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib controllers::health::tests::runtime_worker_readiness_uses_one_canonical_check_per_worker
cargo test -p rustok-server --lib services::app_lifecycle::tests::additional_workers_respect_host_modes_and_auth_retention_config
cargo check -p rustok-server --lib --features commerce-marketplace-financial
cargo check -p rustok-server --lib --no-default-features --features mod-seo
```


## P21 — Keep runtime worker readiness single-source (2026-11-18)

A source-level re-review found that `controllers/health.rs` already creates
configuration-aware checks for the Outbox relay, Outbox retention,
remote-executor reaper, and SEO worker variants. The additional lifecycle
snapshot list had duplicated those checks. The extra Outbox/remote entries
reused the same check names; the SEO entry used a second name for the same
supervised handle. Duplicate readiness entries can confuse operators and
produce repeated worker metrics.

### Implemented

- Removed the redundant Outbox relay, Outbox retention, remote-executor, and
  SEO entries from `additional_runtime_worker_snapshots`; the existing
  `health.rs` checks remain the only canonical entries for those workers.
- Retained the EventBus forwarder snapshot, which has no separate explicit
  check, and classified it as optional until its handle is registered.
- Removed a duplicate feature-gated import that would fail compilation when
  `commerce-marketplace-financial` is enabled.
- Removed the redundant SEO snapshot (which had been gated by both
  `mod-workflow` and `mod-seo`); the canonical SEO readiness checks remain
  independently gated by `mod-seo`.
- Added a regression test asserting readiness check names are unique and that
  Outbox, remote-executor, and SEO worker checks each have one canonical entry.
- Updated the host-mode snapshot test to treat EventBus forwarding and optional
  Comments workers as optional until their runtime handles are registered.
- Updated the readiness runbook to document Outbox retention, EventBus
  forwarder, and the three independently configured SEO worker checks.

### Verification status

Not run in this task, at the user's request. No Rust compilation, formatting,
or tests are claimed. Run the commands listed under P20 on one revision before
integration; tests remain with the maintainer.

## P22 — Atomic tenant-scoped OAuth manifest reconciliation (2026-11-18)

The manifest sync previously committed app upserts and translation writes
individually, then used a separate transaction for orphan cleanup. A conflict
or storage failure late in the pass could leave a tenant with a partially
updated OAuth application set. Concurrent server replicas could also perform
the same read/upsert/orphan sequence without a tenant-level serialization fence.

### Implemented

- The sync_app_connections flow now acquires an exclusive tenant-row lock before
  reading the current managed-app set. PostgreSQL/MySQL use FOR UPDATE;
  SQLite takes its write fence before the first read. Existing managed-app
  rows are then locked with the same app-mutation fence used by token issuance
  and consent before security-sensitive configuration is updated.
- App creation/reactivation, first-party manifest updates, translation writes,
  orphan deactivation, token revocation, consent revocation, and authorization-code
  invalidation now share one transaction per tenant.
- A failure in any manifest entry or orphan cleanup rolls back all earlier
  writes from that tenant's reconciliation.
- Reconciliation includes already-inactive managed apps, repairs stale credentials
  left by older versions, and clears credentials again before reactivating a
  previously disabled app. Embedded-app reactivation also checks the combined
  active/revoked state instead of the raw is_active column alone.
- Explicit manual app revocation uses the same credential cleanup path and
  revokes active user consents alongside tokens and pending authorization codes.
- Added a regression case where an embedded app is upserted before a later
  manual-app slug conflict; the earlier embedded-app write must not leak.
- The all-tenants bootstrap remains a sequence of per-tenant transactions,
  not a single global transaction across every tenant. A failure can leave
  earlier tenants committed while later tenants have not yet been processed;
  rerunning the idempotent sync reconciles those tenants.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-server --lib services::oauth_app::tests::manifest_sync_refuses_to_take_over_a_manual_app_with_a_matching_slug
cargo test -p rustok-server --lib services::oauth_app::tests::app_revocation_revokes_active_user_consents
cargo test -p rustok-server --lib services::oauth_app::tests::manifest_sync_reactivates_embedded_app_with_revocation_timestamp
cargo test -p rustok-server --lib services::oauth_app::tests::sync_app_connections_creates_manifest_managed_apps_and_deactivates_orphans
cargo test -p rustok-server --lib services::oauth_app::tests::orphan_deactivation_database_failure_is_not_silently_ignored
```

## P23 — Unique installer lease-owner identities (2026-11-18)

The HTTP adapter used the shared literal owner `http`, while the CLI used the
shared literal `rustok-cli install apply`. Those labels were not unique to an
execution attempt. The persistence layer records the owner at acquisition but
does not yet compare it on every state/receipt write, so unique values alone
are diagnostic/preparatory rather than a complete stale-worker fence.

### Implemented

- Added one shared lock-owner builder in `rustok-installer`. It sanitizes a
  short diagnostic label and appends the optional stable session/job ID plus a
  fresh attempt UUID; the result is bounded below the `VARCHAR(128)` session
  column limit.
- The HTTP adapter now uses its durable job/session ID and a fresh attempt ID.
- The CLI adapter and `InstallApplyOptions::default()` also generate a fresh
  attempt-specific owner rather than a static label.
- Added contract coverage for unique owners across attempts, label
  sanitization, storage length, and unique defaults; added HTTP-adapter coverage.

### Remaining boundary

This is a unique owner token, not a monotonic attempt-generation protocol,
and persistence writes do not yet verify owner equality. The fail-closed
session-admission gate added in P25 prevents automatic overlap after lease
expiry, but the HTTP executor still runs in-process, is not resumable after
process loss, and does not reconcile ambiguous external effects. Stale
`running` jobs must not be reset based only on lease expiry. The durable
recovery ADR remains in progress.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer --lib --features host-runtime install_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_lock_owner_is_unique_per_attempt_and_bounded
cargo check -p rustok-installer-cli
```

## P24 — Bounded installer lease TTL input (2026-11-18)

The HTTP adapter accepted arbitrary `lock_ttl_secs` values while the CLI only
checked positivity. The persistence layer added the TTL to the current
timestamp directly, so an extreme value could overflow the timestamp range.

### Implemented

- Added a shared contract: installer lease TTL must be between 1 and 86,400
  seconds (default 900). The execution state machine validates it even when
  called outside the HTTP/CLI adapters.
- HTTP and CLI adapters reject zero, negative, and over-limit values before
  creating or starting an installer job.
- Persistence now uses checked timestamp addition and returns a database error
  instead of panicking if a direct caller supplies an unrepresentable duration.
- Added shared contract and HTTP-controller regression coverage for both bounds.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer --lib --features host-runtime installer_lock_ttl_is_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_http_lock_ttl_rejects_zero_negative_and_overflowing_values
cargo check -p rustok-installer-cli
cargo test -p rustok-installer-persistence --lib
```

## P25 — Fail-closed installer session admission (2026-11-18)

Lock arbitration previously treated `lock_expires_at` as proof that a worker
had stopped. A new apply could therefore start while a stale executor was
still running. The current persistence adapter does not yet compare owner
identity on every state/receipt mutation, so automatic lease-based takeover
was unsafe.

### Implemented

- A session row can be claimed only once (`lock_owner IS NULL` and
  `status = draft`); a second attempt cannot overwrite its existing owner.
- Any other unreconciled session with a recorded owner blocks a new apply
  even when its lease timestamp has expired. Expiry alone is no longer treated
  as proof that the previous process stopped. `recovery_required` also blocks
  admission until manual reconciliation, despite being terminal for execution.
- PostgreSQL and MySQL lock acquisition now uses a serializable transaction;
  SQLite obtains its write fence through the conditional session-claim update.
- `completed`, safely `failed`, and `fresh_install_cleaned` sessions release
  the global admission gate. `recovery_required` and stale unfinished sessions
  require explicit reconciliation before another apply can start.
- Regression coverage verifies same-session re-claim rejection, expired-lease
  blocking, and release after finalization.

### Remaining boundary

State, tenant, and receipt mutations do not yet compare the persisted owner on
every write. There is no monotonic attempt generation, periodic lease renewal,
resumable executor, or automatic external-effect reconciliation. The fail-closed
gate prevents automatic overlap but intentionally leaves stale sessions blocked
until an operator reconciles them. The durable recovery ADR remains in progress.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo test -p rustok-installer-persistence --lib session_persistence_lifecycle_and_state_transitions
cargo test -p rustok-installer-persistence --lib acquire_lock_rejects_out_of_range_ttl_before_claiming_session
cargo test -p rustok-installer --lib --features host-runtime install_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_lock_owner_is_unique_per_attempt_and_bounded
cargo test -p rustok-server --lib controllers::installer::tests::installer_http_lock_ttl_rejects_zero_negative_and_overflowing_values
cargo check -p rustok-installer-cli
```

## P26 — Serialize first-party refresh-token reuse revocation (2026-11-18)

Refresh-token reuse detection previously read the immediately preceding token
hash without locking its session row, then converted the read snapshot into an
ActiveModel update. A concurrent successful refresh could rotate the row
between those operations; the stale update could then overwrite the newer
token hash or expiry while revoking the session.

### Implemented

- Reused the database-specific previous-session lookup used by logout, which
  acquires `FOR UPDATE` on PostgreSQL/MySQL and a conditional SQLite write
  fence before returning the row.
- Refresh reuse detection now holds that row lock while evaluating the grace
  window and applying the revocation update.
- Session-id logout no longer reads an active session and then updates its stale
  snapshot without a lock. It locks the row on PostgreSQL/MySQL and reserves a
  SQLite write before reading, then publishes the logout event in the same
  transaction as the revocation.
- Added regression tests for an in-grace duplicate (reject without revoking),
  post-grace replay (revoke the live lineage), and failed revocation persistence
  (propagate the storage error and preserve the active session).
- Corrected the grace-window contract for both first-party and OAuth
  refresh: it suppresses revocation for a likely concurrent duplicate but is
  not an idempotent retry. The old token still receives an invalid-token error;
  if the successful response is lost, it cannot recover the replacement
  cookie/token. Full rotation history and idempotent response recovery remain
  open owner-level design gaps.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_inside_grace_is_rejected_without_revoking_live_session
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_after_grace_revokes_live_session_lineage
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_revocation_failure_is_propagated_without_false_success
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_can_find_previous_rotated_token_without_crossing_tenant
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_by_session_id_revokes_under_lock_and_is_idempotent
```

## P27 — Use write-first SQLite fences in auth and OAuth mutations (2026-11-18)

Several auth and OAuth mutation paths used SELECT followed by a no-op
UPDATE to acquire SQLite's writer lock. In a deferred transaction this can
race another writer during lock promotion; the direct profile path also
converted a row read under unsupported SQLite row-lock syntax into a full
ActiveModel update.

### Implemented

- Profile updates now use the shared database-specific user-row lock helper.
- Login user lookup reserves SQLite's writer by the tenant/email key before
  reading the account; user-by-id and current-session mutation helpers also
  reserve the write lock before their first read.
- Refresh and previous-refresh lookups now reserve the SQLite writer directly
  by the presented digest, then reload the matching row while the transaction
  owns the write fence.
- Session-id logout already uses a write-first fence by tenant/session ID.
- OAuth token issuance now reserves SQLite's writer by known app/subject IDs
  before reloading the current client and subject state; it no longer performs
  a read-first lock upgrade on those rows.
- OAuth admin app mutations use the same write-first fence by tenant/app ID;
  admin user mutations fence by tenant/user ID before loading their snapshots.
- Admin user mutations also reserve SQLite's writer by tenant/user ID before
  reading the row used for status, role, password, or profile changes.
- Unsupported database backends now fail closed in these auth lock helpers
  instead of silently running an unlocked read.
- Existing SQLite tests exercise login, profile update, password change,
  refresh/reuse, and logout paths through the new lock helpers.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::auth_lifecycle::tests::failed_attempts_lock_atomically_and_ignore_obsolete_credentials
cargo test -p rustok-server --lib services::auth_lifecycle::tests::update_profile_trims_name_and_allows_clearing_with_blank_value
cargo test -p rustok-server --lib services::auth_lifecycle::tests::change_password_rejects_expired_current_session
cargo test -p rustok-server --lib services::auth_lifecycle::tests::refresh_reuse_after_grace_revokes_live_session_lineage
cargo test -p rustok-server --lib services::auth_lifecycle::tests::logout_by_session_id_revokes_under_lock_and_is_idempotent
cargo test -p rustok-server --lib services::oauth_token_service::tests::refresh_rotation_consumes_a_token_exactly_once
```

## P28 — Revalidate OAuth consent inside token-issuance transactions (2026-11-18)

Authorization-code and refresh flows validated consent before starting their
issuance transaction. They later acquired the app and subject locks, but did
not re-read consent under those locks. A concurrent consent re-grant or scope
narrowing could therefore commit between preflight validation and token
persistence; the exchange could return credentials carrying a stale consent
epoch or scopes that the user had just narrowed.

### Implemented

- Prepared user-token bundles now retain the consent grant timestamp observed
  during preparation.
- Authorization-code exchange revalidates active consent, requested scopes,
  and the prepared grant epoch after acquiring the app and subject locks.
- Refresh rotation revalidates active consent and scope coverage against the
  presented refresh token's stored consent epoch under the same locks.
- A stale epoch returns invalid_grant and a scope no longer covered by the
  current consent returns invalid_scope before the authorization code or
  refresh token is consumed and before replacement credentials are persisted.
- Added a SQLite regression test that changes the consent epoch after token
  preparation and verifies refresh commit rejects without consuming the old
  refresh token or inserting replacement credentials; it also verifies
  narrowed consent rejects wider token scopes.

### Verification status

Not run in this task, at the user's request. Run before merge:

```bash
cargo fmt --all -- --check
cargo check -p rustok-server --lib
cargo test -p rustok-server --lib services::oauth_token_service::tests::locked_consent_revalidation_rejects_stale_grant_epoch_and_scopes
cargo test -p rustok-server --lib services::oauth_token_service::tests::refresh_rotation_consumes_a_token_exactly_once
cargo test -p rustok-server --lib services::oauth_admin_guard::tests::guarded_secret_rotation_checks_tenant_and_delegated_permission_ceiling
```


## P29 — Supervise HTTP installer jobs and retry durable claim finalization (2026-11-21)

The HTTP installer already persisted its job before executing the plan, but the
claim-to-execution task was a raw tokio::spawn. The global shutdown drain did not
own or join it. A transient database error or a lost commit response during job
creation/finalization could also leave a durable running row even when the
corresponding write had committed.

### Implemented

- Added a runtime-owned installer task registry backed by JoinSet. Admission
  and shutdown closure share one mutex fence: a racing request is either
  registered before the drain snapshot or rejected after admission closes.
- Shutdown now waits up to 60 seconds for accepted installer work, then aborts
  and joins remaining tasks. The drain runs with other producers and completes
  before the event forwarder is stopped.
- The installer reservation guard now releases only the matching job ID.
- Job creation retries the same generated ID and reconciles an ambiguous insert
  by reading back the exact persisted running row (ID, session, status, and
  submitted timestamp).
- Success and failure terminal writes use bounded exponential backoff and
  read-after-error reconciliation. A terminal state already committed is
  treated as success; a still-running row receives another safe compare-and-set
  attempt.
- Added regression coverage for registry shutdown admission/draining, bounded
  abort, ambiguous create recovery, and idempotent failure finalization.

### Remaining crash-recovery boundary

A hard process crash, or a shutdown timeout during an ambiguous claim write or
after executor entry, can still leave the durable HTTP job in running. The patch deliberately does not replay
an installer automatically: external side effects may have happened even when
the final receipt or HTTP-job transition was not observed. The session's
attempt-generation/receipt fence remains authoritative; an operator must
reconcile durable receipts and session state before releasing the global
installer fence. A separate ADR is still required for a durable job lease,
heartbeat, and safe operator reconciliation workflow.

### Verification status

Not run in this task, at the user's request. Run before merge:

- cargo fmt --all -- --check
- cargo check -p rustok-server --lib
- cargo test -p rustok-server --lib controllers::installer::tests::installer_http_job_creation_reconciles_an_ambiguous_duplicate_insert
- cargo test -p rustok-server --lib controllers::installer::tests::installer_http_job_failure_finalization_accepts_an_already_committed_state
- cargo test -p rustok-server --lib services::app_lifecycle::installer_job_task_registry_tests::installer_job_tasks_are_drained_and_admission_closes
- cargo test -p rustok-server --lib services::app_lifecycle::installer_job_task_registry_tests::stuck_installer_job_tasks_are_aborted_after_the_grace_timeout


## P30 — Prevent catch-up bursts in maintenance timers (2026-11-21)

A final pass over periodic server maintenance loops found two intervals still
using Tokio's default Burst missed-tick policy: Redis status monitoring and
rate-limit cache cleanup. If either operation ran longer than its interval,
the next loop iterations could execute immediately to catch up, adding load
precisely while the dependency or cache was slow.

- Set both timers to MissedTickBehavior::Delay. Their first-tick behavior is
  unchanged; after a delayed run, the next poll is scheduled relative to the
  completed tick instead of replaying missed intervals.
- Other audited periodic workers already use Delay or Skip, or use a sleep
  after each completed cycle; no retry/replay semantics were changed.

Not run at the user's request. Verify with cargo fmt --all -- --check and
cargo check -p rustok-server --lib before merge.


## P31 — Bound Comments TCP listener concurrency and accept retries (2026-11-21)

The opt-in Comments TCP listener accepted an arbitrary positive connection
limit and passed it directly to Tokio's semaphore constructor. Tokio panics
when the permit count exceeds its supported maximum, so a configuration typo
could silently turn a successful bootstrap into a failed listener task. The
accept loop also retried persistent TCP accept errors immediately, producing
a CPU and logging hot loop for conditions such as descriptor exhaustion.

- Added explicit bounds for 1..=128 concurrent connections and 1..=8 MiB
  frames, preventing an operator setting from multiplying unbounded per-frame
  allocations across thousands of connections.
- Bounded the pre-request timeout and shutdown grace to 1..=60,000 ms, and made
  the runtime join budget equal to the configured grace plus a five-second
  abort/join margin.
- Added exponential accept-error backoff from 100 ms to 5 seconds.
- The backoff wait is interrupted immediately by the shared shutdown signal.
- Added tests for the configuration bounds, retry progression/cap, and shutdown
  interruption.

Not run at the user's request. Verify with cargo fmt --all -- --check and
cargo check -p rustok-server --lib before merge.


## P32 — Make Comments worker startup cancellation-safe and truthful (2026-11-21)

A lifecycle pass found two opt-in Comments workers that did not follow the
server's shared startup-reservation contract. The TCP listener manually removed
its marker only after a returned error, so cancellation while awaiting bind
could strand the marker permanently. Both the listener and source-retry handoff
worker returned `Ok(())` to a competing starter while the first startup was
still in progress. They also admitted a task after shutdown had already been
signalled, and an already-finished task handle was treated as an idempotently
running worker.

- Replaced manual marker cleanup with `ServerRuntimeStartReservation`; failed
  or cancelled preparation releases only its own claim, while successful
  publication retains the process-local owner marker.
- Concurrent callers now return an explicit startup-in-progress error with
  retry guidance unless a live handle has already been published.
- Both workers reject startup after the shared stop signal and recheck it
  immediately before spawning.
- A finished published task is reported as exited, not as a successful
  running worker; automatic restart remains intentionally unsupported and
  readiness continues to report the failed required worker.
- Corrected the Comments TCP max-connections regression assertion: the bound
  is 128, not the unrelated value 4096.
- Added focused lifecycle regressions for shutdown admission and finished
  task handles. Existing `ServerRuntimeStartReservation` tests cover
  cancellation cleanup and single-owner contention.

Not run at the user's request. Verify with `cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`, and the focused
`comments_provider_runtime_base::tests` and
`comments_provider_runtime_keyring_schedule_audit_handoff_worker::source_retry::source_retry_worker_tests`
suites.


## P33 — Repair Redis rate-limit counters without expiry (2026-11-21)

The Redis fixed-window script set `EXPIRE` only when `INCR` returned 1.
A pre-existing or restored counter key without a TTL therefore remained
permanent: subsequent requests kept incrementing it, while the middleware
reported a one-second retry delay forever.

- The atomic script now checks the key TTL in milliseconds on every increment
  and attaches the configured window whenever the key exists without an expiry.
  Reset and Retry-After values round up to whole seconds, avoiding early retry
  instructions from floor-rounded TTLs.
- If Redis still reports a negative TTL after the repair attempt, the script
  returns an error. The middleware treats that as backend unavailable (503),
  preserving fail-closed behavior instead of silently creating a permanent
  rate-limit lockout.
- Added a regression assertion for the script's expiry-repair and fail-closed
  contract. No Redis server is required by this source-level test.

Not run at the user's request. Verify with `cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`, and
`cargo test -p rustok-server --lib middleware::rate_limit_base::diagnostic_tests::redis_rate_limit_script_repairs_counters_without_expiry`.


## P34 — Preserve verified runner access across artifact download dispatch (2026-11-21)

The composed router validates the host-only runner token in
`registry_artifact_access` and strips the secret header before controllers run.
The artifact-download controller then attempted to validate the removed header
again, so a correctly authenticated remote runner received 401 instead of the
private artifact.

- Added an internal `VerifiedRegistryRunnerAccess` request extension only for
  artifact downloads whose runner token passed constant-time validation.
- The controller trusts that marker, while retaining its user-authority check
  and direct-handler fallback. The shared secret remains absent from controller
  headers and downstream request processing.
- Added an Axum middleware regression test proving that the verified marker
  reaches the download handler while the token header does not.

Not run at the user's request. Verify with `cargo fmt --all -- --check`,
`cargo check -p rustok-server --lib`, and
`cargo test -p rustok-server --lib middleware::registry_artifact_access::tests::artifact_download_passes_verified_runner_context_without_forwarding_secret`.


## Server audit continuation — Comments retry shortcuts and stop-time bootstrap (2026-11-23)

The public Comments audit handoff `claim_next`/`publish_next` convenience path
used a separate source-selection SQL statement. It could acquire deferred or
terminal source-dead-letter rows without applying the worker's configured
attempt budget, bypassing the recovery contract. Both entrypoints now require
an explicit bounded attempt budget and delegate to the same retry-ready claim;
invalid budgets above the canonical maximum fail closed. The publication
read and compare-and-set also fence dead-letter state so a stale claimant
cannot turn an operator-held source record into a published row. Audited
`requeue_dead_letter` remains the only path to clear that state.

The common published-worker liveness check also previously returned success
for a live handle after shutdown had been signaled. Repeated startup now
rejects that state without removing the join handle needed by shutdown.

Source-level regression assertions were added for the PostgreSQL publication
fences and the repeated-bootstrap shutdown state. No Cargo checks, tests,
formatters, SQL execution, or multi-replica race tests were run, as requested.


## Server audit continuation — required Index startup and Auth retention retries (2026-11-23)

Product Index refresh's public worker starter called the panic-on-missing
`module_runtime_extensions_from_ctx` accessor even though an optional enable
flag and independently built event runtime can expose it during partial
bootstrap. The starter now returns an explicit configuration error if the
runtime extensions were not published, without opening a consumer group.
A focused isolated-context regression proves this missing-dependency branch.

Auth retention's successful daily poll interval was also used after a failed
transactional purge. A temporary PostgreSQL outage therefore delayed the next
cleanup attempt for 24 hours. Failed sweeps now retry after 15 minutes, while
successful sweeps keep the daily cadence. The error log uses a fixed machine
code rather than interpolating raw DB error details. The port's atomic
multi-table transaction and cooperative shutdown behavior are unchanged.
A regression exercises success and failure delay choices.

No Cargo compilation, tests, formatting, lints, live PostgreSQL or concurrency
checks were run, per the maintainer's instruction.


## Server audit continuation — Outbox retention recovery cadence (2026-11-23)

The Outbox retention supervisor treated failed PostgreSQL prune attempts as
an empty successful sweep. When its configured successful poll interval was
large (up to 24 hours), transient storage outages postponed the next attempt
by that full interval. The worker now distinguishes failed, complete and
batch-exhausted cycles. Failure retries after `min(interval, 60s)`, while
successful and full-batch timing remains unchanged. The existing atomic failure
counter and stop-aware sleep are retained. A focused delay-selection regression
was authored but not executed; no Cargo or SQL verification was run.


## Server audit continuation — Pages cache duplicate scope (2026-11-23)

The server's Pages invalidation adapter previously reused a single static
`BoundedCacheEventDedupe` for every `CacheService` in the process. Separate
host/runtime cache instances have independent namespace-generation stores,
so the same event ID could mark the first context's invalidation completed
while the second context in the same process was never bumped. That second
port could fail receipt validation because its generations remained zero.
The dedupe registry now follows the clone-shared generation-store owner
identity, with weak references, a bounded number of live owners, and
a fail-open-to-extra-rotation fallback when full. Ports sharing one owner
serialize duplicate events, without constructing a value backend.
A regression for two independent services and two ports sharing one service
was authored but not executed. No compilation, tests, formatting, or live
cache checks were performed, per maintainer instruction.


## Server audit continuation — Shutdown-aware host readiness (2026-11-23)

The readiness handler relied on required worker snapshots to report the
shutdown state. A registry-only host has no required workers, and some
surface configurations may disable all worker checks; `/health/ready` could
continue returning 200 while the host was already draining. Readiness now
includes an unconditional critical `runtime_shutdown` check, sampled after
the asynchronous dependency probes so a stop requested during a slow probe
is also observed. Liveness (`/health/live`) is intentionally unchanged.
A focused registry-only shutdown regression was authored, not run.
No compilation, formatting, integration tests, or runtime verification
were performed.


## Server audit continuation — Readiness circuit ownership (2026-11-23)

The health controller previously stored all dependency-check circuit breakers
in one process-global static map keyed only by check name (e.g. `database`).
Distinct `ServerRuntimeContext` instances could therefore share a failure
counter and a 30-second open circuit even when their dependencies were
independent. A healthy host could report false 503 after another host reached
the failure threshold. Breaker state now belongs to the shared runtime
context; cloned contexts keep the same state, independent contexts do not.
The per-check failure counter is also saturating. A focused two-context
regression was authored but not run. No compilation, tests, formatting, or
live dependency checks were run at the maintainer's request.


## Follow-up code review — Pages cache identity correction (2026-11-23)

Review detected that the first Pages dedupe patch accidentally initialized
the value backend from `invalidate()` just to acquire its identity. This
unnecessary cache construction coupled generation rotation to a separate
read/write-backend initialization. The corrected adapter instead uses the
opaque `CacheService::generation_store_identity()` that is already the
generation-store owner. It leaves cache values lazy; clone-shared contexts
continue to share dedupe, and independent services do not. A regression for
zero value-backend constructions and a concurrent two-port regression were
authored, but not executed. The owner API is documented in
`crates/modules/rustok-cache/CRATE_API.md`.


## Server audit continuation — installer HTTP crash quarantine (2026-11-24)

The HTTP adapter now heartbeats the host-runtime job row every 30 seconds and
atomically quarantines a five-minute stale `running` row as
`recovery_required`. The new unique expression index enforces one unresolved
job across both `running` and `recovery_required`; migration ordering keeps
the previous running-only guard installed if legacy duplicate unresolved rows
prevent the stricter index from being created. Heartbeat, stale reconciliation
and terminal transitions are conditional so a late worker cannot revive or
overwrite the quarantine.

Execution errors now carry a typed recovery classification across the server
executor adapter. Failures after filesystem/database preparation, owner-schema
work, session creation/claim, receipt persistence, state transitions, or other
ambiguous effects are quarantined. The installer state machine also closes
the direct `AdminProvisioned -> Verified` deployment bypass and disallows new
transitions into legacy `failed`; errors are not mislabeled
`fresh_install_cleaned` without actual cleanup. Global status projects the
host job when the target session is not in the runtime database, and receipts
return an explicit conflict when the target session is remote.

This remains interruption detection and fail-closed admission, not automatic
resume or target-side reconciliation. The host job ledger and target session/
receipt database can be different, and the host does not retain a safe
reconnectable target-secret reference. Therefore a quarantined job must be
reconciled by an operator against the actual target database before retrying.
No stale job is reset merely because its lease or heartbeat expired.

Source regression coverage was added for state transitions, error
classification, heartbeat CAS, stale-job quarantine, public status mapping,
recovery finalization, and the migration's cross-status single-slot invariant.
No tests, compilation, formatting, migration execution, or multi-replica
exercise were run at the user's request.


## Follow-up code review — GraphQL storage probe and module codegen inputs (2026-11-24)

The GraphQL `systemHealth` resolver still used `Uuid::nil()` to build an
`ObjectKey::chronological` staging key and then called `expect`. The canonical
key constructor rejects nil object identities, so every host-authorized
`systemHealth` request with `mod-media` and configured storage could panic.
HTTP readiness had already moved to a non-nil per-runtime key, but the GraphQL
path had not. Both surfaces now use one shared service with a stable
`ServerRuntimeContext`-scoped UUID, a shared async operation lock, and
best-effort cleanup after ambiguous writes. A focused regression checks that
cloned contexts share the identity/lock and independent contexts do not.

The server build script also derived generated module wiring from the
existence of `rustok-module.toml`, `src/graphql/mod.rs`, and
`src/graphql.rs`, but did not watch those inputs when absent. Adding a package
manifest or fallback GraphQL entrypoint could therefore leave generated
registry/schema/router wiring stale until a clean build. The build script now
registers all three paths as rerun inputs before checking their existence.

Source review and regression definitions are complete. Tests, compilation,
formatting, and migration/runtime checks were not run, as requested by the
maintainer.


## Follow-up code review — artifact fingerprint blocking I/O (2026-11-24)

The artifact runtime computed the running executable's SHA-256 synchronously
inside an async runtime-composition path. Because the executable can be as
large as the configured 1 GiB bound, metadata/open/read operations could occupy
a Tokio worker for a prolonged period and delay unrelated timers, readiness
and startup work. The digest operation now runs as one bounded
`spawn_blocking` task, and join failures are converted into a typed host
composition error. The synchronous helper remains private and is only invoked
through the blocking-pool wrapper.

No tests, compilation or formatting were run, as requested by the maintainer.


## Follow-up code review — release runtime configuration contract (2026-11-24)

The reproducible release image did not set `RUSTOK_HTTPS=true`, although
production host startup rejects the deployment unless HSTS is explicitly
enabled. It also copied the entire archive config directory into the image,
including development, test and browser-smoke YAML containing known local
credentials. The release workflow now builds a runtime context containing only
the binary and Dockerfile; the image creates an empty operator-owned
`/app/config` and explicitly sets `RUSTOK_ENV=production`,
`RUSTOK_CONFIG_DIR=/app/config`, and `RUSTOK_HTTPS=true`.

The binary release archive previously copied every YAML file. It now includes
only the placeholder production template as `config/production.yaml.example`
and a README that explains how to create `production.yaml`, replace secrets,
and configure HTTPS/Redis. Its self-test checks that development, test and
browser-smoke files do not enter the archive. The release-image verifier now
locks these runtime and packaging contracts.

Tests and release-image builds were not run, as requested by the maintainer.


## Follow-up code review — RBAC invalidation publisher ownership (2026-11-24)

The RBAC listener stored its `CacheService` in a process-global
`Lazy<RwLock<Option<CacheService>>>`, while `ensure_cache_service` owns one
service per `ServerRuntimeContext`. If two independent contexts initialized
listeners, the last one overwrote the publisher owner. Mutations in the other
context could therefore fan out through the wrong local invalidation bus or
Redis client and wait for durable-generation reconciliation to converge.

User and namespace-wide publishers now take an explicit `&CacheService`.
The auth-admin provider and GraphQL role writer receive the cache service from
their own runtime context; committed mutation and role-repair APIs also require
the owner explicitly. The listener still subscribes before recovery and
publishes its supervised runtime only after recovery. A focused unit test
checks that publishing through one cache owner does not notify an independent
owner, and the architecture guard rejects reintroduction of the global pointer.

Tests and compilation were not run, as requested by the maintainer.


## Follow-up code review — release-aware CORS and Redis config parity (2026-11-25)

The main API router passed the environment-label predicate into the HTTP edge
stack even though CORS origin resolution and host startup use the shared
release-aware production-security mode. A release build labeled
`staging`/`development` could therefore receive the development loopback
allow-list. Router composition now uses `is_production_security_mode()`
for the edge stack.

Rate-limit settings validation also checked only environment Redis URLs,
while `CacheService::from_url` already honors `rustok.cache.redis_url` first.
YAML-only Redis deployments were rejected before runtime composition. Validation
now accepts a nonblank configured cache URL before checking environment fallback,
and a focused regression covers that configuration.

The 404 fallback and embedded-admin URI rewrite failure path now log only the
fixed HTTP path family. Static admin asset responses include the selected
representation length for both GET and HEAD; HEAD keeps an empty body. Focused
tests were authored for Redis-only YAML configuration and HEAD metadata.

Tests, compilation and formatting were not run, as requested by the maintainer.

## Server follow-up — installer heartbeat ownership and search limiter readiness (2026-11-25)

The installer heartbeat now runs as a sibling future inside the runtime-owned
executor task rather than a detached Tokio task. Completion or forced
cancellation drops the heartbeat even while its database call is pending.
Heartbeat termination or panic does not cancel an admitted installer operation;
the existing status CAS and quarantine policy remain unchanged. A database
operation already submitted before cancellation may still commit, but cannot
revive a non-running row.

Readiness now checks the independent search rate limiter alongside API, Auth
and OAuth when rate limiting is enabled on applicable host profiles. Missing
search limiter state is a critical readiness failure, as with other limiter
namespaces. This does not add a search indexing requirement.

Focused source regressions cover heartbeat cancellation/completion, continued
execution after heartbeat termination, and search limiter readiness. Tests,
compilation and formatting were not run at the maintainer's request.

## Server follow-up — OAuth confirmation scope binding and session cookie failure (2026-11-25)

OAuth browser confirmation now authenticates the effective scope set in
addition to the original optional scope parameter. If a client policy change
expands or narrows omitted/default scopes between GET and POST, the old form
is rejected and the user must confirm again. Reordering or duplication of
equivalent scope entries does not invalidate the set. Code issuance retains
its transactional app/user/consent checks; the MAC version bump intentionally
invalidates outstanding v2 forms. Issuance failure logs use a bounded error code.

First-party login, registration, refresh and logout responses use one
fallible Set-Cookie attachment path. Header encoding failures return an
internal error instead of falsely reporting success; generated credential
headers are marked sensitive. This does not roll back a committed auth
mutation if response construction fails.

Focused regressions are authored for changed/equivalent scope sets, omitted
versus explicit scope, and invalid/valid session cookie headers. Compilation,
formatting and tests were not run at the maintainer's request.

## Server follow-up — rate-limit runtime ownership (2026-11-25)

Per-context rate limiters now reuse the published namespace Arc across repeat
initialization rather than replacing active counters and splitting middleware,
GraphQL and readiness ownership. A synchronous registration gate covers lookup,
publication and maintenance task registration. Shutdown closes and drains the
same registry even if no limiter has been initialized yet; the closed registry
remains installed and rejects late initializers. Only enabled memory limiters
need Moka maintenance tasks. The runtime bootstrap also rejects a stopped host
before setup and before returning success.

Focused source regressions cover preserved counters, Arc identity, one
maintenance task per namespace, and shutdown before/after registration.
Compilation, formatting and tests were not run at the maintainer's request.

## Server follow-up — single-owner event delivery runtime (2026-11-25)

EventRuntime initialization now serializes construction per host context and
reuses a published runtime rather than replacing its relay/listener bus or
re-reading the desired profile. Shutdown fences direct initialization before
taking worker snapshots. Pending profile/configuration reads are stoppable;
an Iggy connection already being constructed retains its owner startup path.
The durable publisher transport comes from the same atomic resolver used by
early EventBus and transactional callers. Non-transactional or non-outbox
preinstalled transports fail event runtime startup. Registry-only composition
no longer replaces a concurrently registered transport. The operator settings
service is the canonical active-profile reader used by both GraphQL operations.

An already constructed but unpublished Iggy connector is explicitly shut down
on ordinary listener-start failure or stop before publication; this does not
claim cancellation safety inside the connector's own constructor.

Focused source regressions cover reuse without desired-policy reads, stopped
initialization, and active-profile reporting. Compilation, formatting and tests
were not run at the maintainer's request.

## Server audit continuation — global artifact release security API (2026-11-27)

Implemented the missing server boundary for the rustok-modules global artifact
security aggregate. REST now exposes a non-cacheable snapshot read and the
revisioned quarantine, clear-quarantine, and terminal revoke commands; revision
values are decimal strings at both transport boundaries to preserve full u64
precision. GraphQL
exposes the matching snapshot and mutation fields. Both transports require a
direct authenticated user and modules:manage, and the owner adapter binds the
authenticated actor to a tenant-independent command envelope.

The host authorizer revalidates the direct credential and current RBAC grants
inside the owner transaction, taking tenant/user/session/policy/role fences
before the platform release-slug lock and retaining them through commit. The
owner also calls a final host authority check immediately before both new-state
and exact-replay commits; the host rechecks the earliest credential deadline
after slug-lock waits and all writes, rolling back state, receipt and outbox if
the credential has expired. Exact idempotency replay also requires current
authorization. All mutations
retain the owner-issued idempotency receipt and outbox evidence, and successful
changes evict every tenant entry from the process-local effective policy cache.
Marketplace yanking and external-provenance quarantine remain separate controls.

Regression source was added for authorization, command identity, response
cache policy, error mapping, GraphQL field classification and OpenAPI bearer
requirements. Tests, compilation and formatting were not run at the maintainer's
request.


## Follow-up audit — host-global build visibility (2026-11-27)

The build history, active-build, and buildProgress resolvers require
HostAuthority::Read, and GET /ws/builds checks host authority at upgrade. The
GraphQL module-permission classifier nevertheless classified those fields as
tenant modules:read. That redundant gate could block a legitimate host
operator without tenant modules:read. The classifier is now aligned with the
resolver-level host boundary; regressions assert these host-global fields do
not receive a tenant module policy. The process-local BuildEventHub remains
non-durable/non-distributed, and existing WebSocket connections retain the
host-authority context captured at upgrade. Host-token rotation therefore
requires reconnecting sockets.
