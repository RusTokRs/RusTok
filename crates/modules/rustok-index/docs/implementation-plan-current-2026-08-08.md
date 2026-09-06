# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked at `main@97c01e2df66156f449b796f1349252da67a94bc3` and continued on
`agent/index-plan-post-targeted-recheck-20260809`.

`implementation-plan.md` remains historical architecture context. This file is the current execution cursor.

Post-Targeted recheck: #3360 merged `runIndexReplayTargeted` at
`e7b6a135692047398b95966491dbcbb06347da31`. Main then advanced to
`97c01e2df66156f449b796f1349252da67a94bc3` through Forum notification reconciliation, Blog Translation
PostgreSQL evidence, and Commerce Payment owner-port work only. The compare from the Targeted merge to current
`main` contains no `rustok-index` path and no Index replay transport path, so it opens no new Index source-code
boundary and does not relax any execution/admission gate below.

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

`IndexReplayOperatorRuntime` remains the server-owned replay invocation authority. It requires exact request-bound
tenant/actor context and effective `modules:manage`, rejects cross-tenant run requests and derives cancel tenant
from that context.

`SharedIndexReplayRuntime` contains two distinct mutation-capable execution surfaces assembled from the same
frozen source/schema registries and host database:

- durable Full through `PostgresIndexReplayRunner`;
- bounded exact-key Targeted through `IndexReplayTargetedExecutor<PostgresMutationStore>`.

`SharedIndexReplayDryRunRuntime` remains the separate no-write Shadow surface. The guarded server operator exposes
dedicated Full, Targeted and Shadow methods rather than a generic mode selector. `run_targeted` checks exact
request tenant plus the current request-bound effective `modules:manage` snapshot before delegating to
`SharedIndexReplayRuntime::run_targeted`. `run_shadow` applies the same authorization before delegating to the
side-effect-free runtime.

Targeted host dispatch creates no replay job/checkpoint, lease, heartbeat, worker, cancellation state,
graceful-stop path, scheduler registration or automatic retry/requeue owner. It preserves source event UUIDs as
the existing `index_inbox` delivery identity and relies on ordinary duplicate/stale semantics for exact retry.

The GraphQL layer now exposes four source-complete dedicated command surfaces:

- schema-wide or exact-locale durable `runIndexReplay`;
- bounded exact-key `runIndexReplayTargeted`;
- schema-wide or exact-locale no-write `runIndexReplayShadow`;
- durable `cancelIndexReplay`.

There is no caller-controlled generic replay mode. Authorization occurs before custom parsing untrusted Full
schema/locale, Targeted schema/entity/locale, Shadow schema/locale/continuation or cancel job input.

Caller durable Full input contains only module/entity/schema key plus one optional bounded locale. Tenant, actor,
worker ID, source name, partition, scheduler handle, replay resource budget, stop handle and shutdown flag remain
server-owned or unavailable. Locale is canonicalized through `LocaleKey` after authorization. Omission remains
exactly schema-wide. Each Full run creates a server-owned worker identity and applies fixed transport caps: 100
mutations per page, at most 8 pages, heartbeat every page and a 60-second lease.

Caller Targeted input contains one module/entity/schema routing identity plus `targets`. Each target contains only
an entity UUID and optional locale. Target count is bounded to 1..=256 before per-key parsing. Each locale is
canonicalized through `LocaleKey` and the transport constructs only `IndexSourceLoadRequest` using the authenticated
tenant and one parsed schema. Canonical duplicate keys fail closed, including locale aliases such as `EN-us` and
`en-US`. GraphQL delegates only to `IndexReplayOperatorRuntime::run_targeted` and does not expose source name,
worker/page budget, job/checkpoint, lease, cancellation, retry/requeue, scheduler or partition controls.

The Targeted payload contains only requested, returned mutation, missing, applied, duplicate and stale-ignored
counts. `IndexReplayTargetedOutcome::source_name` remains internal. Missing source keys remain counts only; the
transport never synthesizes deletes. Unknown/unregistered schemas and active-schema invalid targets are bad user
input, while source/batch/persistence failures remain generic internal command errors.

Caller Shadow input contains only module/entity/schema key plus one optional bounded locale and one optional
bounded sealed continuation. It has no source name, raw source cursor, worker, page/max-page budget, job,
checkpoint, lease, cancellation, retry or scheduler fields. Locale is canonicalized through the same `LocaleKey`
contract only after request-bound authorization. The server uses the fixed 100-page-size / 8-page invocation
budget, repeats exact tenant authorization, and derives one exact frozen continuation/dry-run scope:

- schema-wide -> `IndexSourceContinuationScope::from_registry` + `IndexReplayDryRunRequest::new`;
- exact locale -> `IndexSourceContinuationScope::for_locale` + `IndexReplayDryRunRequest::for_locale`.

Incoming continuation is opened under that scope before a raw source cursor is reconstructed. The no-write runtime
rejects exact-locale execution for `LocaleMode::None`, constructs every actual scan through matching
`IndexSourceScanRequest::new` or `for_locale`, calls the existing source registry, and seals any outgoing cursor
under the same scope before GraphQL serialization.

The source continuation contract includes canonical locale identity in encrypted claims. Schema-wide scope is
`locale = None`; exact-locale scope is `Some(LocaleKey)`. Opening requires exact locale equality in addition to
tenant/schema/source binding, so schema-wide and exact-locale tokens cannot cross scopes and different locales
cannot exchange tokens.

The continuation format is one current unversioned repository-owned envelope. The previous pre-release shape was
replaced in place; there is no version byte, `contract_version`, `V1`/`V2` claim family, or fallback decoder.

Retained durable command source evidence includes schema-wide command behavior, locale command canonicalization
and a dedicated locale yield/isolation/fresh-runtime resume packet. Shadow source evidence retains
authorization-first schema/locale/continuation preparation, exact schema-wide/locale dry-run scope selection,
`LocaleMode` fail-closed admission, sealed non-durable resume routing and locale-safe continuation identity.
Targeted source evidence retains authorization-first exact-target preparation, canonical per-key locale identity,
1..=256 bounds, duplicate canonical-key rejection and guarded host delegation. Execution/admission remains
maintainer-owned.

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

## M6 multi-host/restart retained source state

The replay ownership/restart chain includes one deterministic concurrent reclaim packet in addition to the
existing sequential and fresh-runtime restart evidence.

`source_replay_multihost_restart_tests.rs` retains two distinct `PostgresIndexReplayRunner` instances over one
durable database and one stable source. Host A acquires attempt 1 and blocks inside source scan; the fixture moves
only that persisted lease expiry into the past; host B then reclaims the same job through the ordinary runner as
attempt 2 and completes it before host A is released.

When host A resumes, the stable delivery remains duplicate-safe but its checkpoint path is fenced by the replaced
owner/attempt identity and returns `IndexReplayRunError::LeaseLost`. The packet requires final durable state to
remain exactly one succeeded attempt-2 job, one complete checkpoint, one applied inbox delivery and one materialized
entity. Host A cannot overwrite success with failure/pending/cancelled or advance a stale checkpoint.

The packet uses `Notify` rather than sleeps/polling and does not directly create/claim/finish jobs or write
checkpoints. The only clock seam is deterministic expiry of the already-running attempt-1 lease. See
`m6-replay-multihost-reclaim-evidence.md`.

Together with bounded resume, graceful duplicate-safe restart and fresh GraphQL locale runtime reconstruction,
this closes the currently tracked source-only multi-host/restart boundary. Execution/admission and any required
PostgreSQL/process orchestration evidence remain maintainer-owned.

## M6 explicit replay mode contract

`IndexReplayMode` separates `Full`, `Targeted` and `Shadow` rebuild intent from locale and future partition scope.
`IndexReplayModeSelection` maps those modes to non-aliasing execution surfaces:

- `Full` -> `DurableScan`; this is the only mode admitted to the existing `PostgresIndexReplayRunner` and retains
  the current fenced replay job/checkpoint, cancellation, locale and lease semantics;
- `Targeted` -> `TargetedLoad`; construction owns the canonical `IndexSourceLoadRequest`, preserving the existing
  bounded exact-key count, tenant/schema scope and uniqueness checks;
- `Shadow` -> `SideEffectFreeScan`; this matches the existing `SharedIndexReplayDryRunRuntime` no-write boundary.

`IndexReplayTargetedExecutor` makes the Targeted application surface executable without creating a second durable
replay owner. It requires `IndexReplayModeSelection::Targeted`, validates active-schema exact-key locale/entity
shape before source resolution, performs one bounded canonical `load`, preflights the full returned batch for
non-nil/unique event IDs and complete schema validation, then applies source-owned stable event identities through
`IndexReplayMutationSink`. Missing keys are counted and never reinterpreted as synthetic deletes. Partial mutation
failure is retried through ordinary stable inbox identity rather than a Targeted checkpoint.

`materialize_postgres_index_replay_runtime` assembles that executor with `PostgresMutationStore` beside the Full
runner inside `SharedIndexReplayRuntime`. `IndexReplayOperatorRuntime::run_targeted` applies the same exact
request-bound `modules:manage` authorization used by Full before Targeted source/mutation execution. This adds no
Targeted job/checkpoint/lease/cancel/scheduler/retry owner.

`runIndexReplayTargeted` is now the dedicated authorization-first Targeted transport. It keeps tenant/actor
server-owned, accepts one schema plus exact entity/locale targets, builds only the canonical
`IndexSourceLoadRequest`, and delegates only to the guarded Targeted host method. Its payload does not expose
source routing or any durable/lifecycle controls.

The server guards `Shadow` through `IndexReplayOperatorRuntime::run_shadow`, using the same exact request-bound
`modules:manage` authorization check. Schema-wide/exact-locale GraphQL Shadow transport sits on one sealed
continuation adapter and does not reinterpret the durable `runIndexReplay` command or add a mode column to jobs or
checkpoints.

The explicit mode identity, bounded Targeted application executor, Targeted PostgreSQL/runtime composition,
request-bound Targeted/Shadow host dispatch, dedicated Targeted GraphQL transport, schema-wide/exact-locale Shadow
GraphQL transport and locale-safe Shadow continuation identity are source-complete. Maintainer execution and
admission are not claimed.

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
- [x] Retain deterministic two-host lease-expiry/reclaim/stale-owner fencing evidence through distinct replay runners.
- [x] Define explicit Full/Targeted/Shadow replay mode identity and fail-closed execution surfaces.
- [x] Guard the existing side-effect-free Shadow replay runtime behind the request-bound `modules:manage` operator boundary.
- [x] Add authorization-first schema-wide GraphQL transport for guarded Shadow replay with sealed caller-carried continuation.
- [x] Make Shadow continuation identity locale-safe before exposing exact-locale Shadow GraphQL transport.
- [x] Add exact-locale Shadow dry-run/runtime/GraphQL execution using the canonical locale-safe continuation scope.
- [x] Define a bounded Targeted mutation-application contract over `IndexSource::load` without aliasing durable scan ownership.
- [x] Materialize the bounded Targeted replay executor with `PostgresMutationStore` and guard host dispatch behind request-bound `modules:manage`.
- [x] Add a dedicated authorization-first Targeted GraphQL transport over `IndexReplayOperatorRuntime::run_targeted`.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Execute/admit replay GraphQL transport behavior and cancellation evidence, including Targeted exact-key behavior.
- [ ] Execute/admit schema-wide/exact-locale Shadow GraphQL transport and continuation-key deployment evidence.
- [ ] Execute/admit retained graceful interruption/restart and GraphQL shutdown evidence.
- [ ] Execute/admit retained dependency pending-future timeout evidence.
- [ ] Execute/admit retained page lease-heartbeat evidence.
- [ ] Execute/admit retained locale replay/restart command evidence, including schema/locale isolation.
- [ ] Execute/admit retained multi-host reclaim evidence.
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

The locale request/source/job/checkpoint/runner/GraphQL identity chain, dependency timeout set,
page-duration/lease-heartbeat policy, deterministic multi-host/restart evidence, explicit Full/Targeted/Shadow
mode identity, bounded Targeted application executor, Targeted PostgreSQL/runtime composition, guarded
Targeted/Shadow host dispatch, dedicated Targeted GraphQL transport, schema-wide/exact-locale Shadow GraphQL
transport and locale-safe source continuation identity are source-complete. Their retained packets still require
maintainer execution/admission.

Partition replay remains blocked: no real partition-capable source contract can yet filter a partition before
pagination, so do not merely populate `partition_key`.

There is no remaining independent source-only M6 replay expansion justified by the current contract. The next M6
steps are maintainer execution/admission of the concrete repair packet and replay transports/evidence. Further
source changes should follow a concrete defect discovered by executed evidence. Partition replay remains blocked
until a real partition-capable source can filter before pagination.

M5 typed Product event work remains gated by canonical event-contract digest admission. M7 serving cutover remains
gated by retained evidence execution/admission. Do not bypass those owner gates by adding parallel compatibility,
mode, transport or traffic-switch surfaces.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.