# M7 Product Storefront public projection boundary

Status: `source_complete_taxonomy_hydration_pending`.

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

`ProductStorefrontIndexShadowExecution` retains both layers:

- `projected` — raw generic Index page and the input to identity/count/page comparison;
- `public_projected` — optional Product public page derived from a clone of successful `projected`.

A raw projection failure produces no public page. A public projection failure is retained separately and never
replaces the successful Product owner result or mutates the raw Index page.

## Why this is post-page only

The Product owner placeholders describe public presentation when no requested/fallback translation is
available. They are not stored translation values and must not become search/filter/order identities. Applying
them before query completion would change owner semantics by making a display fallback participate in
selection, ordering, exact count or pagination.

Keeping them post-page also leaves generic `rustok-index` reusable for entities whose no-localized-row public
contract differs from Product.

## Remaining projection gap

Product owner list returns localized Taxonomy tag names, while the current Index Product page retains
`tag_ids`. The public placeholder adapter deliberately does not transform these IDs.

The next source slice must batch-hydrate tag names through an owner capability after Product page identity,
ordering and exact count are fixed. Tag hydration failure must remain separate from the raw Index page and must
not cause a different Product selection.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
