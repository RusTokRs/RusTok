# M7 Product Storefront public projection boundary

Status: `source_complete_with_separate_tag_hydration`.

## Raw Index page remains generic

The localized Index query and decoder remain Product-neutral. If a Product has neither the requested nor the
fallback translation row, raw projected `title` and `handle` are `IndexValue::Null`.

That raw state is retained intentionally. It participates in no Product public placeholder logic, and existing
PostgreSQL equivalence evidence continues to observe it directly.

## Product public post-page layer

`rustok-distribution::product_index::storefront_projection` applies owner public placeholder semantics only to
an already-decoded `IndexQueryPage`:

- root `title: Null` becomes `"Untitled product"`;
- root `handle: Null` becomes `""`;
- existing string values remain unchanged;
- missing, duplicate or wrong-typed root title/handle projections fail closed.

The adapter preserves item identity/order, exact count, `has_more`, cursor, unrelated projected fields and
`tag_ids`. It has no query, filtering, sorting, localized-fold or execution dependency.

`ProductStorefrontIndexShadowExecution` retains separate layers:

- `projected` — raw generic Index page and input to identity/count/page comparison;
- `public_projected` — Product title/handle page derived from a clone of successful `projected`;
- `tag_hydration` — separate Product-owner tag projection keyed by identities from raw `projected`.

A raw projection failure produces neither public page nor tag hydration. Public projection and tag-hydration
failures are retained independently and never replace the successful Product owner result or mutate the raw
Index page.

## Why placeholders stay post-page

Product owner placeholders describe public presentation when no requested/fallback translation is available.
They are not stored translation values and must not become search/filter/order identities. Applying them before
query completion would change selection, ordering, exact count or pagination semantics.

Keeping them outside generic `rustok-index` also preserves reuse for entities whose no-localized-row public
contract differs from Product.

## Tags remain a separate owner projection

The placeholder adapter intentionally leaves `tag_ids` unchanged. Product tag parity is handled by
`ProductStorefrontTagReadPort` after the raw page is fixed.

That capability is keyed by Product IDs rather than only `tag_ids` because Product owner reads preserve a
legacy `metadata.tags` fallback when relation-backed tags are absent. The separate tag result therefore carries
owner semantics without changing the generic Index page contract.

See `m7-product-storefront-tag-hydration.md` for the retained boundary and remaining serving-budget gate.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
