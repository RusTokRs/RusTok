# M7 Product Storefront Index parity gate

Status: `tag_hydration_source_complete_serving_budget_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

Current-key Storefront core/EAV/collation PostgreSQL packets and retained Product packets remain source-only;
maintainer execution/admission is not claimed.

## Request-shape policy — source complete

- trusted non-empty public channel slug + non-nil UUID is shadow-eligible;
- channel-less requests remain typed owner-native because key `4` cannot distinguish unrestricted metadata
  from restricted membership that resolves to every current Channel;
- owner-valid offsets through `10_000` are shadow-eligible;
- deeper owner-valid pages remain typed owner-native;
- no visibility sentinel, page clamp, cursor rewrite or Product key-5 approximation is used.

## Product public placeholder projection — source complete

Raw localized Index results remain Product-neutral. No requested/fallback row means raw root `title`/`handle`
are `IndexValue::Null` and retained PostgreSQL evidence continues to observe that state.

`public_projected` is derived only from a clone of a successful raw page and maps:

- `title: Null` -> `"Untitled product"`;
- `handle: Null` -> `""`.

Identity/order/count/page/cursor and unrelated fields, including `tag_ids`, remain unchanged. Raw comparison
continues to use `projected`, not the public layer.

## Product-owned tag hydration — source complete

Current Product Index `tag_ids` are relation-backed UUIDs from `product_tags`. Product owner semantics are
broader: when no relations exist, legacy normalized `metadata.tags` remain a read fallback. Therefore a
`tag_ids -> names` adapter would lose valid owner-visible tags.

Product now publishes optional `ProductStorefrontTagReadPort` with a bounded page request:

- at most 48 unique, non-nil already-selected Product IDs;
- tenant-scoped verification of every Product identity;
- requested locale from `PortContext`, explicit fallback locale from the request;
- reuse of `CatalogService::load_product_tag_map`;
- existing Taxonomy requested->fallback name resolution and canonical-key fallback;
- existing legacy metadata-only tag fallback;
- response order matching the supplied Product-ID order.

`ProductCatalogReadRuntime::in_process` selects this capability from the same `CatalogService` owner. External
runtime profiles remain source-compatible and do **not** silently gain an embedded tag provider.

`ProductStorefrontIndexShadowExecutor.tag_hydration` is created only after raw `projected` succeeds. Product
IDs come from `projected.items`; distribution does not construct `TaxonomyService`, query `product_tags`, or
read Product storage directly. Missing capability/owner error is retained separately and cannot replace or
mutate the authoritative owner result, raw Index page, or `public_projected` page.

This closes the tag-hydration **source boundary** without adding localized tag names to Product Index schema and
without pretending relation-backed `tag_ids` cover legacy metadata-only tags.

## Search/collation/EAV source state

Product owns the 1022-byte effective Storefront title-search bound compatible with generic 1024-byte
`TextLike`. The retained collation packet observes real owner/default `LIKE` against Index `COLLATE "C"` and
remains execution/admission pending.

Product-owned EAV resolution still supplies neutral typed term expressions to the shadow builder; missing
option identities remain bind-free `Never`. The raw core/EAV PostgreSQL packets remain current-key source
evidence.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of Storefront core/EAV/collation and actualized retained Product packets.
2. Collation admission per deployment: any owner/default-vs-`C` mismatch keeps eligible Index cutover closed.
3. Define/admit serving latency/deadline/budget policy for Index execution plus post-page Product hydration.
4. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.
5. Any future serving router must preserve typed channel-less and deep-page owner-native branches.

## Next source slice

Define a **non-serving serving-budget policy** before any traffic-switch adapter. It must place explicit bounds
on Index work and post-page Product owner hydration, honor request deadlines, and retain owner-native behavior
when the required budget/capability is unavailable. Do not switch mounted Storefront traffic in that slice.

## Source guards

- `verify-index-product-storefront-channel-scope-policy.mjs` locks channel-less owner-native policy;
- `verify-index-product-storefront-deep-page-policy.mjs` locks deep-page owner-native policy;
- `verify-index-product-storefront-public-projection.mjs` locks raw/public placeholder separation;
- `verify-index-product-storefront-tag-hydration.mjs` locks bounded Product-owned tag hydration including
  Taxonomy and legacy metadata fallback semantics;
- `verify-index-product-storefront-shadow-executor.mjs` locks post-page enrichment after raw Index execution;
- `verify-product-catalog-read-runtime-composition.mjs` locks optional embedded tag capability without external
  implicit fallback;
- current Storefront equivalence/EAV/collation/key-4 guards remain retained;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
