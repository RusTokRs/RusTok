# M6 replay mode contract

Status: `source_complete_shadow_host_dispatch_transport_pending`.

This slice defines explicit rebuild mode identity without changing the existing durable replay runner, job,
checkpoint, cancellation, locale or lease state machines. The Shadow execution surface is now also bound to the
server-owned request authorization guard, while public transport remains separate.

## Mode identity

`IndexReplayMode` has exactly three modes:

- `Full` — cursor-based durable source scan. Its execution surface is `DurableScan` and the existing
  `PostgresIndexReplayRunner` remains the only admitted implementation.
- `Targeted` — bounded exact-key source load. Its execution surface is `TargetedLoad`; construction delegates to
  the canonical `IndexSourceLoadRequest`, so the existing 1..=256 key bound, exact tenant/schema scope and key
  uniqueness remain authoritative.
- `Shadow` — side-effect-free cursor scan. Its execution surface is `SideEffectFreeScan`, matching the existing
  `SharedIndexReplayDryRunRuntime` no-write boundary.

Mode is not locale scope and is not future partition scope. Adding a locale to a full scan does not create a new
mode, and adding partition replay later must not encode partition identity as a mode.

## Fail-closed routing

`IndexReplayModeSelection::is_admitted_to_durable_scan_runner` returns true only for `Full`.

Targeted and shadow modes must not alias the full durable job/checkpoint identity:

- targeted execution must be a bounded `IndexSource::load` path and must define its own operator execution
  contract before it can persist mutations;
- shadow execution remains no-write and uses the dry-run surface rather than the durable mutation/job/checkpoint
  path;
- neither mode introduces automatic retry/requeue, another lease owner, another terminal job state, or a second
  cancellation model.

The current `IndexReplayRunRequest`, `PostgresIndexReplayRunner` and GraphQL `runIndexReplay` command remain
full-scan behavior. They do not accept a generic mode selector and are not reinterpreted by this contract.

## Shadow host dispatch

`Shadow` host dispatch is now source-complete through `IndexReplayOperatorRuntime::run_shadow`.

The server replay materializer retrieves the `SharedIndexReplayDryRunRuntime` already published by Index replay
composition and stores it beside the durable runtime inside the guarded operator. `run_shadow` authorizes the
exact request tenant through the same request-bound `modules:manage` snapshot used by Full replay, then delegates
to the no-write dry-run runtime.

This is a host dispatch boundary, not a new durable mode state machine:

- no job or checkpoint identity is created for Shadow;
- no lease, heartbeat, cancel or terminal job transition is added;
- no mutation sink or database connection is exposed;
- Full continues to route to the durable runner unchanged;
- GraphQL transport remains separate and does not yet expose Shadow.

## Existing contracts reused

The mode contract composes already-retained boundaries rather than duplicating them:

- full: durable fenced replay job/checkpoint runner, optional canonical locale and page lease-heartbeat policy;
- targeted: `IndexSourceLoadRequest` exact-key validation and `IndexSource::load` source boundary;
- shadow: `IndexReplayDryRunRequest` / `SharedIndexReplayDryRunRuntime` bounded side-effect-free scan validation,
  now guarded by the server replay operator.

Partition replay remains blocked until a real partition-capable source can filter before pagination.

## Non-goals

This slice does not add:

- targeted mutation execution;
- shadow persistence or shadow tables;
- GraphQL/API Shadow transport or generic mode selection;
- a mode column in `index_jobs` or `index_checkpoints`;
- partition replay scope;
- automatic retry/requeue or scheduler ownership;
- a second durable ownership/fencing model.

## Next source boundary

The next source-only boundary is authorization-first GraphQL transport for the guarded Shadow command. It should
call `IndexReplayOperatorRuntime::run_shadow`, preserve the existing Full `runIndexReplay` path, keep resume state
caller-carried and non-durable, and add canonical locale scope only through the existing source-scan contract rather
than by creating mode-specific job/checkpoint state.

Targeted execution remains a separate later slice because it needs an explicit bounded mutation-application
contract over `IndexSource::load` rather than a scan checkpoint.

Execution/admission remains maintainer-owned. The Rust tests and Node verifiers for this source slice were not
executed by the implementation agent.
