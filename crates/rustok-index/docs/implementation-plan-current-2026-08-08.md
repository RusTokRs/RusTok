# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Storefront budgeted timeout evidence merge
`e0bbcc885d7670990fdf0c21d5f2ef01f5015a99` and continued on
`agent/product-key4-promotion-contract-20260808`.

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

Persisted tenant authority remains a separate execution boundary. For a tenant with a lower Product key still
active, the required sequence is:

1. ordinary-register the exact current Product key `4` immutable contract;
2. rebuild key `4` while lower persisted Product keys remain historical/staging state;
3. execute/admit exact key-4 readiness, freshness, parity, inbox-isolation and restart evidence;
4. call `PostgresSchemaRegistrationStore::register_current` with that already-staged key `4` contract;
5. require all lower active Product schemas for that tenant to retire atomically;
6. require lower-key readiness/query execution to fail closed as inactive;
7. only then admit an authoritative Product Index consumer.

No runtime dual-read compatibility branch is allowed. Historical lower-key rows are retained storage history,
not a Product v3/v4 compatibility surface.

## Remaining Storefront parity/evidence blockers

- execute/admit the deterministic budgeted timeout packet and retain acceptable runtime latency/cancellation
  evidence for the selected deployment profile;
- execute/review current-key Storefront core/EAV/collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the deployment default-vs-`C` packet agrees;
- complete maintainer-executed stale locale/readiness/admission/restart evidence;
- execute/admit Product key-4 staged promotion evidence and perform tenant stage/rebuild/`register_current`;
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
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Complete remaining multi-host/restart/shutdown/command-transport evidence.

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
- [ ] Execute/admit the retained budgeted timeout evidence.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Execute/admit Product key-4 promotion evidence and stage/rebuild/promote a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code boundary

The Storefront request-shape, projection/hydration, timeout and current-key promotion contracts are source-complete.
Do not add a traffic-switch adapter from source inspection alone. The next useful source-only slice is retained
Product key-4 promotion/restart PostgreSQL evidence if no such packet exists; otherwise maintainer execution is
the blocking action. Any new packet must use production schema registration/readiness/query/replay paths and
must not manufacture a second Product compatibility implementation.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
