# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked at `main@5c0a50eded09efbd4f1b6437b46a94a7f2ef0989` and continued on
`agent/index-replay-page-lease-heartbeat-20260808`.

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

The GraphQL layer exposes source-complete schema-wide or exact-locale `runIndexReplay` plus `cancelIndexReplay`
without calling source adapters or PostgreSQL directly. Authorization occurs before parsing untrusted schema,
locale or job input.

Caller run input contains only module/entity/schema key plus one optional bounded locale. Tenant, actor, worker
ID, source name, partition, scheduler handle, replay resource budget, stop handle and shutdown flag remain
server-owned or unavailable. Locale is canonicalized through `LocaleKey` after authorization. Omission remains
exactly schema-wide; it is not rewritten from schema locale defaults. Each run creates a server-owned worker
identity and applies fixed transport caps: 100 mutations per page, at most 8 pages, heartbeat every page and a
60-second lease.

Retained command source evidence includes schema-wide command behavior, locale command canonicalization and a
dedicated locale yield/isolation/fresh-runtime resume packet. GraphQL execution/admission remains maintainer-owned.

## M6 replay graceful interruption and server lifecycle binding

`PostgresIndexReplayRunner::run_interruptible` carries one host-owned synchronous probe into the existing
one-page safe points: before source scan, before each mutation, and before checkpoint commit. Ordinary `run` and
persisted operator cancellation semantics remain unchanged.

An `IndexReplayError::Interrupted` first preserves any persisted cancellation race. Otherwise the same fenced
job is yielded back to `pending`, lease ownership is cleared, no failure payload is recorded, and the last
committed checkpoint is preserved.

Retained source evidence covers both runner and command boundaries:

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

## M6 replay dependency timeouts and page lease-heartbeat policy

Production replay now has a bounded outer observation window for every dependency phase that can otherwise remain
pending inside one page:

- production source scan: 30 seconds through the canonical `IndexSource` timeout wrapper;
- checkpoint read transaction: 30 seconds with `index_replay_checkpoint_read_timeout`;
- one replay mutation persistence call: 30 seconds with `index_replay_mutation_timeout`;
- checkpoint commit transaction: 30 seconds with `index_replay_checkpoint_commit_timeout`.

These are observation bounds, not rollback guarantees. Dropping a timed-out future does not prove that an
underlying database operation was cancelled. Stable delivery identity, inbox deduplication, monotonic source
versions, durable checkpoint reads and active-lease fencing remain authoritative.

A coarse whole-page timeout is deliberately not added because it could mask the precise dependency failure code.
Instead, `IndexReplayRunRequest` now requires at least a 60-second lease and both ordinary/graceful runners
maintain an active page lease every one third of the configured lease duration. The server-owned 60-second lease
therefore heartbeats a still-running page every 20 seconds. Existing page-count heartbeat cadence remains intact.

Checkpoint-read bounding closes the remaining replay data-plane future that could otherwise remain pending while
lease heartbeats continued. The timeout helper still owns no `StopHandle`, persisted cancellation or retry/requeue
policy. Persisted cancellation is rechecked before terminal page failure; graceful shutdown remains worker
safe-point-only.

Retained source assertions and guards are complete but have **not** been executed/admitted by the implementation
agent. See `m6-replay-pending-future-timeouts.md` and `m6-replay-page-lease-heartbeat.md`.

## M6 locale-scoped replay source state

Locale replay is split from partition/rebuild-mode work and tracked end-to-end instead of as one aggregate
future item.

Merged source-complete chain:

- generic `IndexSourceScanRequest` accepts optional canonical `LocaleKey` and validates returned mutation scope;
- current Product PostgreSQL replay scans honor exact locale before pagination while preserving the schema-wide
  `(product_id, locale)` path;
- durable replay jobs support explicit locale scope through the locale job migration, canonical locale request
  contract, lease identity, advisory locking, active-job lookup and locale-specific completion probe;
- one-page replay requests/checkpoint keys carry the same optional canonical locale;
- one-page worker resolves the exact runtime schema before checkpoint/source work, rejects locale scope for
  `LocaleMode::None`, and derives source scan plus checkpoint from the same request locale;
- PostgreSQL checkpoint storage requires exact lease/checkpoint locale equality and persists canonical locale
  while schema-wide checkpoints retain the historical empty locale string;
- optional locale is carried through the multi-page replay runner and GraphQL command transport;
- ordinary and graceful runner paths derive the durable job lease from the same page locale scope;
- terminal success binds the leased locale checkpoint instead of hard-coding the schema-wide empty locale;
- GraphQL locale parsing remains authorization-first and locale omission remains exactly schema-wide;
- retained GraphQL/runtime/runner evidence forces bounded `en-US` yield, completes distinct `de` and schema-wide
  jobs, rebuilds runtime composition and resumes the same `en-US` job as attempt 2 with duplicate-safe redelivery;
- final evidence retains exactly three job/checkpoint scopes: schema-wide, `en-US` and `de`.

The retained locale packet is source-complete but has not been executed. SQLite/PostgreSQL runtime execution and
production admission remain maintainer-owned. See `m6-locale-replay-command-evidence.md`.

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
- [x] Guarded replay run/cancel GraphQL command transport with server-owned bounded run policy.
- [x] Carry a host-owned interruption probe through replay runner safe points and retain duplicate-safe restart evidence source.
- [x] Bind the shared server `StopHandle::is_stopping` signal through guarded replay runtime/operator composition without caller shutdown controls.
- [x] Retain deterministic GraphQL StopHandle -> pending -> fresh-runtime attempt-2 completion evidence source without sleeps/polling.
- [x] Bound replay checkpoint-read/mutation/checkpoint-commit pending futures with retryable timeout identities and preserve cancel/lease precedence.
- [x] Define canonical locale-scoped source scan and make current Product filter before pagination.
- [x] Add durable locale replay job scope and exact locale checkpoint identity.
- [x] Carry locale through one-page replay worker/checkpoint storage with fail-closed `LocaleMode` admission.
- [x] Carry optional locale through the multi-page replay runner and GraphQL command transport.
- [x] Retain deterministic locale replay/restart command evidence through the real GraphQL/runtime/runner path.
- [x] Define/retain whole-page duration versus lease/heartbeat policy beyond per-dependency bounds.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Execute/admit replay GraphQL transport behavior and cancellation evidence.
- [ ] Execute/admit retained graceful interruption/restart and GraphQL shutdown evidence.
- [ ] Execute/admit retained dependency pending-future timeout evidence.
- [ ] Execute/admit retained page lease-heartbeat evidence.
- [ ] Execute/admit retained locale replay/restart command evidence, including schema/locale isolation.
- [ ] Complete remaining multi-host/restart evidence beyond existing convergence/replay packets.
- [ ] Add partition replay scope only after a real partition-capable source contract exists.
- [ ] Add explicit targeted/full/shadow rebuild modes under a separate contract.

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

The locale request/source/job/checkpoint/runner/GraphQL identity chain, deterministic locale command/restart
evidence, dependency timeout set and page-duration/lease-heartbeat source policy are complete. Their next actions
are maintainer execution/admission, not additional scope abstraction.

For source-only M6 continuation, the next independent boundary is the remaining multi-host/restart evidence under
the established lease, cancellation, graceful-stop and locale identities. Any retained packet must prove existing
fencing/reclaim behavior rather than introduce automatic retry or a second job-ownership model.

Partition remains separate and blocked. Add partition replay scope only after a real partition-capable source
contract exists and can filter before pagination; do not merely populate `partition_key`. Explicit
targeted/full/shadow rebuild modes remain a later separate contract and must not be smuggled into timeout,
partition or locale evidence work.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
