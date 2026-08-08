# M7 Product Storefront Index parity gate

Status: `core_eav_and_retained_key4_packets_source_complete_execution_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, pure localized shadow builder and non-serving owner-first shadow executor are
source-complete. Two current-key PostgreSQL owner-vs-shadow Storefront packets are retained in source: the
localized core packet and a separate EAV packet. Historical retained Product PostgreSQL packets are also
source-actualized to Product routing key `4`. None of those packets has been executed or admitted by the
implementation agent.

## Product-owned EAV resolution and shadow execution

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` remains the only Product metadata
boundary used by shadow execution. It preserves mounted owner semantics for typed values, localized
requested/fallback text and Select/Multiselect UUID/code resolution, returning neutral
`ProductAttributeTermExpr` values owned by Product.

Distribution translates those owner-owned expressions into `attribute_terms` predicates. `Never` maps to a
bind-free false predicate using the current Product schema's required/non-null `id` invariant.

`ProductStorefrontIndexShadowExecutor` executes the authoritative Product owner list first. Only after owner
success does it resolve EAV metadata, build `LocalizedEntityQuery` and call `execute_localized_query`.
Projected failures cannot replace the successful owner result. It remains non-serving.

## Core PostgreSQL packet — source complete, execution pending

`storefront_shadow_postgres_tests.rs` uses current Product routing key `4`, real Product/Index migrations,
channel relation freshness, Product source/mutation materialization, persisted Index schema registration,
real Product owner reads and the non-serving shadow executor.

It retains requested/fallback/neither localized projection, third-locale title matching, `%`/`_`/escaped `_`
LIKE behavior, locale identity de-duplication, exact count, Asc/Desc equal-timestamp Product-ID ties, offset
page boundaries and trusted public-channel membership.

It also records the remaining public projection adapter gap: owner uses `"Untitled product"` and empty handle
when neither requested nor fallback translation exists, while generic localized Index returns null. Final
serving projection must map that null state only after Product page identity/order/count are fixed.

## EAV PostgreSQL packet — source complete, execution pending

`storefront_shadow_eav_postgres_tests.rs` is a separate crate-internal opt-in packet on routing key `4`.
Its clean fixture seeds real active/filterable Product attributes, active options and value rows before the
first Index materialization, then uses the real Product resolver and shadow executor side-by-side.

The packet retains source scenarios for:

- nonlocalized integer term (`weight=7`);
- requested localized text (`label=Punainen`);
- requested-locale-present fallback suppression plus requested-missing fallback (`label=Red` admits B, not A);
- Select exact option code (`color=red` / `color=blue`);
- direct Select option UUID;
- Multiselect exact option code (`features=wifi`);
- missing option code -> Product `Never` -> empty owner/Index result;
- nil option UUID -> Product `Never` -> empty owner/Index result;
- owner/Index ordered identity list, exact count and page-boundary agreement for every scenario.

The EAV fixture deliberately does not call `save_product_attribute_values`: Product EAV owner-clock command
semantics already have a separate retained gate. This packet isolates query resolution/materialization parity
from command-publication evidence.

## Historical retained Product packets — key 4 source actualized

The following retained PostgreSQL fixtures now use the current Product routing key `4` while preserving their
existing scenario semantics:

- `product_locale_absence_postgres.rs`;
- `product_materialized_query_freshness_postgres.rs`;
- `product_channel_convergence_postgres.rs`;
- `product_channel_identity_transitions_postgres.rs`;
- `product_linked_target_recreate_postgres.rs`;
- `product_linked_target_availability_equivalence_postgres.rs`;
- `product_linked_target_replay_redelivery_postgres.rs`.

They continue to build the real distribution schema/source registries, so current Product materialization is
against the 15-field key-4 contract rather than a test-only compatibility schema. ProductVariant stays on key
`2`, SalesChannel stays on key `1`, and no Product key-3 runtime alias exists.

`verify-index-product-postgres-key4-fixtures.mjs` locks that boundary. Actualization is source maintenance,
not proof that the packets pass; execution/review remains a maintainer gate.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of the Storefront core/EAV packets and actualized retained Product packets;
   source presence is not evidence admission.
2. Search-length mismatch: owner search is not explicitly bounded while Index `TextLike` is capped at 1024
   UTF-8 bytes.
3. Owner/default PostgreSQL collation vs Index deterministic `COLLATE "C"`.
4. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot represent
   that distinction exactly.
5. Owner page depth exceeds the Index 10,000 offset bound.
6. Final Storefront projection must map no-localized-row null title/handle to owner placeholders.
7. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
8. Shadow execution has no serving latency/deadline policy and remains non-serving.
9. Stale locale/readiness/admission/restart cases still need maintainer-executed retained Storefront evidence.

## Next source slice

Establish an authoritative Product Storefront title-search length contract compatible with bounded Index
`TextLike`. Do not silently truncate owner-valid input. After that contract is source-aligned, retain explicit
PostgreSQL evidence for owner/default title `LIKE` collation versus deterministic Index `COLLATE "C"`.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product-owned EAV resolution;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term -> localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first non-serving execution;
- `verify-index-product-storefront-equivalence-postgres-packet.mjs` locks the core current-key packet;
- `verify-index-product-storefront-eav-equivalence-postgres-packet.mjs` locks the EAV current-key packet;
- `verify-index-product-postgres-key4-fixtures.mjs` locks all actualized retained Product packets on key `4`;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
