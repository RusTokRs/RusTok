# M7 Product Storefront tag hydration boundary

Status: `source_complete_serving_budget_pending`.

## Why raw `tag_ids` are not enough

Product Index materializes `tag_ids` from `product_tags`. That is correct canonical relation identity, but it is
not the complete public Product read contract: legacy Products with no tag relations may still expose
normalized `metadata.tags` through Product owner reads.

A post-page adapter that resolves only Index `tag_ids` would therefore drop owner-visible legacy tags. The
hydration boundary must be keyed by the already-selected Product identities, not treated as a pure UUID-name
lookup over Index fields.

## Product-owned capability

`ProductStorefrontTagReadPort::hydrate_storefront_product_tags` accepts:

- up to 48 already-selected Product IDs;
- fallback locale;
- requested locale and tenant identity from `PortContext`.

The embedded implementation is `CatalogService` and:

1. requires a read policy;
2. rejects nil/duplicate/over-bound Product identities;
3. tenant-scopes and verifies every requested Product row;
4. calls the existing `load_product_tag_map` owner helper;
5. preserves Taxonomy requested->fallback resolution and canonical-key fallback;
6. preserves legacy normalized `metadata.tags` only when relation-backed tags are absent;
7. returns one item per requested Product ID in input order, with empty tags represented explicitly by an empty
   vector.

`ProductCatalogReadRuntime::in_process` selects the tag capability from the same Product owner instance.
External profiles remain compatible but have no implicit embedded tag capability.

## Index composition

The non-serving Storefront shadow executor calls tag hydration only after raw `IndexQueryPage` success. It
extracts Product IDs from `projected.items`, not from the authoritative owner page and not from `tag_ids`.

Hydration is retained as a separate `tag_hydration` result. It does not mutate:

- raw `projected`;
- Product placeholder `public_projected`;
- identity/order/exact-count/page/cursor comparison.

Distribution does not query Product/Taxonomy storage and does not construct `TaxonomyService`.

## Serving boundary

This source capability removes the semantic tag-name/legacy-fallback gap, but it adds an owner read after the
Index page. Before any serving cutover, the combined Index + Product hydration path needs an explicit deadline
and latency budget plus maintainer-run evidence. Missing external capability must remain fail-closed/owner-native
rather than silently dropping tags.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
