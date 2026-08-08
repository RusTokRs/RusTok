# M6 replay mode contract

Status: `source_complete_targeted_host_guard_transport_pending`.

This contract keeps `Full`, `Targeted` and `Shadow` on separate execution surfaces without changing the existing
durable replay runner, job, checkpoint, cancellation, locale or lease state machines. Shadow has guarded
schema-wide/exact-locale transport. Targeted has a bounded application mutation executor, canonical PostgreSQL
runtime composition and request-bound server host dispatch, but no public transport yet.

## Mode identity

`IndexReplayMode` has exactly three modes:

- `Full` — cursor-based durable source scan. Its execution surface is `DurableScan` and the existing
  `PostgresIndexReplayRunner` remains the only admitted implementation.
- `Targeted` — bounded exact-key source load. Its execution surface is `TargetedLoad`; construction delegates to
  the canonical `IndexSourceLoadRequest`, so the existing 1..=256 key bound, exact tenant/schema scope and key
  uniqueness remain authoritative.
- `Shadow` — side-effect-free cursor scan. Its execution surface is `SideEffectFreeScan`, matching the existing
  `SharedIndexReplayDryRunRuntime` no-write boundary.

Mode is not locale scope and is not future partition scope. Targeted locale identity remains part of each exact
`EntityKey`; adding partition replay later must not encode partition identity as a mode.

## Fail-closed routing

`IndexReplayModeSelection::is_admitted_to_durable_scan_runner` returns true only for `Full`.

Targeted and Shadow modes do not alias the Full durable job/checkpoint identity:

- Targeted execution uses `IndexReplayTargetedExecutor` over one canonical `IndexSourceLoadRequest` and the
  existing replay mutation sink;
- Shadow execution remains no-write and uses the dry-run surface rather than the durable mutation/job/checkpoint
  path;
- neither mode introduces automatic retry/requeue, another lease owner, another terminal job state, or a second
  cancellation model.

The current `IndexReplayRunRequest`, `PostgresIndexReplayRunner` and GraphQL `runIndexReplay` command remain Full
scan behavior. They do not accept a generic mode selector and are not reinterpreted by this contract.

## Targeted mutation application

`IndexReplayTargetedExecutor` accepts only `IndexReplayModeSelection::Targeted`. `Full` and `Shadow` fail before
source or mutation execution.

The Targeted selection owns the canonical `IndexSourceLoadRequest`, preserving one non-nil tenant, one exact
schema and 1..=256 unique requested keys. Because the generic load request does not own active-schema semantics,
the executor validates requested keys against the active schema before source resolution or load:

- every requested entity UUID is non-nil;
- `LocaleMode::Required` requires a locale on every requested key;
- `LocaleMode::None` forbids a locale on every requested key;
- `LocaleMode::Optional` accepts either key shape.

This prevents an invalid exact target from being silently reclassified as a missing source key.

After requested-key admission the executor resolves the frozen source owner, performs exactly one bounded
`SharedIndexSourceRegistry::load`, then preflights the whole returned batch before the first write.

The returned-batch preflight requires:

- every source mutation event UUID to be non-nil;
- invocation-local event UUID uniqueness;
- complete `SchemaRegistry::validate_mutation` validity.

The source registry independently guarantees that every returned mutation corresponds to one requested key and
that a key appears at most once. Only after all checks succeed are mutations applied sequentially through the
existing `IndexReplayMutationSink`.

Missing requested keys are allowed by the canonical load contract after requested-key admission. Targeted reports
their count and does not manufacture delete mutations. Owners that model authoritative deletion must return their
own typed delete mutation.

Targeted preserves each source-owned event UUID. With the existing PostgreSQL replay mutation sink this remains
the stable `index_inbox` delivery identity, so exact retry after a partial mutation failure can converge through
ordinary `Duplicate` / `StaleIgnored` behavior without a Targeted checkpoint. Retained source evidence covers the
mutation-1-applied / mutation-2-failed-once window and exact retry convergence.

## Targeted PostgreSQL composition and host dispatch

`materialize_postgres_index_replay_runtime` constructs one
`IndexReplayTargetedExecutor<PostgresMutationStore>` from the same frozen source registry, immutable schema
registry and host database used by replay composition. `SharedIndexReplayRuntime` stores that executor beside the
durable Full runner and exposes a dedicated `run_targeted(IndexSourceLoadRequest)` method. It does not expose a
generic mode switch.

The server's existing `IndexReplayOperatorRuntime` adds a dedicated `run_targeted` method. It requires the exact
request tenant to equal the request-bound operator tenant and requires the same effective `modules:manage`
permission snapshot used by Full before delegating to `SharedIndexReplayRuntime::run_targeted`.

This composition does not create a Targeted job, checkpoint, lease, worker, heartbeat, cancellation state,
graceful-stop path, scheduler registration or retry/requeue owner. The database handle remains encapsulated by the
shared runtime and mutation store.

## Shadow host dispatch

`Shadow` host dispatch is source-complete through `IndexReplayOperatorRuntime::run_shadow`.

The server replay materializer retrieves the `SharedIndexReplayDryRunRuntime` already published by Index replay
composition and stores it beside the durable runtime inside the guarded operator. `run_shadow` authorizes the
exact request tenant through the same request-bound `modules:manage` snapshot used by Full replay, then delegates
to the no-write dry-run runtime.

This remains a host dispatch boundary, not a new durable mode state machine:

- no job or checkpoint identity is created for Shadow;
- no lease, heartbeat, cancel or terminal job transition is added;
- no mutation sink or database connection is exposed;
- Full continues to route to the durable runner unchanged.

## Shadow GraphQL transport

`runIndexReplayShadow` is a dedicated transport rather than a generic mode selector on `runIndexReplay`.
It accepts schema routing identity, one optional canonicalizable locale and one optional authenticated confidential
continuation token.

The GraphQL layer authorizes request-bound `modules:manage` before parsing schema/locale/continuation input. Locale
uses the same bounded `LocaleKey` canonicalization as durable Full replay. A separate server-owned
`IndexReplayShadowTransportRuntime` repeats exact-tenant authorization, derives schema-wide or exact-locale frozen
continuation scope, opens the token, constructs the matching `IndexReplayDryRunRequest`, calls guarded
`run_shadow`, and seals any outgoing cursor under that same scope.

Resource bounds remain server-owned: `100` mutations per source page and at most `8` pages per invocation. Shadow
has no caller-visible worker, lease, heartbeat, job, checkpoint, cancel, retry/requeue or source-name field.
Its payload contains only Complete/Yielded status, bounded scan counters and optional sealed continuation.

## Locale-safe continuation and dry-run execution

`IndexSourceContinuationScope` distinguishes scan scope in encrypted claims:

- schema-wide -> `locale = None`;
- exact locale -> `locale = Some(LocaleKey)`.

`IndexReplayDryRunRequest` carries that same optional canonical locale. `SharedIndexReplayDryRunRuntime` rejects
exact-locale execution for `LocaleMode::None` and constructs every actual scan through schema-wide
`IndexSourceScanRequest::new` or exact-locale `IndexSourceScanRequest::for_locale`. Source-page validation therefore
keeps every returned mutation on the same exact scope.

The continuation codec has one current unversioned envelope. There is no version byte, `contract_version`,
old-format claims type, or fallback decoder. Key rotation remains a cryptographic-key concern only and does not
create format compatibility.

## Existing contracts reused

The mode contract composes already-retained boundaries rather than duplicating them:

- Full: durable fenced replay job/checkpoint runner, optional canonical locale and page lease-heartbeat policy;
- Targeted: `IndexSourceLoadRequest`, active-schema requested-key admission, `SharedIndexSourceRegistry::load`,
  full-batch replay preflight, `IndexReplayMutationSink` stable delivery application, PostgreSQL mutation-store
  composition and request-bound `modules:manage` host dispatch;
- Shadow: locale-aware `IndexReplayDryRunRequest` / `SharedIndexReplayDryRunRuntime` bounded side-effect-free scan
  validation, guarded by the server replay operator and transported only through sealed caller-carried
  continuation.

Partition replay remains blocked until a real partition-capable source can filter before pagination.

## Non-goals

This slice does not add:

- Targeted GraphQL/HTTP/CLI/admin transport;
- Targeted jobs/checkpoints/leases/cancellation, graceful-stop semantics or automatic retry/requeue;
- Shadow persistence or shadow tables;
- a generic caller-controlled mode selector;
- token-format version families or legacy continuation decoders;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- a second durable ownership/fencing model.

## Next source boundary

The explicit mode identity, bounded Targeted application executor, PostgreSQL Targeted composition, guarded
Targeted/Shadow host dispatch and schema-wide/exact-locale Shadow transport are source-complete.

The next independent Targeted boundary is a dedicated authorization-first public transport. It must authorize the
request-bound tenant/actor before parsing untrusted exact targets, build only the canonical bounded
`IndexSourceLoadRequest`, delegate solely to `IndexReplayOperatorRuntime::run_targeted`, expose bounded counters,
and keep source names, workers, jobs/checkpoints, leases, cancellation, retry/requeue, partition scope and generic
mode selection unavailable to callers.

Execution/admission remains maintainer-owned. Rust tests, Node verifiers, database scenarios and CI for this source
slice were not executed by the implementation agent.
