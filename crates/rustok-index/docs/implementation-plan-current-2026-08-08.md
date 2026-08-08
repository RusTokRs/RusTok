# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked at `main@232aeb8c936964c0f76d1a7661379619199f1d49` and continued on
`agent/index-replay-pending-future-timeouts-20260808`.

`implementation-plan.md` remains historical architecture context. This file is the current execution cursor.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

Repair implementation/harness/admission source remains complete. Maintainer execution/admission is still
required and is not claimed by source inspection.

## M7 Product Storefront source state

Source-complete:

- one current 15-field Product schema on routing key `4`; lower keys are historical storage identities only;
- schema-scoped Product replay IDs, Product owner clock and canonical typed `attribute_terms`;
- Product channel relation/freshness materialization;
- localized identity fold, cursor v3 and requested -> fallback projection;
- localized PostgreSQL compiler/decoder/runtime with readiness/admission and repeatable-read page/count snapshot;
- Product-owned 1022-byte Storefront title-search bound compatible with generic 1024-byte `TextLike`;
- retained owner/default-vs-Index-`C` PostgreSQL title-search collation packet source;
- Product-owned Storefront EAV filter -> neutral canonical term resolution;
- pure Storefront shadow builder and owner-first non-serving evidence executor;
- channel-less current-key policy: trusted slug/UUID is eligible; channel-less remains owner-native;
- deep-page policy: offsets through `10_000` are eligible; deeper owner-valid pages remain owner-native;
- post-page Product public title/handle placeholder projection while preserving raw Index null evidence;
- bounded Product-owned post-page tag hydration keyed by selected Product IDs, including Taxonomy fallback and
  legacy metadata-only tags;
- host-measured post-owner serving-budget eligibility policy without hard-coded production SLO values;
- non-serving post-owner budgeted execution that accepts only `Eligible`, narrows phase port deadlines and
  enforces outer Tokio timeouts for raw Index/EAV execution and Product tag hydration;
- deterministic storage-free timeout evidence source for that real budgeted adapter through a crate-private
  post-owner phase seam implemented in production by `ProductStorefrontIndexShadowExecutor`;
- explicit current Product key-4 tenant promotion contract: ordinary-register/stage, schema-scoped rebuild,
  evidence admission, `register_current`, lower-key retirement and old-key fail-closed readiness/query behavior;
- retained Product key-4 PostgreSQL promotion/restart packet source using production migrations, current
  distribution schema/source/query composition, mutation storage, `register_current`, typed inactive probes and
  fresh restart composition;
- current-key core/EAV Storefront PostgreSQL packet source;
- historical retained Product PostgreSQL fixtures actualized to current Product routing key `4`.

## Request-shape owner-native policies

Channel-less owner semantics are metadata-unrestricted only and cannot be recovered exactly from current
`sales_channel_ids`, so channel-less stays owner-native on key `4`.

The Product owner has no generic Index offset ceiling, while Index offset is bounded to `10_000`; deeper valid
pages therefore stay owner-native without clamp or cursor rewrite.

## Post-page Product projection

Raw `projected` remains the generic Index page and the source for identity/order/count/page comparison.
`public_projected` maps only no-localized-row title/handle nulls to Product public placeholders after the raw
page is fixed.

`ProductStorefrontTagReadPort` hydrates tags using already-selected Product IDs rather than only `tag_ids`, so
Product relation ordering, Taxonomy requested->fallback/canonical-key resolution and legacy `metadata.tags`
fallback remain owner semantics. Embedded runtime selects the capability; external profiles have no implicit
embedded fallback.

## Serving budget, timeout enforcement and retained evidence

`PortContext.deadline_ms` is the original duration budget. A future host/router must measure `remaining_ms`
after owner success. `ProductStorefrontIndexServingBudget` carries host-selected positive Index/tag phase
budgets plus safety margin; classification stays owner-native when timing/capability state is missing,
inconsistent or insufficient.

`ProductStorefrontIndexBudgetedProjectionExecutor` is post-owner only: it receives the already-successful owner
page, rejects non-`Eligible` decisions before work starts, narrows the projected phase context and applies an
outer timeout, then separately narrows and times Product tag hydration. Public placeholder mapping occurs only
after successful raw projection. Timeout/error results remain separate and never replace owner success.

`ProductStorefrontIndexProjectionPhases` isolates only those two post-owner phases. Production uses the real
shadow executor. The retained packet substitutes deterministic fake phases and covers non-eligible no-work,
projected timeout/error, tag timeout, exact phase deadlines and the fast identity/count/public/tag path. The
packet is source-complete but has **not** been executed or admitted by the implementation agent.

Mounted Storefront still uses only Product owner reads and does not call either serving-budget classification or
budgeted execution.

## Product key-4 tenant promotion

Current Product code has already completed the source-code replacement from lower historical keys to one current
key `4`. `product-postgres-primary` uses `derive_index_schema_source_event_id`; Product source, absence and query
admission do not select key `3`.

The retained PostgreSQL packet covers the staged authority-transition path in source:

1. obtain the exact current Product schema from `SharedIndexSchemaRegistry` and assert key `4`;
2. ordinary-register a storage-only lower-key fixture plus the actual current runtime schemas, leaving both
   Product keys active during staging;
3. load one real current Product mutation through `product-postgres-primary` and prove its schema-scoped event ID
   differs from the same owner coordinates under key `3`;
4. materialize/query key `4` through production mutation/query paths;
5. call `PostgresSchemaRegistrationStore::register_current` on the already-staged key `4` contract;
6. require one lower active Product key to retire and repeated promotion to retire zero more;
7. require typed `Inactive` from readiness and query verification for the lower storage contract;
8. rebuild distribution/query composition on a separate connection and require exactly one runtime Product
   schema, key `4`, to read the retained current materialization.

The lower-key contract is deliberately a test-only storage/probe fixture derived from the current immutable
contract with a lower routing key. It does not reconstruct the historical key3 Product fingerprint and does not
register a key3 Product source/factory. Runtime dual-read compatibility remains forbidden.

## M6 replay command transport

`IndexReplayOperatorRuntime` remains the only server-owned replay invocation authority. It requires exact
request-bound tenant/actor context and effective `modules:manage`, rejects cross-tenant run requests and derives
cancel tenant from that context.

The GraphQL layer exposes source-complete schema-wide `runIndexReplay` / `cancelIndexReplay` commands without
calling source adapters or PostgreSQL directly. Authorization occurs before parsing untrusted schema/job input.

Caller input contains no tenant, actor, worker ID, source name, locale/partition, scheduler handle, replay
resource budget, stop handle or shutdown flag. Each run creates a server-owned worker identity and applies fixed
transport caps: 100 mutations per page, at most 8 pages, heartbeat every page and a 60-second lease.

GraphQL runtime execution/cancellation evidence remains maintainer-owned.

## M6 replay graceful interruption and server lifecycle binding

`PostgresIndexReplayRunner::run_interruptible` carries one host-owned synchronous probe into the existing
one-page safe points: before source scan, before each mutation, and before checkpoint commit. Ordinary `run` and
persisted operator cancellation semantics remain unchanged.

An `IndexReplayError::Interrupted` first preserves any persisted cancellation race. Otherwise the same fenced
job is yielded back to `pending`, lease ownership is cleared, no failure payload is recorded, and the last
committed checkpoint is preserved.

Retained source evidence now covers both runner and command boundaries:

- runner-level SQLite evidence covers stop before scan and the harder durable-mutation-before-checkpoint window,
  where attempt 2 replays the stable delivery as `Duplicate` before checkpoint/job completion;
- GraphQL schema-data evidence runs the real `IndexReplayMutation` with request-scoped RBAC, coordinates source
  scan through `Notify`, invokes the shared `StopHandle::stop()`, requires GraphQL `YIELDED` plus an uncancelled
  lease-free pending attempt-1 job with no checkpoint/mutation, then constructs fresh runtime/operator/GraphQL
  state over the same SQLite database and requires the same job to complete as attempt 2.

The GraphQL packet uses no wall-clock sleeps or polling and intentionally does **not** claim full HTTP/process
bootstrap execution. Production schema lifecycle reservation/keepalive remains separately source-guarded.

Neither retained shutdown packet nor a full process-shutdown replay scenario has been executed or admitted by
the implementation agent.

## M6 replay pending storage-future timeout boundary

The PostgreSQL replay adapter now bounds the two storage futures that could remain pending after a page has
already entered a durable phase:

- one replay mutation persistence call;
- one replay checkpoint commit transaction.

`source_replay_timeout.rs` applies a canonical 30-second outer bound and stable retryable dependency identities:
`index_replay_mutation_timeout` and `index_replay_checkpoint_commit_timeout`.

The one-page worker and multi-page runner error/state surfaces are unchanged. Mutation timeout still appears as
`IndexReplayError::MutationFailed`; checkpoint timeout still appears as `IndexReplayError::CheckpointCommitFailed`.
The existing runner records the dependency code plus `retryable: true`, but first rechecks persisted cancellation,
so a user cancellation that won the race remains `Cancelled`.

These are observation bounds, not rollback guarantees. Dropping the timed-out future does not prove that the
underlying database operation was cancelled. No synthetic checkpoint is created after mutation timeout. A timed
checkpoint commit is treated as storage-state-unknown: the next admitted attempt must read the durable checkpoint
normally under the existing lease fence.

The timeout helper contains no `StopHandle`, `request_cancel`, `cancel_requested` or yield semantics. Graceful
shutdown remains safe-point-only, while replay retry/recovery policy remains the owner of later retry admission.

Retained timeout unit source and source guard are complete but have **not** been executed or admitted by the
implementation agent. See `m6-replay-pending-future-timeouts.md`.

## Remaining Storefront parity/evidence blockers

- execute/admit the deterministic budgeted timeout packet and retain acceptable runtime latency/cancellation
  evidence for the selected deployment profile;
- execute/admit the retained Product key-4 promotion/restart PostgreSQL packet;
- execute/review current-key Storefront core/EAV/collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the deployment default-vs-`C` packet agrees;
- complete maintainer-executed stale locale/readiness/admission/restart evidence beyond the focused promotion
  packet;
- perform a real tenant stage/rebuild/`register_current` only after the relevant packets are admitted;
- move only eligible Storefront traffic after every evidence/latency gate; channel-less/deep pages stay owner-native.

## M5 incremental ingestion

- [x] Source replay registry and bounded failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation event orchestration and exact source refresh.
- [x] Product locale/ProductVariant refresh ledgers and durable relay.
- [ ] Execute canonical event-contract digest admission on reviewed `main`.
- [ ] Add canonical Product Index typed event family only after digest admission.
- [ ] Retain commit/ack crash-redelivery evidence for that route.

## M6 replay/reconciliation/repair

- [x] Bounded replay, durable jobs/leases/checkpoints and cancellation.
- [x] Reconciliation, drift diagnosis and targeted repair source.
- [x] Real-migration repair PostgreSQL harness and retained-evidence admission tooling.
- [x] Guarded schema-wide replay run/cancel GraphQL command transport with server-owned bounded run policy.
- [x] Carry a host-owned interruption probe through replay runner safe points and retain duplicate-safe restart evidence source.
- [x] Bind the shared server `StopHandle::is_stopping` signal through guarded replay runtime/operator composition without caller shutdown controls.
- [x] Retain deterministic GraphQL StopHandle -> pending -> fresh-runtime attempt-2 completion evidence source without sleeps/polling.
- [x] Bound replay mutation/checkpoint-commit pending futures with retryable timeout identities and preserve cancel/lease precedence.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Execute/admit replay GraphQL transport behavior and cancellation evidence.
- [ ] Execute/admit retained graceful interruption/restart and GraphQL shutdown evidence.
- [ ] Execute/admit retained mutation/checkpoint pending-future timeout evidence.
- [ ] Define/retain whole-page duration versus lease/heartbeat policy beyond per-dependency bounds.
- [ ] Complete remaining multi-host/restart evidence beyond existing convergence/replay packets.
- [ ] Add locale/partition replay checkpoint dimensions and explicit rebuild modes under separate contracts.

## M7 Product Storefront graph

- [x] Current Product/ProductVariant/SalesChannel sources and graph freshness.
- [x] One current 15-field Product contract and schema-safe replacement mechanism.
- [x] Canonical typed EAV terms and Product owner clock.
- [x] Localized identity/fallback architecture and PostgreSQL query/runtime contract.
- [x] Generic scalar String `TextLike` and Product-owned compatible search bound.
- [x] Retain owner/default-vs-Index-`C` title-search collation packet source.
- [x] Product Storefront shadow query builder and Product-owned EAV resolution.
- [x] Compose non-serving Product-owner + Index evidence executor.
- [x] Retain current-key core/EAV owner-vs-shadow PostgreSQL packet source.
- [x] Actualize historical retained Product PostgreSQL packets to routing key `4`.
- [x] Keep channel-less Storefront owner-native on current key `4`.
- [x] Keep owner-valid offsets above `10_000` owner-native without clamp/rewrite.
- [x] Map raw title/handle nulls to Product public placeholders only post-page.
- [x] Batch-hydrate Product tags post-page through Product owner capability, including legacy metadata fallback.
- [x] Define host-measured post-owner serving-budget eligibility.
- [x] Enforce admitted Index/tag phase timeouts in a separate non-serving post-owner adapter.
- [x] Retain deterministic timeout/error/deadline/fast-path evidence source for the budgeted adapter.
- [x] Define/guard the current Product key-4 tenant staging/rebuild/final-supersession contract.
- [x] Retain a focused Product key-4 stage/replay/promote/inactive-old-key/restart PostgreSQL packet source.
- [ ] Execute/admit the retained budgeted timeout evidence.
- [ ] Execute/admit the Product key-4 promotion/restart PostgreSQL packet.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Stage/rebuild/promote a real Product tenant only after evidence admission.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code boundary

M7 Storefront remains execution/admission-gated and must not gain a traffic switch from source inspection alone.

After this timeout slice, the next independent M6 source boundary is replay checkpoint scope. The persistence table
already reserves `locale_key` and `partition_key`, but the current replay request/source-scan contract is
schema-wide and the runtime key still writes empty values. Do not merely populate those columns. First define a
real locale-scoped replay request/scan identity end-to-end; only then let checkpoint/job identity carry that scope.
Partition scope must remain separate until the source contract can actually scan a partition.

Explicit targeted/full/shadow rebuild modes remain a later separate contract and must not be smuggled into locale
checkpoint work.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
