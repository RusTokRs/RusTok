# Index replay GraphQL transport

Status: `full_locale_schema_wide_shadow_and_locale_safe_continuation_source_complete_execution_pending`.

## Boundary

The server exposes three bounded GraphQL mutations over server-owned guarded replay capabilities:

- `runIndexReplay(input: ...)` runs one bounded durable Full schema-wide or exact-locale replay chunk and samples the server-owned graceful-shutdown signal at replay durable safe points;
- `runIndexReplayShadow(input: ...)` runs one bounded **schema-wide**, side-effect-free Shadow validation chunk and returns only an authenticated confidential continuation token when more source pages remain;
- `cancelIndexReplay(input: ...)` requests cancellation of one durable replay job in the authenticated tenant.

This remains command transport. It does not add automatic replay scheduling, background worker ownership, a new
replay algorithm, partition checkpoint scope, Targeted mutation execution, Shadow persistence, or a traffic
cutover.

## Authority before input parsing

Tenant and actor identities are never accepted from GraphQL input. The transport derives both from
`TenantContext` / `AuthContext`, requires the request-bound effective permission snapshot, and requires
`modules:manage` before parsing untrusted schema identifiers, schema version, durable Full locale, Shadow
continuation, or job UUID.

Only after authorization does the durable Full run path canonicalize a supplied locale through
`rustok_index::LocaleKey`. Locale input is bounded to 32 bytes before parsing. Omission is not inferred from schema
metadata and preserves the historical schema-wide durable replay identity exactly.

The current Shadow GraphQL path intentionally still has no locale input. Its continuation input is bounded to
16 KiB only after authorization. The token is then authenticated, decrypted, expired and scope-checked by the
server-owned Shadow transport runtime before any raw `IndexSourceCursor` is reconstructed.

The guarded server runtimes perform the same request-bound authorization again before source execution. A
cross-tenant durable job UUID or Shadow continuation therefore cannot widen authority.

## Server-owned replay budgets

`IndexReplayRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- optional canonicalizable locale.

`IndexReplayShadowRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- optional sealed continuation token.

Neither input accepts tenant, actor, source name, database handle, scheduler controls, partition, retry state, or
resource budget controls. The durable Full input additionally exposes no worker identity, heartbeat cadence, lease
duration, shutdown state, or stop handle. The Shadow input exposes no job, checkpoint, lease, cancellation,
worker, locale, page limit, or max-page field.

Each durable Full GraphQL invocation constructs a fresh server-owned worker identity and uses one fixed bounded
chunk:

- page limit: `100` mutations;
- maximum pages: `8`;
- heartbeat cadence: every page;
- lease duration: `60` seconds.

Each schema-wide Shadow invocation uses the same fixed source page limit and maximum-page count (`100 × 8`) but has
no worker, lease or heartbeat because it creates no durable replay ownership. A yielded Shadow invocation is
resumed only by presenting the sealed caller-carried continuation token to a later authorized invocation.

## Shadow continuation boundary

`IndexReplayShadowTransportRuntime` is a server-owned adapter over the already-guarded
`IndexReplayOperatorRuntime::run_shadow` method. It reuses the deployment continuation keyring and frozen
`SharedIndexSourceRegistry` already used by the source-page diagnosis transport.

Authorization happens before token parsing. The adapter currently derives schema-wide
`IndexSourceContinuationScope::from_registry` from the exact authenticated tenant, requested schema and frozen
source owner. Incoming continuation is opened only after that scope is known; outgoing raw source cursor is sealed
before the result crosses the adapter boundary.

The continuation contract itself is now locale-safe. Its encrypted canonical scope distinguishes schema-wide
`locale = None` from one exact canonical `LocaleKey`, and `IndexSourceContinuationScope::for_locale` constructs
the exact-locale identity from the same frozen source registry. A schema-wide token cannot open under a locale
scope, a locale token cannot open schema-wide, and different canonical locales cannot exchange tokens.

The codec has one current unversioned envelope. No old-format decoder, token version byte, or parallel claim family
is retained. This repository-owned pre-release format is replaced in place when its shape changes.

Exact-locale Shadow GraphQL remains a separate next source boundary because locale still must be carried through
`IndexReplayDryRunRequest`, the actual `IndexSourceScanRequest::for_locale` execution path, the sealed Shadow
adapter and authorization-first input parsing. The now-complete continuation scope no longer blocks that work.

The Shadow result exposes only Complete/Yielded status, bounded page/mutation/upsert/delete counters and the
optional sealed continuation. It does not expose raw cursor JSON, source name, source payloads, source version,
database errors, job/checkpoint identity, lease state, cancellation state, retry state or key material.

## Locale identity for durable Full replay

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

Shadow replay does not import this durable lifecycle/cancellation path. It remains bounded by server-owned page
counts and the source call timeout policy and creates no durable pending state to resume on host shutdown.

The cancellation mutation does not sample or mutate shutdown state. User cancellation and host shutdown remain
separate state machines.

## Result surface

The durable Full run payload exposes only bounded operational counters plus status and job UUID. It does not expose
source payloads, SQL, database errors, request authority, lease owner identity, locale internals, shutdown state,
or raw dependency causes.

The Shadow payload exposes only bounded validation counters plus status and optional sealed continuation. The
cancellation payload exposes only the normalized durable outcome: requested, cancelled, already terminal, or not
found.

Operational runner failures are mapped to generic GraphQL errors; unknown source-owned schemas are reported as bad
user input. Invalid/expired/scope-mismatched Shadow continuations receive stable input error handling while
missing/unresolvable continuation keyring dependencies are reported through fixed server-owned error identities.

## Evidence state

Source guards retain:

- authorization-before-schema/locale/continuation/job parsing ordering;
- absence of caller authority/worker/resource-budget/shutdown/partition fields;
- optional durable locale canonicalization and schema-wide omission compatibility;
- schema-wide Shadow input limited to schema identity plus sealed continuation;
- exact durable runner page/job/checkpoint locale coupling;
- locale-aware interruptible runner acquisition and terminal success fencing;
- fixed server-owned `100 × 8` bounds for both durable Full and schema-wide Shadow invocations;
- Shadow continuation open/seal through the deployment keyring and frozen source scope;
- canonical continuation scope separation between schema-wide and exact-locale scans;
- one current unversioned continuation envelope with no legacy decoder family;
- delegation of Shadow execution only through guarded `IndexReplayOperatorRuntime::run_shadow`;
- merged GraphQL schema registration through the existing `IndexReplayMutation` object;
- one shared lifecycle handle and API-host watch keepalive for durable Full replay;
- no direct database/source/scheduler access from GraphQL and no `.stop()` call from the transport.

GraphQL execution, actual Shadow source execution, continuation key deployment evidence, durable locale
replay/restart execution, actual process-shutdown replay interruption, database replay execution, cancellation
races, CI, and retained runtime evidence remain maintainer-owned and are not claimed by this source slice.

Exact-locale Shadow transport remains source-open only for the dry-run/runtime/GraphQL locale execution path; the
continuation identity prerequisite is source-complete.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
