# M6 targeted replay mutation application

Status: `source_complete_transport_execution_pending`.

## Purpose

`IndexReplayTargetedExecutor` defines bounded exact-key mutation application for
`IndexReplayMode::Targeted` without reusing the durable Full replay job/checkpoint state machine.
It consumes only `IndexReplayModeSelection::Targeted`, which owns the canonical
`IndexSourceLoadRequest` tenant/schema/key-set validation.

The application contract is composed into the canonical PostgreSQL replay runtime, guarded by the
request-bound server replay operator, and exposed through one dedicated authorization-first GraphQL
mutation. It is not a new durable replay owner and does not reinterpret `runIndexReplay` with a mode
field.

## Canonical Targeted request and key admission

Targeted continues to reuse `IndexSourceLoadRequest` unchanged:

- 1 through 256 unique `EntityKey` values;
- one non-nil tenant;
- one exact schema;
- per-key locale identity remains part of each `EntityKey`;
- mixed tenant/schema keys and duplicate canonical keys fail during request construction.

The generic load request intentionally does not own active-schema semantics. Before source resolution
or `IndexSource::load`, `IndexReplayTargetedExecutor` adds requested key admission against the active
`SchemaRegistry`:

- every requested entity UUID must be non-nil;
- `LocaleMode::Required` requires every requested key to carry a locale;
- `LocaleMode::None` rejects every requested key that carries a locale;
- `LocaleMode::Optional` accepts either key shape.

This prevents malformed or locale-incompatible exact targets from being reinterpreted as ordinary
missing source keys.

`Full` and `Shadow` selections are rejected by `IndexReplayTargetedExecutor`; they cannot alias the
Targeted mutation path.

## Execution order

One Targeted GraphQL invocation performs these boundaries in order:

1. derive tenant/actor from `TenantContext` / `AuthContext`;
2. require the current request-bound effective `modules:manage` snapshot before parsing Targeted schema/entity/locale strings;
3. parse one exact schema routing identity;
4. require a bounded 1..=256 `targets` list;
5. parse every target UUID and canonicalize its optional locale through `LocaleKey`;
6. construct the canonical `IndexSourceLoadRequest` from server-owned tenant, parsed schema and exact keys;
7. call only `IndexReplayOperatorRuntime::run_targeted`;
8. the operator repeats exact tenant plus `modules:manage` authorization before runtime execution;
9. `SharedIndexReplayRuntime::run_targeted` selects the canonical Targeted execution surface;
10. require the exact schema in the active `SchemaRegistry` and validate every requested entity/locale key;
11. resolve the exact source owner and perform one bounded canonical `load`;
12. preflight the complete returned batch before the first mutation write;
13. apply returned mutations through the existing `IndexReplayMutationSink`;
14. return bounded counters only.

The source registry rejects mutations for unrequested keys and duplicate returned entity keys. The
Targeted executor additionally requires every event UUID to be non-nil and invocation-unique and every
mutation to pass complete `SchemaRegistry::validate_mutation` validation before persistence begins.

If transport authorization, exact-target request construction, active-schema admission, or returned
batch preflight fails, no mutation sink call occurs.

## PostgreSQL/runtime composition

`materialize_postgres_index_replay_runtime` assembles one
`IndexReplayTargetedExecutor<PostgresMutationStore>` from the same frozen
`SharedIndexSourceRegistry`, immutable schema registry and host database already used by replay
composition. `SharedIndexReplayRuntime` stores that executor beside the durable Full runner and
exposes only a dedicated `run_targeted(IndexSourceLoadRequest)` method.

This reuses the existing mutation persistence contract; it is not a second durable replay state
machine. `PostgresMutationStore` derives each inbox delivery identity from the source-owned mutation
event UUID through the existing `IndexReplayMutationSink` implementation.

The server's existing `IndexReplayOperatorRuntime` owns Targeted dispatch. `run_targeted` first calls
the same `IndexReplayOperatorContext::authorize_for` exact-tenant/request-snapshot check used by Full
replay and then delegates to `SharedIndexReplayRuntime::run_targeted`. Targeted failures use a separate
`IndexReplayTargetedOperatorError`, so Full/cancel GraphQL error mapping remains unchanged.

## GraphQL transport

`runIndexReplayTargeted(input: ...)` is a dedicated command rather than a caller-controlled generic
replay mode. Its input contains only:

- module name;
- entity name;
- positive schema routing key;
- `targets`, each with one entity UUID and one optional canonicalizable locale.

Tenant and actor are server-owned. Target count is bounded to the same canonical maximum of 256 before
per-key parsing. Locale is canonicalized separately for every exact key, so aliases such as `EN-us`
and `en-US` collapse to one `EntityKey` identity and duplicate canonical targets fail closed.

The GraphQL payload contains only:

- requested key count;
- returned mutation count;
- missing key count;
- applied count;
- duplicate count;
- stale-ignored count.

The application outcome internally carries the resolved source name for diagnostics, but GraphQL does
not expose it. Source routing remains server-owned.

Transport preparation errors are authorization-first. Unknown/unregistered schemas and active-schema
invalid targets are reported as bad user input; source contract failures, invalid returned mutations
and persistence failures remain generic internal command failures rather than leaking owner/storage
details.

## Missing requested keys

`IndexSource::load` may return fewer mutations than requested keys. Once requested keys pass
active-schema admission, Targeted does not infer deletion from source absence and does not manufacture
tombstones. The outcome reports `missing_count = requested_count - mutation_count` only.

Owner adapters remain responsible for returning an authoritative delete mutation when deletion is the
owner contract for a requested key. Authorization-safe or otherwise intentional absence remains a
missing key, not a synthetic write.

## Mutation identity and partial failure

Targeted execution preserves the source-owned mutation event UUID. It does not generate a command UUID,
job UUID, checkpoint delivery identity, or transport-specific event identity.

With the PostgreSQL replay mutation sink, that event UUID remains the `index_inbox` delivery identity.
A later exact Targeted invocation can therefore safely encounter `Duplicate` or `StaleIgnored` after an
earlier invocation partially committed mutations before a later mutation failed.

Retained source evidence models the harder window where mutation 1 succeeds and mutation 2 fails once;
the exact retry observes mutation 1 as `Duplicate` and applies mutation 2. The executor intentionally
does not add a checkpoint for partial progress. Retry/requeue remains an operator decision and is not
automatic here.

## Explicitly absent

Targeted still has no:

- durable job or checkpoint;
- lease/heartbeat/fencing state;
- cancellation or automatic retry/requeue;
- graceful-stop handling or second lifecycle owner;
- scheduler/background worker ownership;
- generic caller-controlled replay mode selector;
- caller-controlled source name;
- caller-controlled worker/page budget;
- raw source payload/result exposure;
- HTTP/CLI/admin transport added by this slice;
- partition replay scope;
- synthetic deletes for missing load keys.

## Next gate

The independent Targeted source chain is complete through application execution, PostgreSQL mutation
composition, request-bound host dispatch and dedicated GraphQL transport. The next Targeted step is
maintainer execution/admission of the transport and PostgreSQL behavior; source inspection alone must
not claim that evidence.

No additional source-only M6 replay expansion is justified until either execution/admission exposes a
concrete defect or a real partition-capable source contract can filter partition scope before
pagination.

## Validation ownership

Rust tests, Node verifiers, Cargo checks, formatting, database scenarios, workflows and CI are
maintainer-run and were not executed by the implementation agent.
