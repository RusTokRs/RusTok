# M6 replay mode contract

Status: `source_complete_shadow_schema_wide_transport_locale_execution_pending`.

This slice defines explicit rebuild mode identity without changing the existing durable replay runner, job,
checkpoint, cancellation, locale or lease state machines. Shadow execution is bound to the server-owned request
authorization guard and has a dedicated schema-wide GraphQL command through a sealed continuation boundary.
The shared continuation identity is now locale-safe; exact-locale Shadow execution remains the next separate
transport/runtime slice.

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

## Schema-wide Shadow GraphQL transport

`runIndexReplayShadow` is a dedicated transport rather than a generic mode selector on `runIndexReplay`.
It currently accepts only schema routing identity and an optional authenticated confidential continuation token.

The GraphQL layer authorizes request-bound `modules:manage` before parsing schema/continuation input. A separate
server-owned `IndexReplayShadowTransportRuntime` repeats exact-tenant authorization, opens continuation only under
the frozen schema-wide tenant/schema/source scope, constructs the existing `IndexReplayDryRunRequest`, calls
guarded `run_shadow`, and seals any outgoing cursor before returning.

Resource bounds remain server-owned: `100` mutations per source page and at most `8` pages per invocation. Shadow
has no caller-visible worker, lease, heartbeat, job, checkpoint, cancel, retry/requeue or source-name field.
Its payload contains only Complete/Yielded status, bounded scan counters and optional sealed continuation.

## Locale-safe continuation identity

`IndexSourceContinuationScope` now distinguishes scan scope as part of the encrypted claims:

- schema-wide -> `locale = None`;
- exact locale -> `locale = Some(LocaleKey)`.

`from_registry` constructs only schema-wide scope and `for_locale` constructs only exact canonical-locale scope.
Opening requires exact locale equality in addition to tenant/schema/source ownership. Schema-wide and locale
tokens cannot cross scopes, and different canonical locales cannot exchange tokens.

The continuation codec has one current unversioned envelope. The previous pre-release shape was replaced in place:
there is no version byte, `contract_version`, old-format claims type, or fallback decoder. Key rotation remains a
cryptographic-key concern only and does not create format compatibility.

Exact-locale Shadow transport remains source-open because locale still must be carried through
`IndexReplayDryRunRequest`, `SharedIndexReplayDryRunRuntime`, the sealed server adapter and authorization-first
GraphQL input. That next slice can now use `IndexSourceContinuationScope::for_locale` without changing the
continuation format again.

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
- exact-locale Shadow execution/GraphQL transport yet;
- token-format version families or legacy continuation decoders;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- automatic retry/requeue or scheduler ownership;
- a second durable ownership/fencing model.

## Next source boundary

The next independent Shadow boundary is exact-locale dry-run/runtime/GraphQL execution using the now-canonical
locale-safe continuation scope. Locale must be authorization-first, canonicalized through `LocaleKey`, carried by
`IndexReplayDryRunRequest`, applied through `IndexSourceScanRequest::for_locale`, and used to derive
`IndexSourceContinuationScope::for_locale` for open/seal. It still must not create job, checkpoint, lease,
cancellation or retry state.

Targeted execution remains a separate later slice because it needs an explicit bounded mutation-application
contract over `IndexSource::load` rather than a scan checkpoint.

Execution/admission remains maintainer-owned. Rust tests, Node verifiers, database scenarios and CI for this source
slice were not executed by the implementation agent.
