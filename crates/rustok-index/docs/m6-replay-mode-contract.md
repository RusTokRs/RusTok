# M6 replay mode contract

Status: `source_complete_targeted_graphql_execution_pending`.

This contract keeps `Full`, `Targeted` and `Shadow` on separate execution surfaces without changing
the durable replay runner, job, checkpoint, cancellation, locale or lease state machines. Full keeps
its durable GraphQL command, Shadow keeps its sealed schema-wide/exact-locale GraphQL transport, and
Targeted now has bounded application execution, PostgreSQL composition, request-bound host dispatch
and one dedicated exact-key GraphQL command.

## Mode identity

`IndexReplayMode` has exactly three modes:

- `Full` — cursor-based durable source scan. Its execution surface is `DurableScan` and the existing
  `PostgresIndexReplayRunner` remains the only admitted implementation.
- `Targeted` — bounded exact-key source load. Its execution surface is `TargetedLoad`; construction
  delegates to the canonical `IndexSourceLoadRequest`, so the 1..=256 key bound, exact tenant/schema
  scope and key uniqueness remain authoritative.
- `Shadow` — side-effect-free cursor scan. Its execution surface is `SideEffectFreeScan`, matching
  `SharedIndexReplayDryRunRuntime`.

Mode is not locale scope and is not future partition scope. Targeted locale identity remains part of
each exact `EntityKey`; adding partition replay later must not encode partition identity as a mode.

## Fail-closed routing

`IndexReplayModeSelection::is_admitted_to_durable_scan_runner` returns true only for `Full`.

Targeted and Shadow do not alias the Full durable job/checkpoint identity:

- Targeted execution uses `IndexReplayTargetedExecutor` over one canonical `IndexSourceLoadRequest`
  and the existing replay mutation sink;
- Shadow execution remains no-write and uses the dry-run surface;
- neither mode introduces automatic retry/requeue, another lease owner, another terminal job state,
  or a second cancellation model.

`IndexReplayRunRequest`, `PostgresIndexReplayRunner` and GraphQL `runIndexReplay` remain Full scan
behavior. They do not accept a generic mode selector and are not reinterpreted by this contract.

## Targeted mutation application

`IndexReplayTargetedExecutor` accepts only `IndexReplayModeSelection::Targeted`. `Full` and `Shadow`
fail before source or mutation execution.

The Targeted selection owns the canonical `IndexSourceLoadRequest`, preserving one non-nil tenant,
one exact schema and 1..=256 unique requested keys. Because the generic load request does not own
active-schema semantics, the executor validates requested keys against the active schema before
source resolution or load:

- every requested entity UUID is non-nil;
- `LocaleMode::Required` requires a locale on every requested key;
- `LocaleMode::None` forbids a locale on every requested key;
- `LocaleMode::Optional` accepts either key shape.

After requested-key admission the executor resolves the frozen source owner, performs exactly one
bounded `SharedIndexSourceRegistry::load`, then preflights the whole returned batch before the first
write. It requires non-nil and invocation-unique source event UUIDs plus complete
`SchemaRegistry::validate_mutation` validity.

Missing requested keys are allowed after requested-key admission. Targeted reports their count and
does not manufacture delete mutations. Owners that model authoritative deletion must return their
own typed delete mutation.

Targeted preserves each source-owned event UUID. With the existing PostgreSQL replay mutation sink
this remains the stable `index_inbox` delivery identity, so exact retry after a partial mutation
failure converges through ordinary `Duplicate` / `StaleIgnored` behavior without a Targeted
checkpoint.

## Targeted PostgreSQL composition and host dispatch

`materialize_postgres_index_replay_runtime` constructs one
`IndexReplayTargetedExecutor<PostgresMutationStore>` from the same frozen source registry, immutable
schema registry and host database used by replay composition. `SharedIndexReplayRuntime` stores that
executor beside the durable Full runner and exposes a dedicated
`run_targeted(IndexSourceLoadRequest)` method. It does not expose a generic mode switch.

The server's existing `IndexReplayOperatorRuntime` adds a dedicated `run_targeted` method. It requires
the exact request tenant to equal the request-bound operator tenant and requires the same effective
`modules:manage` permission snapshot used by Full before delegating to
`SharedIndexReplayRuntime::run_targeted`.

Targeted uses a separate `IndexReplayTargetedOperatorError` wrapper around the unchanged Full/cancel
`IndexReplayOperatorError`, keeping error surfaces and authorization models separate.

This composition creates no Targeted job, checkpoint, lease, worker, heartbeat, cancellation state,
graceful-stop path, scheduler registration or retry/requeue owner.

## Targeted GraphQL transport

`runIndexReplayTargeted` is a dedicated mutation, not a generic `mode` field on `runIndexReplay`.
It accepts one schema routing identity and a bounded `targets` list. Each target contains one entity
UUID and one optional canonicalizable locale.

Tenant/actor come only from request context. The GraphQL preparation path requires request-bound
effective `modules:manage` before parsing schema, target UUIDs or target locales. It rejects empty or
over-256 target sets before per-key parsing, canonicalizes each locale through `LocaleKey`, constructs
only `IndexSourceLoadRequest`, then delegates only to `IndexReplayOperatorRuntime::run_targeted`.
The operator repeats exact-tenant/request-snapshot authorization before source or persistence work.

Canonical locale aliases therefore share exact-key identity: for example, duplicate requests for the
same entity using `EN-us` and `en-US` collapse to the same `EntityKey` and fail the canonical duplicate
key check.

The payload exposes only requested, returned mutation, missing, applied, duplicate and stale-ignored
counts. The application outcome's resolved source name is deliberately not exposed through GraphQL.
The input contains no source name, generic mode, worker, page budget, job/checkpoint, lease,
cancellation, retry/requeue, scheduler or partition controls.

Unknown/unregistered schemas and active-schema invalid targets are bad user input. Source contract,
returned-batch and mutation persistence failures remain generic internal Targeted command errors so
owner/storage details do not cross the transport.

## Shadow host dispatch and GraphQL transport

`Shadow` host dispatch remains `IndexReplayOperatorRuntime::run_shadow`, authorized through the same
request-bound `modules:manage` snapshot. `runIndexReplayShadow` remains a dedicated transport rather
than a generic mode selector. It accepts schema identity, optional canonical locale and optional
authenticated confidential continuation.

`IndexReplayShadowTransportRuntime` repeats exact-tenant authorization, derives schema-wide or
exact-locale continuation scope, opens the token, constructs the matching
`IndexReplayDryRunRequest`, calls guarded `run_shadow`, and seals any outgoing cursor under that same
scope. Resource bounds remain server-owned at `100 × 8`; Shadow has no caller-visible worker, lease,
heartbeat, job, checkpoint, cancel, retry/requeue or source-name field.

## Locale-safe continuation and dry-run execution

`IndexSourceContinuationScope` distinguishes scan scope in encrypted claims:

- schema-wide -> `locale = None`;
- exact locale -> `locale = Some(LocaleKey)`.

`IndexReplayDryRunRequest` carries that same optional canonical locale. The dry-run runtime rejects
exact-locale execution for `LocaleMode::None` and constructs every scan through schema-wide
`IndexSourceScanRequest::new` or exact-locale `IndexSourceScanRequest::for_locale`.

The continuation codec has one current unversioned envelope. There is no version byte,
`contract_version`, old-format claims type or fallback decoder. Key rotation is a cryptographic-key
concern only and does not create format compatibility.

## Existing contracts reused

The mode contract composes existing boundaries rather than duplicating them:

- Full: durable fenced replay job/checkpoint runner, optional canonical locale and page
  lease-heartbeat policy;
- Targeted: `IndexSourceLoadRequest`, active-schema requested-key admission,
  `SharedIndexSourceRegistry::load`, whole-batch replay preflight, stable mutation delivery,
  `PostgresMutationStore`, request-bound host authorization and dedicated exact-key GraphQL transport;
- Shadow: locale-aware dry-run validation, guarded host dispatch and sealed caller-carried
  continuation transport.

Partition replay remains blocked until a real partition-capable source can filter before pagination.

## Non-goals

This contract does not add:

- Targeted jobs/checkpoints/leases/cancellation, graceful-stop semantics or automatic retry/requeue;
- Targeted HTTP/CLI/admin transport;
- Shadow persistence or shadow tables;
- a generic caller-controlled mode selector;
- token-format version families or legacy continuation decoders;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- a second durable ownership/fencing model.

## Next gate

The explicit mode identity, Targeted application executor, PostgreSQL composition, guarded
Targeted/Shadow host dispatch, dedicated Targeted GraphQL command and schema-wide/exact-locale Shadow
transport are source-complete.

No additional independent source-only M6 replay boundary is open. Targeted, Full and Shadow transport
execution/admission remains maintainer-owned. Partition replay stays blocked until a real source
contract can filter partition scope before pagination. Any further source change should be driven by
executed evidence or a newly available partition-capable owner contract, not by adding parallel mode,
version or compatibility surfaces.

Rust tests, Node verifiers, database scenarios and CI were not executed by the implementation agent.
