# M6 targeted replay mutation application

Status: `source_complete_host_guard_pending`.

## Purpose

`IndexReplayTargetedExecutor` defines the bounded mutation-application contract for
`IndexReplayMode::Targeted` without reusing the durable Full replay job/checkpoint state machine.
It consumes only `IndexReplayModeSelection::Targeted`, which already owns the canonical
`IndexSourceLoadRequest` exact-key validation.

This is an application execution boundary, not a public transport and not a new durable replay
owner. PostgreSQL/runtime materialization and request-bound host dispatch remain separate follow-up
slices.

## Canonical Targeted request

Targeted selection continues to reuse `IndexSourceLoadRequest` unchanged:

- 1 through 256 unique `EntityKey` values;
- one non-nil tenant;
- one exact schema;
- per-key locale identity remains part of each `EntityKey`;
- mixed tenant/schema keys and duplicate keys fail before source execution.

`Full` and `Shadow` selections are rejected by `IndexReplayTargetedExecutor`; they cannot alias the
Targeted mutation path.

## Execution order

One Targeted invocation performs exactly these steps:

1. require `IndexReplayModeSelection::Targeted`;
2. resolve the exact source owner from `SharedIndexSourceRegistry`;
3. require the exact schema to exist in the active `SchemaRegistry`;
4. call the canonical bounded `SharedIndexSourceRegistry::load` once;
5. preflight the complete returned batch before the first mutation write;
6. apply each returned mutation sequentially through the existing `IndexReplayMutationSink`;
7. return bounded counters only.

The source registry already rejects mutations for unrequested keys and duplicate returned entity keys.
The Targeted executor adds the replay-specific preflight used before persistence:

- every mutation event UUID must be non-nil;
- event UUIDs must be unique within the invocation;
- every mutation must pass complete `SchemaRegistry::validate_mutation` validation.

If any preflight check fails, no mutation sink call occurs.

## Missing requested keys

`IndexSource::load` is allowed to return fewer mutations than requested keys. Targeted execution does
not infer deletion from that absence and does not manufacture tombstones. The outcome reports
`missing_count = requested_count - mutation_count` only.

Owner adapters remain responsible for returning an authoritative delete mutation when deletion is the
owner contract for a requested key. Authorization-safe or otherwise intentional absence remains a
missing key, not a synthetic write.

## Mutation identity and partial failure

Targeted execution preserves the source-owned mutation event UUID. It does not generate a command UUID,
job UUID, checkpoint delivery identity, or transport-specific event identity.

When backed by the existing PostgreSQL replay mutation sink, that source event UUID remains the
`index_inbox` delivery identity. A later exact Targeted invocation can therefore safely encounter
`Duplicate` or `StaleIgnored` after an earlier invocation partially committed mutations before a later
mutation failed.

The executor intentionally does not add a checkpoint for partial progress. A failure reports the exact
mutation position and existing bounded replay failure. Retry/requeue policy remains an operator/host
concern and is not automatic here.

## Outcome

A successful `IndexReplayTargetedOutcome` exposes only:

- resolved source name;
- requested key count;
- returned mutation count;
- missing key count;
- applied count;
- duplicate count;
- stale-ignored count.

It exposes no source payloads, database handles, SQL, job identity, checkpoint, lease owner, worker,
retry state, cancellation state, scheduler handle, or partition scope.

## Explicitly absent

This slice does not add:

- durable Targeted jobs or checkpoints;
- lease/heartbeat/fencing state;
- Targeted cancellation or automatic retry/requeue;
- scheduler/background worker ownership;
- a generic caller-controlled replay mode selector;
- GraphQL/HTTP/CLI/admin transport;
- request-bound host authorization;
- partition replay scope;
- synthetic deletes for missing load keys.

## Next source boundary

The next independent boundary is PostgreSQL/runtime composition plus request-bound server host dispatch
for this executor. That slice should reuse `PostgresMutationStore` as the existing
`IndexReplayMutationSink`, expose only a guarded Targeted capability under the same effective
`modules:manage` authority as Full/Shadow, and still avoid durable scan jobs/checkpoints, leases,
cancellation and automatic retry ownership.

Public GraphQL transport should remain separate until the guarded host capability is source-complete.

## Validation ownership

Rust tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows and CI are
maintainer-run and were not executed by the implementation agent.
