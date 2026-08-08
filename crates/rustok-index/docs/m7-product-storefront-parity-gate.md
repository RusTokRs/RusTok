# M7 Product Storefront Index parity gate

Status: `public_projection_source_complete_taxonomy_hydration_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, localized shadow builder and owner-first non-serving shadow executor are
source-complete. Current-key Storefront core/EAV/collation PostgreSQL packets and the historical retained
Product packet set are retained in source on Product routing key `4`. They have **not** been executed or
admitted by the implementation agent.

## Channel-less serving policy — source complete for current key 4

Owner channel-less semantics are stricter than resolved membership. With no public channel slug, Product owner
admits only Products whose `metadata.channel_visibility.allowed_channel_slugs` is absent or empty.

The Product relation resolver represents unrestricted metadata by resolving **all current Channel UUIDs** into
`sales_channel_ids`. A restricted Product whose allowed slugs currently resolve to every Channel therefore has
the same membership vector as an unrestricted Product. Current key `4` cannot distinguish those states.

For the current key-4 contract:

- absent/blank slug and absent channel UUID => `OwnerNativeChannelLess`;
- trusted non-empty slug and non-nil UUID => `ShadowEligible`;
- malformed/partial identity => `PublicChannelIdentityUnavailable`.

The shadow executor produces the authoritative owner result first. A channel-less request records
`ChannelLessOwnerNative` on the projected side and never fabricates an Index page. No sentinel UUID, unrelated
`attribute_terms` encoding or key-5 approximation is used.

## Deep-page serving policy — source complete

The Product owner validates `page >= 1` and `1 <= per_page <= 48`, but it does not impose the generic Index
offset ceiling. The generic Product Storefront path remains bounded at offset `10_000`.

`classify_product_storefront_index_page_scope` preserves this owner/Index difference after authoritative owner
success and before projected schema/EAV work:

- checked offset `<= 10_000` => `ShadowEligible { offset }`;
- checked offset `> 10_000` => `OwnerNativeDeepPage { offset }` and `DeepPageOwnerNative { offset }`;
- invalid pagination/overflow => existing invalid-pagination failure.

The policy does not clamp page/offset or rewrite to cursor pagination. The pure shadow builder independently
retains `OffsetTooDeep` for direct callers.

## Product public placeholder projection — source complete

The raw localized Index page deliberately remains Product-neutral. If no requested or fallback translation row
exists, raw root `title` and `handle` remain `IndexValue::Null`. The retained core PostgreSQL packet continues
to preserve that raw evidence beside the authoritative owner result (`"Untitled product"` / empty handle).

`storefront_projection.rs` adds a Product distribution-owned **post-page** transform:

- root `title: Null` => `String("Untitled product")`;
- root `handle: Null` => `String("")`;
- existing strings are preserved;
- missing, duplicate, or wrong-typed root title/handle fields fail closed;
- item identities/order, `exact_count`, `has_more`, `next_cursor`, unrelated fields and `tag_ids` are preserved.

`ProductStorefrontIndexShadowExecution` now retains two explicit layers:

- `projected`: the raw generic `IndexQueryPage`, still used for identity/order/count/page comparison;
- `public_projected`: an optional result derived only from a clone of a successful raw page by
  `project_product_storefront_index_page`.

The public transform is not called inside `execute_projected`, the shadow query builder contains no owner
placeholder strings, and generic `rustok-index` remains unaware of Product public placeholder semantics.
Therefore placeholders cannot influence title search, filters, ordering, localized identity folding, exact
count, offset/cursor construction or raw equivalence evidence.

This closes the no-requested/fallback public title/handle **source adapter** gap. It does not close Taxonomy tag
parity: current Index projection still carries `tag_ids`, while Product owner list returns localized tag names.

## Product-owned Storefront search bound — source complete

Product owns `MAX_STOREFRONT_PRODUCT_SEARCH_BYTES = 1022`. The owner wraps normalized search as
`%{search}%`, making the maximum pattern exactly representable by generic Index `TextLike`'s 1024-byte bound.
The constructor and owner SQL path both enforce the Product bound; over-bound input is rejected, never
truncated. The shadow builder imports the same Product constant.

## Title-search collation packet — source complete, execution pending

`product_storefront_search_collation_postgres.rs` compares real owner/default `title LIKE pattern` with the
Index-equivalent `(title COLLATE "C") LIKE pattern ESCAPE E'\\'` on the real Product translation column. It
covers ASCII case, NFC/NFD Unicode, `%`, `_`, escaped wildcards and sharp-s/ASCII-SS distinctions and reports
`lc_collate` on mismatch. It does not manufacture a favorable collation. Deployment admission remains a
maintainer-run gate.

## Product-owned EAV resolution and shadow execution

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` remains the Product metadata boundary
used by shadow execution. Distribution translates Product-owned neutral term expressions into
`attribute_terms`; missing option identities remain `Never` and map to a bind-free false predicate.

`ProductStorefrontIndexShadowExecutor` executes the authoritative Product owner list first. Only eligible
channel-scoped, shallow projected work proceeds through Product metadata resolution, localized query
construction and `execute_localized_query`. Projected or public-projection failures cannot replace the
successful authoritative owner result.

## Core/EAV and retained Product PostgreSQL packets

The core Storefront packet retains localized requested/fallback/neither projection, all-locale title matching,
wildcards, identity de-duplication, count, Asc/Desc tie ordering, pagination and trusted public-channel
membership. Its raw null-vs-owner-placeholder assertions intentionally remain unchanged.

The EAV packet separately retains scalar/localized terms, Select/Multiselect option code/direct UUID and
missing/nil option `Never` behavior.

Historical Product locale-absence, materialized-freshness, channel convergence/identity-transition and
linked-target recreate/availability/replay packets are source-aligned on Product key `4`. ProductVariant stays
key `2`, SalesChannel key `1`. Execution/review remains maintainer-owned.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of Storefront core/EAV/collation and actualized retained Product packets.
2. Collation admission per deployment: any owner/default-vs-`C` mismatch keeps eligible Index cutover closed.
3. Taxonomy tag names must be batch-hydrated requested-locale -> fallback-locale only after Product page
   identity/order/count is fixed; current post-page adapter intentionally leaves `tag_ids` unchanged.
4. Shadow execution has no serving latency/deadline policy and remains non-serving.
5. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.
6. Any future serving router must preserve typed channel-less and deep-page owner-native branches.

## Next source slice

Add a Product/Taxonomy owner capability for bounded batched hydration of the already-selected Product page's
`tag_ids`. Resolve requested-locale then fallback-locale display names only after Index identity/order/count is
fixed. Hydration failure must be retained separately and must not replace the raw Product page. Do not copy tag
names into Product Index schema merely to avoid post-page owner hydration.

## Source guards

- `verify-index-product-storefront-channel-scope-policy.mjs` locks current-key channel-less owner-native policy;
- `verify-index-product-storefront-deep-page-policy.mjs` locks owner-valid shallow/deep classification;
- `verify-index-product-storefront-public-projection.mjs` locks raw-vs-public page separation, Product public
  placeholder values and preservation of page metadata/`tag_ids`;
- `verify-product-storefront-search-bound.mjs` locks the Product-owned search-length contract;
- `verify-index-product-storefront-collation-postgres-packet.mjs` locks retained collation evidence;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term -> localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first execution plus typed request scopes;
- `verify-index-product-storefront-equivalence-postgres-packet.mjs` and the EAV counterpart lock current-key
  parity packets;
- `verify-index-product-postgres-key4-fixtures.mjs` locks retained Product packets on key `4`;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
