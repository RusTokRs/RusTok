# M7 Product Storefront localized query architecture

Status: `source_decision_complete_implementation_and_evidence_pending`.

## Decision

Keep the current owner Storefront behavior and keep exactly one current Product Index schema. Close the
remaining locale/search mismatch in the **Index query layer** with a generic localized-entity identity
fold over the existing physical locale rows.

This decision deliberately rejects both shortcuts that would make parity ambiguous:

- do not narrow Product owner search/fallback semantics merely to fit the current single-locale
  `IndexQueryScope`;
- do not add another Product routing key, compatibility source, or Storefront-only Product schema.

The current Product routing key remains an internal storage/replay identity. Localized Storefront query
semantics are a query-shape concern, not a new Product schema version.

## Owner contract preserved

`CatalogService::list_published_products_with_query` remains authoritative until cutover evidence passes.
The replacement query path must preserve these existing semantics exactly:

- one result identity per Product;
- title search may match **any** Product translation;
- result localization prefers the requested locale, then the fallback locale;
- if neither requested nor fallback translation is present, owner projection uses its existing
  placeholder behavior rather than selecting an unrelated translation as user-visible content;
- Product status/publication/category/channel/EAV predicates apply to Product identity;
- ordering is by `published_at`/`created_at` plus stable Product ID tie-break;
- pagination and exact count operate on distinct Product identities, never translation rows.

Product creation currently requires at least one translation. Retained parity evidence must still cover
translation removal/lifecycle boundaries instead of assuming that creation-time invariant is sufficient
forever.

## Generic Index model

The localized fold operates only for a schema whose locale mode is `Required` and only when a caller
explicitly requests the fold. Ordinary `IndexQuery` remains exact-locale and unchanged.

For one folded query, the logical root identity is:

`(tenant_id, schema_ref, entity_id)`

Locale is intentionally excluded from that **query result identity** while remaining part of physical
Index storage, mutation, replay, absence, freshness, and ordinary query identity.

The fold receives two canonical locale roles:

1. requested locale;
2. fallback locale, de-duplicated when equal to requested.

It then reasons over all current/admitted physical locale rows for each root identity in one SQL query.
A consumer must not emulate this contract by issuing independent locale queries and merging pages in
memory.

## Search semantics

Any-locale title search is an identity predicate:

- a Product identity is admitted when at least one current/admitted physical locale row satisfies the
  requested text predicate;
- the row that satisfied search does **not** become the result locale merely because it matched;
- stale/deleted/unready locale rows cannot admit an identity;
- search matching and result localization are therefore independent, matching the owner Storefront
  contract.

The future generic text-pattern primitive must be applied inside this identity predicate. Adding scalar
substring/LIKE support to an exact-locale query alone remains insufficient and is not admitted by this
decision.

## Effective localized projection

For an admitted Product identity, effective localized projection is selected independently from search:

1. current/admitted requested-locale row;
2. otherwise current/admitted fallback-locale row;
3. otherwise no effective localized row.

When there is no effective localized row, the Storefront adapter must preserve the owner placeholder
contract for localized fields. It must not expose title/handle/description from an arbitrary third
locale.

Locale-invariant Product fields may be read from a deterministic current/admitted identity anchor only
under the current single Product source invariant that those values are repeated identically across
locale rows for one Product source version. Retained PostgreSQL evidence must prove this together with
partial locale delivery and replay/restart cases before Storefront cutover.

## Pagination, sorting, and exact count

Grouping happens **before** pagination and exact count.

- exact count is the number of distinct admitted Product identities;
- page limit/lookahead is applied to grouped Product identities, not physical translation rows;
- the Storefront ordering tuple remains the owner tuple (`published_at`/`created_at`, companion
  timestamp, Product ID) and must be evaluated from the identity-level current Product state;
- continuation state must bind requested locale, fallback locale, localized-fold mode, schema
  fingerprint, filter/order shape, and the same identity-level ordering tuple;
- a cursor from an ordinary exact-locale query must not be accepted by the folded query path, and vice
  versa.

An implementation that separately pages requested/fallback/other locales and merges afterward cannot
provide this contract and is forbidden.

## Freshness and graph availability

Existing schema readiness, Product entity freshness, and queried link-target availability remain
mandatory. The localized fold does not weaken them.

Every physical row used to admit search, supply requested/fallback projection, or act as the deterministic
identity anchor must already satisfy the same current query admission rules that protect ordinary Product
queries. A stale translation row cannot keep a Product visible or satisfy search after the owner Product
clock has advanced.

ProductVariant/SalesChannel linked paths remain one-hop and use the existing query-path-scoped target
availability policy after Product identity admission. No new Product graph clock or relation ledger is
introduced.

## Storefront adapter boundary

The future Storefront adapter may translate owner inputs to the generic localized fold only after the
Index-side contract exists. It must then:

- map Active + published-only/category/channel visibility predicates;
- resolve public Product attribute/option codes to canonical `attribute_terms`;
- use the folded any-locale title search primitive;
- preserve requested/fallback localized Product projection;
- batch-hydrate localized Taxonomy tag names after the Product page is fixed;
- preserve owner page/per-page, exact count, ordering, and stable ID tie-break semantics.

Taxonomy localization remains Taxonomy-owned. Product Index continues to store stable tag UUIDs only.

## Required retained evidence

Before Storefront traffic can move, PostgreSQL equivalence must cover at least:

1. requested translation present;
2. requested absent + fallback present;
3. requested/fallback absent while another translation exists;
4. search match only in requested locale;
5. search match only in fallback locale;
6. search match only in a third locale while projection still uses requested/fallback rules;
7. duplicate matches in multiple locales yielding one Product identity and exact count of one;
8. two Products whose locale rows interleave physically but whose global timestamp+ID order is stable;
9. page boundary and exact-count parity across locale combinations;
10. delayed/stale locale materialization excluded by owner freshness;
11. restart/recomposition and replay redelivery preserving the same grouped identity page;
12. ProductVariant/SalesChannel target lag on a folded Product query;
13. current Product routing-key rebuild/promotion rather than any lower historical key.

Until these packets are source-ready, executed, and admitted, Storefront remains owner-native.

## Deliberate limits

This source decision does not yet:

- change `IndexQueryScope` or public Index query types;
- add the generic localized fold implementation;
- add scalar text-pattern SQL compilation;
- implement the Storefront Index adapter;
- actualize the retained Product PostgreSQL packets to the current Product routing key;
- execute or admit PostgreSQL evidence;
- run the event-contract digest admission workflow;
- add Product typed wire events;
- stage/rebuild/promote a tenant Product schema;
- switch Storefront traffic.

## Next implementation slice

Implement the generic localized-entity fold in `rustok-index` as a separate controlled query mode with
identity-level page/count/cursor semantics. Keep ordinary exact-locale `IndexQuery` behavior unchanged.
Only after that fold exists should scalar text-pattern matching be wired into its any-locale identity
predicate and consumed by a Product Storefront adapter.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
