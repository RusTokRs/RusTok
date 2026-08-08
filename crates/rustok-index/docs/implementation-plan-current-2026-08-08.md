# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked after Product Storefront collation packet merge
`26d6e340ee6d1be462f29da6b95b6539458b843f` and continued on
`agent/product-storefront-channel-scope-policy-20260808`.

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
- localized entity identity fold, cursor v3 and requested -> fallback projection;
- localized PostgreSQL compiler/decoder/runtime with readiness/admission and one repeatable-read page/count snapshot;
- generic String `TextLike` plus the Product-owned 1022-byte effective Storefront search bound;
- retained owner/default-vs-Index-`C` PostgreSQL title-search collation packet source;
- localized Product-ID tie-break direction matching owner Asc/Desc ordering;
- Product-owned Storefront attribute-filter -> neutral canonical term resolution;
- pure Storefront shadow builder and owner-first non-serving shadow executor;
- explicit current-key channel-scope policy: trusted non-empty slug + non-nil UUID is shadow-eligible, while
  channel-less owner requests remain owner-native and partial/invalid channel identity fails closed;
- current-key core/EAV Storefront PostgreSQL packet source;
- historical retained Product PostgreSQL fixtures actualized to current Product routing key `4`.

## Channel-less visibility decision for current Product key 4

Owner channel-less semantics are **metadata-unrestricted only**: the Product owner predicate admits rows whose
`metadata.channel_visibility.allowed_channel_slugs` is absent or empty.

The Product -> SalesChannel relation resolver intentionally represents unrestricted visibility as membership
in **all current Channel UUIDs**. A restricted Product whose allowed slugs currently resolve to every Channel
therefore has the same `sales_channel_ids` value as an unrestricted Product. Current Product key `4` cannot
recover the owner distinction from resolved membership.

The generic PostgreSQL entity-admission catalog is schema-scoped and does not carry an arbitrary
request-specific Product visibility predicate. Consequently the current source does not fabricate a sentinel,
encode visibility into unrelated `attribute_terms`, or introduce a key-5 schema without the replacement
protocol/evidence required for a real schema change.

`classify_product_storefront_index_channel_scope` now makes the request policy explicit:

- absent/blank slug + absent UUID => `OwnerNativeChannelLess`;
- non-empty slug + non-nil UUID => `ShadowEligible`;
- partial, contradictory or nil channel identity => `PublicChannelIdentityUnavailable`.

The shadow executor still runs the authoritative Product owner list first. For channel-less requests it retains
that result and records typed projected reason `ChannelLessOwnerNative`; it never approximates an Index result.
This closes the current-key **policy decision**, not channel-less Index parity. Any future serving router must
continue to route this shape owner-native unless a later schema/query capability represents the distinction
exactly with freshness evidence.

## Remaining Storefront parity/evidence blockers

- execute/review current-key Storefront, collation and actualized retained Product PostgreSQL packets;
- admit collation parity only where the retained deployment matrix agrees;
- owner page depth exceeds the Index bounded offset depth;
- map localized null title/handle to owner public placeholders in final projection;
- hydrate localized Taxonomy tag names only after Product page identity/count is fixed;
- define serving latency/deadline policy before any shadow-to-serving transition;
- preserve channel-less owner-native routing in any future serving composition;
- complete maintainer-executed stale locale/readiness/admission/restart evidence.

## Retained Product evidence state

The retained Product PostgreSQL fixture set is source-aligned on routing key `4`: locale absence, materialized
query freshness, Product/Channel convergence, Channel identity transitions, linked-target delete/recreate,
linked-target availability equivalence and linked-target replay/redelivery. ProductVariant remains key `2`
and SalesChannel remains key `1`.

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
- [x] Localized identity/fallback architecture and query/cursor contract.
- [x] Localized PostgreSQL compiler/decoder/runtime.
- [x] Generic scalar String `TextLike` and Product-owned compatible search bound.
- [x] Retain owner/default-vs-Index-`C` PostgreSQL title-search collation packet source.
- [x] Explicit localized entity-ID tie-break direction matching owner Asc/Desc ordering.
- [x] Product Storefront shadow query builder and Product-owned EAV resolution.
- [x] Compose non-serving Product-owner + Index shadow executor.
- [x] Retain current-key core/EAV owner-vs-shadow PostgreSQL packet source.
- [x] Actualize historical retained Product PostgreSQL packets to routing key `4`.
- [x] Keep channel-less Storefront owner-native on current key `4`; distinguish malformed channel identity.
- [ ] Execute/review retained Product/Storefront/collation PostgreSQL packets.
- [ ] Admit owner/default vs Index `COLLATE "C"` title-search parity for a deployment.
- [ ] Decide authoritative deep-page policy.
- [ ] Map no-localized-row nulls to owner public title/handle placeholders in final projection.
- [ ] Batch-hydrate Taxonomy tag names after the Product page is fixed.
- [ ] Extend folded linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL evidence.
- [ ] Stage/rebuild/promote Product key `4` for a tenant.
- [ ] Move eligible Storefront traffic only after every parity/readiness/freshness/restart/latency gate passes;
      channel-less remains owner-native unless a later exact representation is admitted.

## Next source-code step

Resolve deep-page request policy without silently narrowing owner semantics. The owner accepts any checked
`(page - 1) * per_page` offset representable by its query path, while generic Index offset pagination is
bounded to `10_000`. Classify an otherwise valid owner request whose computed offset exceeds that bound as a
typed **owner-native deep-page** shape, distinct from invalid pagination, and keep shallow pages shadow-eligible.
Do not clamp page/offset or silently switch pagination semantics.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
