# ADR: Boundary between `rustok-index` and `rustok-search`

## Status

Accepted

## Context

After extracting `rustok-search` into a separate core module, the repository still
had an architectural risk: `rustok-index` and `rustok-search` solve similar,
but not identical tasks, and without an explicit boundary, product search flows
can easily start stacking back into the index/read-model layer.

This is especially dangerous for the following areas:

- ownership of `search_documents` and search-facing query contract;
- ranking/relevance, dictionaries, and merchandising rules;
- admin/storefront search UI and analytics;
- optional connector crates for external engines;
- runtime dependency direction between indexing and search capabilities.

## Decision

The following architectural boundary is adopted:

- `rustok-index` remains the platform indexing/read-model substrate module.
- `rustok-search` remains the product search module and the sole owner of
  search-facing API/UX/runtime contract.
- Canonical search storage (`search_documents`, query analytics, dictionaries,
  query rules) lives in `rustok-search`, not in `rustok-index`.
- `rustok-search` may read domain tables directly and may optionally use
  neutral read-model data from `rustok-index`, but does not depend on
  it as a source-of-truth for product search.
- Dependency direction is only allowed as `search -> index`, if this
  really helps ingestion/read-model reuse; the reverse dependency
  `index -> search` is prohibited.
- External search engines are connected only through dedicated connector crates
  registered under `rustok-search`; domain modules do not integrate with
  provider SDKs directly.

## Consequences

- Product search contracts, ranking, synonyms/stop words, query rules,
  autocomplete, analytics, and search UI do not return to `rustok-index`.
- `rustok-index` can evolve as a common substrate for denormalized reads,
  sync/rebuild/consistency tooling, and cross-module joins without pressure from
  storefront/admin UX.
- Any attempt to move search-facing API or engine-specific behavior into
  `rustok-index` now requires a new ADR.
