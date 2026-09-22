# Index replay GraphQL transport

Status: `full_shadow_targeted_source_complete_execution_pending`.

## Boundary

The server exposes four bounded GraphQL mutations over server-owned guarded replay capabilities:

- `runIndexReplay(input: ...)` runs one bounded durable Full schema-wide or exact-locale replay chunk and samples the server-owned graceful-shutdown signal at durable safe points;
- `runIndexReplayTargeted(input: ...)` runs one bounded exact-key Targeted mutation-application invocation;
- `runIndexReplayShadow(input: ...)` runs one bounded schema-wide or exact-locale side-effect-free Shadow validation chunk and returns only an authenticated confidential continuation token when more source pages remain;
- `cancelIndexReplay(input: ...)` requests cancellation of one durable Full replay job in the authenticated tenant.

These are dedicated command surfaces, not one caller-controlled replay mode selector. The transport
does not add automatic replay scheduling, background worker ownership, a new replay algorithm,
partition checkpoint scope, Targeted durable state, Shadow persistence, or a traffic cutover.

## Authority before input parsing

Tenant and actor identities are never accepted from GraphQL input. The transport derives both from
`TenantContext` / `AuthContext`, requires the request-bound effective permission snapshot, and requires
`modules:manage` before custom parsing untrusted replay fields.

For Full this means authorization precedes schema and optional locale parsing. For Shadow it precedes
schema, optional locale and continuation parsing. For Targeted it precedes schema, target UUID and
per-target locale parsing. Cancellation authorizes before parsing the job UUID.

Full/Shadow locale and every Targeted key locale are bounded to 32 bytes and canonicalized through
`rustok_index::LocaleKey`. Targeted therefore treats canonical aliases as the same exact key: the same
entity requested with `EN-us` and `en-US` becomes a duplicate canonical `EntityKey` and fails closed.
Omission is not inferred from schema metadata and preserves the historical schema-wide replay identity exactly.

Shadow continuation input is bounded to 16 KiB only after authorization. The token is then
authenticated, decrypted, expired and scope-checked by the server-owned Shadow transport runtime
before any raw `IndexSourceCursor` is reconstructed.

The guarded server runtimes repeat request-bound authorization before source execution. Cross-tenant
requests therefore cannot widen authority even if a transport preparation path is bypassed.

## Caller-owned request shapes

`IndexReplayRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- optional canonicalizable locale.

`IndexReplayTargetedRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- `targets`, bounded to 1..=256 entries.

Each Targeted entry contains only:

- entity UUID;
- optional canonicalizable locale.

`IndexReplayShadowRunInput` contains only:

- module name;
- entity name;
- positive schema routing key;
- optional canonicalizable locale;
- optional sealed continuation token.

No replay run input accepts tenant, actor, source name, database handle, scheduler controls, partition,
retry state, or a generic mode. Full additionally exposes no worker identity, heartbeat cadence, lease
duration, shutdown state or stop handle. Shadow exposes no job/checkpoint/lease/cancel/worker/page-limit
controls. Targeted exposes no worker/page budget, continuation, job/checkpoint, lease, cancellation,
retry/requeue or graceful-stop control.

## Server-owned replay bounds

Each durable Full GraphQL invocation constructs a fresh server-owned worker identity and uses one fixed
bounded chunk:

- page limit: `100` mutations;
- maximum pages: `8`;
- heartbeat cadence: every page;
- lease duration: `60` seconds.

Each Shadow invocation uses the same fixed source page limit and maximum-page count (`100 × 8`) but has
no worker, lease or heartbeat because it creates no durable replay ownership. A yielded Shadow
invocation resumes only by presenting the sealed continuation to a later authorized invocation with
the same scan scope.

Each Targeted invocation uses the canonical exact-key load bound: 1..=256 unique keys in one tenant and
one exact schema. The transport rejects empty or over-256 lists before per-key UUID/locale parsing and
then constructs `IndexSourceLoadRequest`; canonical request validation remains authoritative for
mixed-scope and duplicate exact keys.

## Targeted exact-key boundary

`runIndexReplayTargeted` delegates only to `IndexReplayOperatorRuntime::run_targeted`. GraphQL never
calls `SharedIndexReplayRuntime`, `IndexReplayTargetedExecutor`, `PostgresMutationStore` or an
`IndexSource` directly.

After authorization, preparation parses one schema and constructs every `EntityKey` from:

- server-owned tenant;
- that one parsed schema;
- caller-supplied entity UUID;
- optional canonical `LocaleKey`.

`IndexReplayOperatorRuntime::run_targeted` repeats exact-tenant/request-bound `modules:manage`
authorization before `SharedIndexReplayRuntime::run_targeted`. The application executor then owns
active-schema entity/locale admission, exact source resolution, one bounded source load, whole-batch
preflight and stable-event mutation persistence.

The Targeted payload exposes only requested, returned mutation, missing, applied, duplicate and
stale-ignored counts. Although `IndexReplayTargetedOutcome` carries the resolved source name
internally, the GraphQL payload does not expose it. Missing keys are counts only; the transport does
not synthesize deletes.

Unknown/unregistered schemas and active-schema invalid targets become bad user input. Source contract,
returned-batch and mutation persistence failures map to a generic Targeted command error so owner and
storage internals do not cross the API boundary.

Targeted has no durable pending state and does not receive `StopHandle`. Exact retry is a new bounded
request over the same keys and converges through source-owned stable event UUIDs plus existing inbox
`Duplicate` / `StaleIgnored` semantics.

## Shadow continuation boundary

`IndexReplayShadowTransportRuntime` is a server-owned adapter over the already-guarded
`IndexReplayOperatorRuntime::run_shadow` method. It reuses the deployment continuation keyring and
frozen `SharedIndexSourceRegistry` also used by source-page diagnosis.

Authorization happens before token parsing. The adapter derives canonical continuation scope from the
exact authenticated tenant, requested schema, frozen source owner and optional canonical locale:

- schema-wide -> `IndexSourceContinuationScope::from_registry`;
- exact-locale -> `IndexSourceContinuationScope::for_locale`.

Incoming continuation is opened only after exact scope is known. The adapter then creates matching
schema-wide `IndexReplayDryRunRequest::new` or exact-locale `IndexReplayDryRunRequest::for_locale`.
Outgoing raw source cursor is sealed under the same scope before crossing the adapter boundary.

`SharedIndexReplayDryRunRuntime` rejects exact-locale execution for `LocaleMode::None` before source
scanning and constructs every actual page through `IndexSourceScanRequest::new` or
`IndexSourceScanRequest::for_locale`.

The continuation codec has one current unversioned envelope. No old-format decoder, token version byte
or parallel claim family is retained. Schema-wide and exact-locale tokens cannot cross scopes, and
different canonical locales cannot exchange tokens.

The Shadow result exposes only Complete/Yielded status, bounded page/mutation/upsert/delete counters
and optional sealed continuation. It exposes no raw cursor JSON, source name, source payloads, source
version, database errors, job/checkpoint identity, lease state, cancellation state, retry state or key
material.

## Locale identity for durable Full replay

A supplied Full locale is canonicalized once at the transport boundary and carried by
`IndexReplayRunRequest` into the multi-page runner. The runner derives durable job lease scope and
every page request from the same `LocaleKey`:

- schema-wide request -> schema job + schema checkpoint (`locale_key = ''`);
- exact-locale request -> locale job + same canonical locale checkpoint.

The terminal success fence checks the checkpoint using the leased locale rather than a hard-coded empty locale.
This keeps acquisition, page scan, checkpoint writes and final success on one exact scope.
`partition_key` remains empty in both cases.

The one-page worker resolves the registered runtime schema before checkpoint/source work and rejects a
locale run for `LocaleMode::None`. Product is the currently retained production source that filters
exact locale before pagination. Schema-wide replay remains valid, including for
`LocaleMode::Required`, so omission does not silently become one locale.

## Graceful shutdown binding

GraphQL schema initialization resolves one `StopHandle` from `ServerRuntimeContext`. If no worker or
module-work host has created it yet, schema initialization atomically publishes one. All cloned
`ServerRuntimeContext` instances share the same typed value map, so later background-worker/module-work
initialization reuses the same handle.

Schema initialization retains one watch receiver for the server-context lifetime. `runIndexReplay`
reads only `StopHandle::is_stopping` and passes it through
`IndexReplayOperatorRuntime::run_interruptible` to the durable runner. It never calls
`StopHandle::stop`, and no shutdown field is accepted from GraphQL input.

Targeted and Shadow do not import this durable lifecycle/cancellation path. Targeted has no resumable
durable pending state; Shadow resumes only from a sealed continuation returned by a completed bounded
call.

Cancellation does not sample or mutate shutdown state. User cancellation and host shutdown remain
separate state machines.

## Result surface

The durable Full payload exposes only bounded operational counters plus status and job UUID. The
Targeted payload exposes only exact-key counters. The Shadow payload exposes bounded validation
counters plus status and optional sealed continuation. Cancellation exposes only the normalized
durable outcome: requested, cancelled, already terminal, or not found.

None of these surfaces expose source payloads, SQL, database errors, request authority, lease owner
identity, source owner routing or raw dependency causes.

## Evidence state

Source guards retain:

- authorization-before-Full schema/locale, Targeted schema/entity/locale, Shadow schema/locale/continuation and cancel job parsing;
- absence of caller authority/source/mode/lifecycle/partition fields;
- canonical 1..=256 Targeted exact-key request construction and per-key locale canonicalization;
- Targeted delegation only through `IndexReplayOperatorRuntime::run_targeted`;
- absence of `source_name` from Targeted GraphQL payload;
- optional Full and Shadow locale canonicalization plus schema-wide omission compatibility;
- exact durable runner page/job/checkpoint locale coupling;
- exact Shadow request/source/token locale coupling without durable ownership;
- fixed server-owned `100 × 8` bounds for Full/Shadow and canonical 256-key bound for Targeted;
- Shadow continuation open/seal through deployment keyring and frozen source scope;
- one current unversioned continuation envelope with no legacy decoder family;
- merged GraphQL schema registration through the existing `IndexReplayMutation` object;
- one shared lifecycle handle and API-host watch keepalive for durable Full only;
- no direct database/source/scheduler access from GraphQL and no `.stop()` call from transport.

GraphQL execution, actual Targeted/Shadow source execution, PostgreSQL Targeted behavior, continuation
key deployment evidence, durable locale replay/restart execution, process-shutdown interruption,
database replay execution, cancellation races, CI and retained runtime evidence remain
maintainer-owned and are not claimed by this source slice.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows,
CI, or `git diff --check` were executed by the implementation agent.
