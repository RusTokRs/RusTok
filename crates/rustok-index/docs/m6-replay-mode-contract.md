# M6 replay mode contract

Status: `source_complete_shadow_schema_wide_transport_locale_pending`.

This slice defines explicit rebuild mode identity without changing the existing durable replay runner, job,
checkpoint, cancellation, locale or lease state machines. Shadow execution is bound to the server-owned request
authorization guard and now has a dedicated schema-wide GraphQL command through a sealed continuation boundary.

## Mode identity

`IndexReplayMode` has exactly three modes:

- `Full` — cursor-based durable source scan. Its execution surface is `DurableScan` and the existing
  `PostgresIndexReplayRunner` remains the only admitted implementation.
- `Targeted` — bounded exact-key source load. Its execution surface is `TargetedLoad`; construction delegates to
  the canonical `IndexSourceLoadRequest`, so the existing 1..=256 key bound, exact tenant/schema scope and key
  uniqueness remain authoritative.
- `Shadow` — side-effect-free cursor scan. Its execution surface is `SideEffectFreeScan`, matching the existing
  `SharedIndexReplayDryRunRuntime` no-write boundary.

Mode is not locale scope and is not future partition scope. Adding a locale to a Full scan does not create a new
mode, and adding partition replay later must not encode partition identity as a mode.

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

## Schema-wide Shadow GraphQL transport

`runIndexReplayShadow` is now a dedicated transport rather than a generic mode selector on `runIndexReplay`.
It accepts only schema routing identity and an optional authenticated confidential continuation token.

The GraphQL layer authorizes request-bound `modules:manage` before parsing schema/continuation input. A separate
server-owned `IndexReplayShadowTransportRuntime` repeats exact-tenant authorization, opens continuation only under
the frozen tenant/schema/source scope, constructs the existing `IndexReplayDryRunRequest`, calls guarded
`run_shadow`, and seals any outgoing cursor before returning.

Resource bounds remain server-owned: `100` mutations per source page and at most `8` pages per invocation. Shadow
has no caller-visible worker, lease, heartbeat, job, checkpoint, cancel, retry/requeue or source-name field.
Its payload contains only Complete/Yielded status, bounded scan counters and optional sealed continuation.

The transport is intentionally schema-wide. `IndexSourceContinuationScope` currently binds tenant/schema/source
but not locale. Until continuation claims make schema-wide and exact-locale scopes distinct, exposing a locale
field would allow a valid source cursor token to cross scan scopes. Exact-locale Shadow transport therefore
remains fail-closed and source-open.

## Existing contracts reused

The mode contract composes already-retained boundaries rather than duplicating them:

- Full: durable fenced replay job/checkpoint runner, optional canonical locale and page lease-heartbeat policy;
- Targeted: `IndexSourceLoadRequest` exact-key validation and `IndexSource::load` source boundary;
- Shadow: `IndexReplayDryRunRequest` / `SharedIndexReplayDryRunRuntime` bounded side-effect-free scan validation,
  guarded by the server replay operator and transported only through sealed caller-carried continuation.

Partition replay remains blocked until a real partition-capable source can filter before pagination.

## Non-goals

This slice does not add:

- Targeted mutation execution;
- Shadow persistence or shadow tables;
- a generic caller-controlled mode selector;
- exact-locale Shadow transport before locale-safe continuation scope;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- automatic retry/requeue or scheduler ownership;
- a second durable ownership/fencing model.

## Next source boundary

The next independent Shadow boundary is locale-safe continuation identity before exact-locale GraphQL transport.
The continuation contract must distinguish schema-wide from canonical exact-locale source scans without weakening
existing tenant/schema/source binding or invalidating retained schema-wide transport semantics. Only after that
scope exists should Shadow reuse the canonical `LocaleKey` / `IndexSourceScanRequest::for_locale` contract.

Targeted execution remains a separate later slice because it needs an explicit bounded mutation-application
contract over `IndexSource::load` rather than a scan checkpoint.

Execution/admission remains maintainer-owned. Rust tests, Node verifiers, database scenarios and CI for this source
slice were not executed by the implementation agent.
