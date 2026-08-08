# M6 replay mode contract

Status: `source_complete_shadow_locale_transport_execution_pending`.

This slice defines explicit rebuild mode identity without changing the existing durable replay runner, job,
checkpoint, cancellation, locale or lease state machines. Shadow execution is bound to the server-owned request
authorization guard and has a dedicated schema-wide or exact-locale GraphQL command through one sealed
continuation boundary.

## Mode identity

`IndexReplayMode` has exactly three modes:

- `Full` — cursor-based durable source scan. Its execution surface is `DurableScan` and the existing
  `PostgresIndexReplayRunner` remains the only admitted implementation.
- `Targeted` — bounded exact-key source load. Its execution surface is `TargetedLoad`; construction delegates to
  the canonical `IndexSourceLoadRequest`, so the existing 1..=256 key bound, exact tenant/schema scope and key
  uniqueness remain authoritative.
- `Shadow` — side-effect-free cursor scan. Its execution surface is `SideEffectFreeScan`, matching the existing
  `SharedIndexReplayDryRunRuntime` no-write boundary.

Mode is not locale scope and is not future partition scope. Adding a locale to a Full or Shadow scan does not
create a new mode, and adding partition replay later must not encode partition identity as a mode.

## Fail-closed routing

`IndexReplayModeSelection::is_admitted_to_durable_scan_runner` returns true only for `Full`.

Targeted and Shadow modes must not alias the Full durable job/checkpoint identity:

- Targeted execution must be a bounded `IndexSource::load` path and must define its own operator execution
  contract before it can persist mutations;
- Shadow execution remains no-write and uses the dry-run surface rather than the durable mutation/job/checkpoint
  path;
- neither mode introduces automatic retry/requeue, another lease owner, another terminal job state, or a second
  cancellation model.

The current `IndexReplayRunRequest`, `PostgresIndexReplayRunner` and GraphQL `runIndexReplay` command remain Full
scan behavior. They do not accept a generic mode selector and are not reinterpreted by this contract.

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

`IndexReplayDryRunRequest` now carries that same optional canonical locale. `SharedIndexReplayDryRunRuntime` rejects
exact-locale execution for `LocaleMode::None` and constructs every actual scan through schema-wide
`IndexSourceScanRequest::new` or exact-locale `IndexSourceScanRequest::for_locale`. Source-page validation therefore
keeps every returned mutation on the same exact scope.

The continuation codec has one current unversioned envelope. There is no version byte, `contract_version`,
old-format claims type, or fallback decoder. Key rotation remains a cryptographic-key concern only and does not
create format compatibility.

## Existing contracts reused

The mode contract composes already-retained boundaries rather than duplicating them:

- Full: durable fenced replay job/checkpoint runner, optional canonical locale and page lease-heartbeat policy;
- Targeted: `IndexSourceLoadRequest` exact-key validation and `IndexSource::load` source boundary;
- Shadow: locale-aware `IndexReplayDryRunRequest` / `SharedIndexReplayDryRunRuntime` bounded side-effect-free scan
  validation, guarded by the server replay operator and transported only through sealed caller-carried
  continuation.

Partition replay remains blocked until a real partition-capable source can filter before pagination.

## Non-goals

This slice does not add:

- Targeted mutation execution;
- Shadow persistence or shadow tables;
- a generic caller-controlled mode selector;
- token-format version families or legacy continuation decoders;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- automatic retry/requeue or scheduler ownership;
- a second durable ownership/fencing model.

## Next source boundary

The explicit Full/Targeted/Shadow identity, guarded Shadow host dispatch and schema-wide/exact-locale Shadow
transport are source-complete. The next independent source-only M6 boundary is the bounded mutation-application
contract required before Targeted can execute over `IndexSource::load`. It must reuse the canonical targeted load
request and must not alias durable scan jobs/checkpoints or invent a second ownership/retry state machine.

Execution/admission remains maintainer-owned. Rust tests, Node verifiers, database scenarios and CI for this source
slice were not executed by the implementation agent.
