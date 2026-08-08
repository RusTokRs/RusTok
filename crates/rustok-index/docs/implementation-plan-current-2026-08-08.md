# Current `rustok-index` implementation plan — 2026-08-08

Status overlay rechecked from `main@1ddbc6b5457d2f9263b842c6e5b1942598a26ff7` (#3215) and continued on
`agent/index-text-like-20260808`.

`implementation-plan.md` remains historical architecture context. This file is the current execution
cursor.

## Recheck result

The Product/Index sequence relevant to this cursor includes:

- #3190 retained linked-target replay/redelivery source evidence;
- #3192 added the fail-closed Product Storefront parity gate;
- #3193 added staged single-current schema supersession;
- #3194 added schema-scoped source delivery IDs;
- #3197 advanced the canonical Product owner clock for EAV writes;
- #3198 defined canonical typed Product `attribute_terms`;
- #3199 replaced Product runtime code with one current 15-field Product contract on internal routing key
  `4`, with lower keys historical only;
- #3200 proved owner title search is all-translations while result projection is requested -> fallback;
- #3204 selected the generic localized-entity identity-fold architecture;
- #3208 added explicit localized query validation and dedicated cursor identity;
- #3210 added the root-only PostgreSQL localized page/count compiler and decoder;
- #3212 wired localized execution through the canonical PostgreSQL query runtime with persisted readiness,
  generic admission and one read-only repeatable-read snapshot.

Later #3213, #3214 and #3215 are respectively Moderation recovery evidence, Commerce Product
attribute-values owner-port cutover, and Pages contribution generation. They do not change the Product
Storefront list owner contract or the `rustok-index` localized query semantics in this slice.

This branch closes the remaining generic scalar title-pattern capability with bounded `TextLike` while
keeping Storefront traffic owner-native and all execution/equivalence gates unchanged.

## Old execution branch

`agent/index-linked-target-replay-redelivery-evidence-20260807` is not a valid continuation base; its source
was already squash-merged through #3190. Do not reuse it. Branch deletion is repository hygiene only and
remains dependent on repository tooling that exposes ref deletion.

## Current primary owner gate

`M6 - execute and admit concrete repair PostgreSQL evidence`

The concrete repair implementation, recovery policy, PostgreSQL harness and retained-evidence admission
source remain complete. Maintainer execution/admission is still required and is not claimed by source
inspection.

## Current Product/Storefront source state

The canonical Product graph keeps one current Product runtime contract plus ProductVariant and
SalesChannel target contracts.

Current Product source/runtime facts:

- one current Product schema on internal routing key `4`;
- 15 fields: `id`, `status`, `title`, `handle`, `description`, `seller_id`, `vendor`, `product_type`,
  `primary_category_id`, `tag_ids`, `created_at`, `published_at`, `attribute_terms`, `variant_ids`, and
  `sales_channel_ids`;
- Product replay IDs are schema-scoped through `derive_index_schema_source_event_id`;
- lower Product routing keys are historical persisted identities, not compatibility implementations;
- EAV writes advance the same Product owner `index_revision` / graph `projection_epoch` clock;
- dynamic EAV filters use stable UUID-keyed `attribute_terms`;
- ProductVariant/SalesChannel recreate ordering remains tombstone-backed and monotonic;
- Product root freshness and ordinary linked-target availability remain fail-closed.

## Localized Product query source state

The localized fold is source-complete through runtime execution:

- `LocalizedEntityQuery` keeps requested locale in `query.scope.locale`, canonical fallback separately,
  `any_locale_filter` as an identity-level existential predicate, and explicit
  `localized_projection_fields` for requested -> fallback -> null projection;
- validation requires `LocaleMode::Required`, reuses ordinary field/operator/value rules and keeps this
  implementation root-only;
- `LocalizedCursorCodec` uses scoped wire version `3`; ordinary exact-locale cursor wire version `2`
  remains unchanged;
- page/count compilation uses `t0` admitted identity anchor, `t1` requested, `t2` fallback, `t3`
  any-locale predicate and `t4` lower-locale anti-duplicate candidate;
- physical row roles retain canonical `is_deleted = FALSE` anchors for generic owner admission;
- identity de-duplication happens before ordering/lookahead/limit/exact count;
- requested/fallback projection uses row-presence `CASE`;
- localized decoding checks ordinary/localized plan identities and emits localized cursors only;
- `IndexQueryPort::execute_localized_query` has a fail-closed default and is forwarded by
  `SharedIndexQueryRuntime`;
- `PostgresIndexQueryPort` applies availability/entity admission before storage execution, then verifies
  persisted readiness and executes page/count inside one `REPEATABLE READ, READ ONLY` transaction.

## Generic scalar `TextLike` — source complete in this slice

`FilterExpr::TextLike(FieldPath, String)` is appended after existing filter variants so previous postcard
discriminants remain stable.

Validation permits it only for a filterable scalar String field and enforces:

- at most 1024 UTF-8 bytes;
- no NUL;
- no dangling trailing backslash escape.

Semantics match PostgreSQL `LIKE` with explicit backslash escape: `%` matches zero-or-more characters,
`_` one character and `\` escapes the next wildcard/literal.

The ordinary PostgreSQL compiler binds the pattern and supports linked/many paths through the existing
correlated `compile_many_exists` machinery. The localized compiler uses the same bound pattern and SQL
operator for root `any_locale_filter`. The in-memory reference engine and PostgreSQL equivalence reference
fixture implement the same wildcard grammar.

The current Product owner title helper is still `format!("%{search}%")` + `pt.title LIKE $1` over all
translations, so the future Product adapter can express the same title predicate as folded
`TextLike(title, pattern)` without Product-specific Index SQL.

## Search parity caveats discovered during recheck

`StorefrontProductListQuery` currently normalizes optional search text but imposes no explicit search
length bound. Generic `TextLike` is intentionally bounded to 1024 UTF-8 bytes. Therefore the adapter must
not claim full owner parity until one of these is proven in reviewed source/evidence:

1. an authoritative upstream Storefront request bound <= 1024 bytes already exists; or
2. the owner/API contract is explicitly bounded with matching validation and compatibility review.

Silent truncation or rejection in an Index-only adapter is forbidden.

The owner title `LIKE` uses database-default collation while Index String scalar SQL uses deterministic
`COLLATE "C"`. Retained PostgreSQL equivalence must cover the admitted deployment/input contract before
search parity can be promoted.

## Retained M7 PostgreSQL packets

Several retained packets still encode historical Product routing key `3` / pre-replacement assumptions and
must be actualized before they count as current replacement evidence:

1. `product_materialized_query_freshness_postgres.rs`;
2. `product_channel_convergence_postgres.rs`;
3. `product_channel_identity_transitions_postgres.rs`;
4. `product_linked_target_recreate_postgres.rs`;
5. `product_linked_target_availability_equivalence_postgres.rs`;
6. `product_linked_target_replay_redelivery_postgres.rs`.

The Product locale-absence retained packet and related guards also require recheck. Do not add a runtime
alias for key `3`; evidence must follow the one current Product contract.

## M5 incremental ingestion

- [x] Source replay registry and bounded source failures.
- [x] Inbox deduplication and monotonic source versions.
- [x] Mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Product locale/ProductVariant refresh ledgers and durable relay step.
- [ ] Execute canonical event-contract digest admission on current reviewed `main` and commit the
      generated canonical digest artifact through its own reviewed PR.
- [ ] Add exactly one canonical Product Index typed event family only after the digest gate is valid.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence for the typed incremental route.

## M6 replay, reconciliation, diagnosis, and repair

- [x] Bounded scan/load and stable replay identities.
- [x] Durable jobs, leases, checkpoints, multi-page replay, cancellation and reconciliation.
- [x] Source timeout, dry-run, interruption, retry and dead-letter recovery.
- [x] Drift discovery/confirmation/finding lifecycle and targeted repair.
- [x] Concrete missing-entity/orphan-link repair and prepared-command recovery.
- [x] Real-migration PostgreSQL repair harness and retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Retain remaining multi-host/restart/graceful-shutdown/command-transport evidence.
- [ ] Add remaining locale/partition checkpoint dimensions and explicit rebuild modes.

## M7 Product/ProductVariant/SalesChannel production graph

- [x] Canonical Product, ProductVariant and SalesChannel bounded sources.
- [x] Product `variants` and `sales_channels` links and retained delete identities.
- [x] Product-to-SalesChannel relation membership ledger, resolver and freshness witness.
- [x] Canonical Product graph projection epoch and projection-aware absence.
- [x] Rejected-Product poison isolation and materialized root freshness fence.
- [x] Entity-level stale linked-target freshness fence and recreate-safe target revisions.
- [x] Query-path-scoped fail-closed linked-target availability for ordinary Product queries.
- [x] One current 15-field Product Storefront-capable source contract.
- [x] Schema-safe single-current replacement/promotion mechanism.
- [x] Canonical typed EAV term representation and Product EAV owner clock.
- [x] Recheck owner all-translations search + requested/fallback projection mismatch.
- [x] Select generic localized identity/fallback architecture without another Product routing key.
- [x] Add localized query shape/validation and dedicated cursor identity.
- [x] Compile/decode root-only localized identity-fold page/count.
- [x] Wire localized execution through canonical PostgreSQL runtime with readiness/admission/snapshot.
- [x] Add generic scalar String `TextLike` usable inside folded `any_locale_filter`.
- [ ] Implement the Product Storefront Index adapter and Taxonomy tag hydration boundary.
- [ ] Resolve owner-search input bound and collation parity for authoritative adapter search.
- [ ] Actualize retained Product PostgreSQL packets/guards to routing key `4` / 15-field source.
- [ ] Retain source-ready owner-vs-Index localized PostgreSQL equivalence packets.
- [ ] Extend folded execution to linked paths only with dedicated target-availability evidence.
- [ ] Execute/admit current replacement Product PostgreSQL packets.
- [ ] Stage/rebuild/promote the current Product schema for a tenant.
- [ ] Move Storefront traffic only after readiness/equivalence/freshness/availability/restart evidence
      passes.

## Next implementation step

Primary maintainer gate remains: **execute and admit the locked M6 repair PostgreSQL packet**.

Next source-code step: **implement the Product Storefront Index adapter plus retained localized-query
PostgreSQL equivalence packet**, while leaving Storefront traffic owner-native. The adapter must map
Active/published/category/channel/EAV/order/page/count, requested/fallback localized projection and
`TextLike` all-translations title search, then batch-hydrate Taxonomy tag names after the Product page is
fixed.

The same slice must make the search-bound/collation mismatch explicit and fail closed until an
authoritative equivalent input contract is demonstrated. Do not silently truncate owner-valid search.

In parallel, actualize retained Product PostgreSQL packets to routing key `4` / current 15-field contract.
No historical-key runtime compatibility path is allowed.

Typed Product events remain separately blocked on maintainer event-digest admission.

## Maintainer verification after this source slice

The implementation agent has not run these commands. Maintainer verification should include:

```bash
node scripts/verify/verify-index-text-like-filter.mjs
node scripts/verify/verify-index-localized-query-contract.mjs
node scripts/verify/verify-index-localized-query-postgres-fold.mjs
node scripts/verify/verify-index-localized-query-runtime.mjs
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
