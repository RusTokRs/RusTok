# M7 Product Storefront Index parity gate

Status: `deep_page_policy_source_complete_projection_placeholder_pending`.

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

The generic entity-admission catalog is schema-scoped rather than an arbitrary request-scoped Product
predicate channel, so the shadow path cannot recover the owner metadata distinction at query time without a
new generic contract or a persisted Product field/schema replacement.

For the current key-4 contract the policy is explicit:

- absent/blank slug and absent channel UUID => `OwnerNativeChannelLess`;
- trusted non-empty slug and non-nil UUID => `ShadowEligible`;
- partial identity, UUID-without-slug, slug-without-UUID, or nil UUID =>
  `PublicChannelIdentityUnavailable`.

The shadow executor still produces the authoritative owner result first. A channel-less request then retains
that result and records `ChannelLessOwnerNative` in `projected`; it does not fabricate an Index page. This is a
serving-policy decision, not a claim of channel-less Index parity.

No sentinel UUID, `attribute_terms` visibility encoding, key-5 Product schema, or membership-equality inference
is introduced. Any future serving composition must preserve owner-native handling for this shape until a later
exact representation is implemented with the normal schema replacement and freshness evidence gates.

## Deep-page serving policy — source complete

The Product owner validates `page >= 1` and `1 <= per_page <= 48`, but it does not impose the generic Index
offset ceiling. The generic Product Storefront shadow builder remains bounded at offset `10_000`.

`classify_product_storefront_index_page_scope` now preserves this owner/Index difference after the authoritative
owner list succeeds and before projected schema/EAV work:

- checked offset `<= 10_000` => `ShadowEligible { offset }`;
- checked offset `> 10_000` => `OwnerNativeDeepPage { offset }` and projected
  `DeepPageOwnerNative { offset }`;
- invalid page/per-page or arithmetic overflow => existing invalid-pagination query-build error.

The policy does not clamp page/offset or rewrite the request to cursor pagination. The pure shadow builder
retains `OffsetTooDeep` as a second fail-closed boundary for direct callers. Deep pages therefore remain
owner-native under the current contracts rather than being made artificially representable.

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
construction and `execute_localized_query`. Projected failures cannot replace the successful owner result.

## Core/EAV and retained Product PostgreSQL packets

The core Storefront packet retains localized requested/fallback/neither projection, all-locale title matching,
wildcards, identity de-duplication, count, Asc/Desc tie ordering, pagination and trusted public-channel
membership. It also records the null-vs-public-placeholder projection gap.

The EAV packet separately retains scalar/localized terms, Select/Multiselect option code/direct UUID and
missing/nil option `Never` behavior.

Historical Product locale-absence, materialized-freshness, channel convergence/identity-transition and
linked-target recreate/availability/replay packets are source-aligned on Product key `4`. ProductVariant stays
key `2`, SalesChannel key `1`. Execution/review remains maintainer-owned.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of Storefront core/EAV/collation and actualized retained Product packets.
2. Collation admission per deployment: any owner/default-vs-`C` mismatch keeps eligible Index cutover closed.
3. Final Storefront projection must map no-localized-row null title/handle to owner placeholders only after
   entity identity/order/count/page boundary are fixed.
4. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
5. Shadow execution has no serving latency/deadline policy and remains non-serving.
6. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.
7. Any future serving router must preserve typed channel-less and deep-page owner-native branches.

## Next source slice

Add a post-page Product Storefront projection adapter for the Index result. A localized null `title` must map to
owner public placeholder `"Untitled product"` and a localized null `handle` to `""`, but only after the Index
query has already fixed identity, ordering, exact count and page boundary. Do not feed placeholders back into
filtering, sorting, identity folding or count. Taxonomy tag-name hydration remains a later slice.

## Source guards

- `verify-index-product-storefront-channel-scope-policy.mjs` locks owner channel-less semantics, unrestricted
  relation materialization and the typed current-key owner-native decision without sentinels;
- `verify-index-product-storefront-deep-page-policy.mjs` locks owner-valid shallow/deep classification and
  forbids page/offset clamp or cursor rewriting;
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
