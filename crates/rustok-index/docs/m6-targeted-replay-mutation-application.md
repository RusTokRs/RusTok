# M6 targeted replay mutation application

Status: `source_complete_host_guard_transport_pending`.

## Purpose

`IndexReplayTargetedExecutor` defines the bounded mutation-application contract for
`IndexReplayMode::Targeted` without reusing the durable Full replay job/checkpoint state machine.
It consumes only `IndexReplayModeSelection::Targeted`, which owns the canonical
`IndexSourceLoadRequest` bounded tenant/schema/key-set validation.

The application contract is now composed into the canonical PostgreSQL replay runtime and exposed to
server callers only through request-bound `modules:manage` host dispatch. It is still not a public
transport and not a new durable replay owner.

## Canonical Targeted request and key admission

Targeted selection continues to reuse `IndexSourceLoadRequest` unchanged:

- 1 through 256 unique `EntityKey` values;
- one non-nil tenant;
- one exact schema;
- per-key locale identity remains part of each `EntityKey`;
- mixed tenant/schema keys and duplicate keys fail during request construction.

The generic load request intentionally does not own active-schema semantics. Before source resolution
or `IndexSource::load`, Targeted adds requested key admission against the active `SchemaRegistry`:

- every requested entity UUID must be non-nil;
- `LocaleMode::Required` requires every requested key to carry a locale;
- `LocaleMode::None` rejects every requested key that carries a locale;
- `LocaleMode::Optional` accepts either key shape.

This prevents malformed or locale-incompatible exact targets from being reinterpreted as ordinary
missing source keys.

`Full` and `Shadow` selections are rejected by `IndexReplayTargetedExecutor`; they cannot alias the
Targeted mutation path.

## Execution order

One Targeted invocation performs exactly these steps:

1. require the dedicated Targeted host method and canonical `IndexSourceLoadRequest`;
2. server `IndexReplayOperatorContext` checks exact tenant equality and current request-bound effective
   `modules:manage`;
3. `SharedIndexReplayRuntime::run_targeted` wraps the request as the canonical Targeted selection;
4. require the exact schema to exist in the active `SchemaRegistry`;
5. validate every requested entity/locale key against that active schema;
6. resolve the exact source owner from `SharedIndexSourceRegistry`;
7. call the canonical bounded `SharedIndexSourceRegistry::load` once;
8. preflight the complete returned batch before the first mutation write;
9. apply each returned mutation sequentially through the existing `IndexReplayMutationSink`;
10. return bounded counters only.

The source registry rejects mutations for unrequested keys and duplicate returned entity keys. The
Targeted executor then adds the replay-specific whole-batch preflight before persistence:

- every mutation event UUID must be non-nil;
- event UUIDs must be unique within the invocation;
- every mutation must pass complete `SchemaRegistry::validate_mutation` validation.

If host authorization, requested key admission, or any returned-batch preflight fails, no mutation sink
call occurs.

## PostgreSQL/runtime composition

`materialize_postgres_index_replay_runtime` assembles one `IndexReplayTargetedExecutor<PostgresMutationStore>`
from the same frozen `SharedIndexSourceRegistry`, immutable schema registry and host database already used by
replay composition. `SharedIndexReplayRuntime` stores that executor beside the durable Full runner and exposes
only a dedicated `run_targeted(IndexSourceLoadRequest)` method.

This is reuse of the existing mutation persistence contract, not a second durable replay state machine.
`PostgresMutationStore` derives each inbox delivery ID from the source-owned mutation event UUID through the
existing `IndexReplayMutationSink` implementation.

The server's existing `IndexReplayOperatorRuntime` owns Targeted dispatch. `run_targeted` first calls the same
`IndexReplayOperatorContext::authorize_for` exact-tenant/request-snapshot check used by Full replay and then
delegates to `SharedIndexReplayRuntime::run_targeted`. No separate Targeted permission, operator identity or
caller-controlled mode selector is introduced.

## Missing requested keys

`IndexSource::load` is allowed to return fewer mutations than requested keys. Once requested keys have
passed active-schema admission, Targeted execution does not infer deletion from source absence and does
not manufacture tombstones. The outcome reports
`missing_count = requested_count - mutation_count` only.

Owner adapters remain responsible for returning an authoritative delete mutation when deletion is the
owner contract for a requested key. Authorization-safe or otherwise intentional absence remains a
missing key, not a synthetic write.

## Mutation identity and partial failure

Targeted execution preserves the source-owned mutation event UUID. It does not generate a command UUID,
job UUID, checkpoint delivery identity, or transport-specific event identity.

With the PostgreSQL replay mutation sink, that source event UUID remains the `index_inbox` delivery
identity. A later exact Targeted invocation can therefore safely encounter `Duplicate` or `StaleIgnored`
after an earlier invocation partially committed mutations before a later mutation failed.

Retained source evidence models the harder window where mutation 1 succeeds and mutation 2 fails once;
the exact retry observes mutation 1 as `Duplicate` and applies mutation 2. The executor intentionally
does not add a checkpoint for partial progress. A failure reports the exact mutation position and
existing bounded replay failure. Retry/requeue policy remains an operator/transport concern and is not
automatic here.

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
- Targeted graceful-stop handling or a second lifecycle owner;
- scheduler/background worker ownership;
- a generic caller-controlled replay mode selector;
- GraphQL/HTTP/CLI/admin transport;
- partition replay scope;
- synthetic deletes for missing load keys.

## Next source boundary

The next independent boundary is a dedicated authorization-first Targeted public transport. It must keep
request tenant and actor server-owned, require `modules:manage` before parsing untrusted schema/entity/locale
targets, build only the canonical bounded exact-key request, delegate solely to
`IndexReplayOperatorRuntime::run_targeted`, and expose only bounded counters/failures.

Do not add a generic mode selector, caller-owned source name, worker, page budget, job/checkpoint identity,
lease, cancellation, retry/requeue state, partition scope or synthetic delete semantics in that transport.

## Validation ownership

Rust tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows and CI are
maintainer-run and were not executed by the implementation agent.
