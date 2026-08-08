# Index replay GraphQL transport

Status: `locale_source_complete_execution_pending`.

## Boundary

The server exposes two bounded GraphQL mutations over the guarded `IndexReplayOperatorRuntime`:

- `runIndexReplay(input: ...)` runs one bounded schema-wide or exact-locale replay chunk and samples the server-owned graceful-shutdown signal at replay durable safe points;
- `cancelIndexReplay(input: ...)` requests cancellation of one replay job in the authenticated tenant.

This remains a command transport. It does not add automatic replay scheduling, background worker ownership, a
new replay algorithm, partition checkpoint scope, explicit targeted/full/shadow rebuild modes, or a traffic
cutover.

## Authority before input parsing

Tenant and actor identities are never accepted from GraphQL input. The transport derives both from
`TenantContext` / `AuthContext`, requires the request-bound effective permission snapshot, and requires
`modules:manage` before parsing untrusted schema identifiers, schema version, optional locale, or job UUID.

Only after authorization does the run path canonicalize a supplied locale through `rustok_index::LocaleKey`.
Locale input is bounded to 32 bytes before parsing. Omission is not inferred from schema metadata and preserves
the historical schema-wide replay identity exactly.

The server-owned `IndexReplayOperatorRuntime` performs the same authorization again before delegating to the
canonical shared replay runtime. A cross-tenant job UUID is therefore only looked up in the authenticated
tenant and cannot widen authority.

## Server-owned replay budget

`IndexReplayRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- optional canonicalizable locale.

It does not accept tenant, actor, worker identity, page limit, page count, heartbeat cadence, lease duration,
partition, source name, database handle, scheduler controls, shutdown state, or a stop handle.

Each GraphQL invocation constructs a fresh server-owned worker identity and uses one fixed bounded chunk:

- page limit: `100` mutations;
- maximum pages: `8`;
- heartbeat cadence: every page;
- lease duration: `60` seconds.

Those transport caps are intentionally narrower than the generic replay runner bounds. A yielded job remains
resumable by a later authorized invocation through the existing durable job/checkpoint contract.

## Locale identity

A supplied locale is canonicalized once at the transport boundary and carried by `IndexReplayRunRequest` into
the multi-page runner. The runner derives both durable job lease scope and every page request from that same
`LocaleKey`:

- schema-wide request -> schema job + schema checkpoint (`locale_key = ''`);
- exact-locale request -> locale job + same canonical locale checkpoint.

The terminal success fence checks the checkpoint using the leased locale rather than a hard-coded empty locale.
This keeps acquisition, page scan, checkpoint writes and final success on one exact scope. `partition_key`
remains empty in both cases.

The one-page worker still resolves the registered runtime schema before checkpoint/source work and rejects a
locale run for `LocaleMode::None`. Product is the currently retained production source that filters exact locale
before pagination. Schema-wide replay remains valid, including for `LocaleMode::Required`, so omission does not
silently become one locale.

## Graceful shutdown binding

GraphQL schema initialization resolves one `StopHandle` from `ServerRuntimeContext`. If no worker or module-work
host has created it yet, schema initialization atomically publishes one. All cloned `ServerRuntimeContext`
instances share the same typed value map, so later background-worker/module-work initialization reuses that same
handle instead of creating a separate shutdown domain.

Schema initialization also retains one watch receiver for the lifetime of the server context. This matters for
API-only hosts: `StopHandle::stop()` uses the watch sender and must have a receiver alive for the stop value to be
published. The retained receiver is not exposed through GraphQL.

`runIndexReplay` obtains only the server-owned handle from schema data and passes
`StopHandle::is_stopping` as a synchronous boolean probe through:

`IndexReplayOperatorRuntime::run_interruptible` -> `SharedIndexReplayRuntime::run_interruptible` ->
`PostgresIndexReplayRunner::run_interruptible`.

The interruptible runner uses the same locale-aware lease helper as ordinary replay. A yielded locale job
therefore keeps the same durable locale identity on the next authorized attempt instead of falling back to a
schema job.

The Index runtime and server operator remain lifecycle-type neutral; neither imports `StopHandle`. They accept
only the boolean probe and keep authorization ahead of replay execution.

If shutdown is observed at a replay safe point, the runner-level contract returns the job to durable `pending`
without setting `cancel_requested` or publishing failure. A later process/authorized invocation may resume from
the last committed checkpoint; already-durable deliveries remain duplicate-safe.

The cancellation mutation does not sample or mutate shutdown state. User cancellation and host shutdown remain
separate state machines.

## Result surface

The run payload exposes only bounded operational counters plus status and job UUID. It does not expose source
payloads, SQL, database errors, request authority, lease owner identity, locale internals, shutdown state, or raw
dependency causes.

The cancellation payload exposes only the normalized outcome: requested, cancelled, already terminal, or not
found. Operational runner failures are mapped to a generic GraphQL error; an unknown source-owned schema is
reported as bad user input.

## Evidence state

Source guards retain:

- authorization-before-schema/locale parsing ordering;
- absence of caller authority/worker/resource-budget/shutdown/partition fields;
- optional locale canonicalization and schema-wide omission compatibility;
- exact runner page/job/checkpoint locale coupling;
- locale-aware interruptible runner acquisition and terminal success fencing;
- fixed server-owned bounds;
- delegation only through `IndexReplayOperatorRuntime`;
- merged GraphQL schema registration;
- one shared lifecycle handle and API-host watch keepalive;
- `StopHandle::is_stopping` as the only replay shutdown observation;
- no direct database/source/scheduler access and no `.stop()` call from the transport.

GraphQL execution, actual locale replay/restart execution, actual process-shutdown replay interruption, database
replay execution, cancellation races, CI, and retained runtime evidence remain maintainer-owned and are not
claimed by this source slice.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
