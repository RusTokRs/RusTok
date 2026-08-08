# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Storefront serving-budget policy merge
`0c40387d9bb2257f8345448792f9c9ddd6b38480` and continued on
`agent/product-storefront-budgeted-execution-20260808`.

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
- **non-serving post-owner budgeted execution** that accepts only `Eligible`, narrows phase port deadlines and
  enforces outer Tokio timeouts for raw Index/EAV execution and Product tag hydration;
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

## Serving budget and timeout enforcement

`PortContext.deadline_ms` is the original duration budget. A future host/router must measure `remaining_ms`
after owner success. `ProductStorefrontIndexServingBudget` carries host-selected positive Index/tag phase
budgets plus safety margin; classification stays owner-native when timing/capability state is missing,
inconsistent or insufficient.

`ProductStorefrontIndexBudgetedProjectionExecutor` is post-owner only: it receives the already-successful owner
page, rejects non-`Eligible` decisions before work starts, narrows the projected phase context and applies an
outer timeout, then separately narrows and times Product tag hydration. Public placeholder mapping occurs only
after successful raw projection. Timeout/error results remain separate and never replace owner success.

The ordinary owner-first shadow executor remains the unbudgeted evidence path. Mounted Storefront still uses
only Product owner reads and does not call either serving-budget classification or budgeted execution.

## Remaining Storefront parity/evidence blockers

- retain and execute deterministic timeout/latency evidence for budgeted post-owner execution;
- execute/review current-key Storefront core/EAV/collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the deployment default-vs-`C` packet agrees;
- complete maintainer-executed stale locale/readiness/admission/restart evidence;
- execute/admit current Product replacement evidence and stage/rebuild/promote key `4`;
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
- [ ] Retain deterministic runtime timeout/latency evidence for the budgeted adapter.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes.

## Next source-code step

Retain deterministic non-serving timeout-behavior evidence for `ProductStorefrontIndexBudgetedProjectionExecutor`.
The source packet should prove: a non-eligible decision starts no projected work; Index timeout preserves the
owner page; raw projection failure skips public/tag enrichment; tag timeout preserves raw/public pages; phase
`deadline_ms` values reach Product capabilities; and an eligible fast path preserves raw identity/count/page
semantics. Do not run the packet and do not mount Storefront traffic.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
