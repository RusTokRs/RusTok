# ADR: bounded Blog tag pagination through a derived usage projection

## Status

Accepted

## Date

2026-09-22

## Context

Blog tag listing previously loaded every visible Taxonomy term and every matching
`blog_post_tags` relation into Rust memory, sorted the complete collection in the
service, and only then applied the API page boundary. The page-size DTO bound did
not make the underlying database read bounded.

The required public order is:

- `use_count DESC`;
- `taxonomy canonical_key ASC`;
- stable `tag_id ASC` as the final tie-break.

Blog owns `blog_post_tags`, while Taxonomy owns term identity and `canonical_key`.
Blog must not read Taxonomy persistence directly merely to implement its own
attachment semantics.

## Decision

Introduce `blog_tag_usage` as a Blog-owned derived read projection.

Its canonical source is:

- attachment membership and usage count: Blog-owned `blog_post_tags`;
- term identity and stable ordering key: Taxonomy-owned term identity,
  projected through `TaxonomyOwnerReader`.

The projection stores:

- `(tenant_id, tag_id)` identity;
- immutable Taxonomy `canonical_key` used for deterministic list ordering;
- `use_count`.

Projection semantics are:

- every Blog-module tag has a row, including `use_count = 0`;
- a global Taxonomy tag has a row only while it is attached to at least one Blog
  post;
- Taxonomy term deletion removes the projection row through the tenant-scoped
  foreign key cascade.

The projection is updated in the same Blog transaction as the canonical
`blog_post_tags` mutation. Post deletion decrements usage before the Blog post
cascade removes the relations. A usage decrement that would underflow is an
internal invariant failure and rolls back the transaction.

Tag listing uses a database paginator over `blog_tag_usage` and fetches only the
bounded page of term IDs. Taxonomy localization is then materialized for that
bounded page through the owner reader inside the same read transaction.

The projection is derived state, never an authoritative write source. It is
backfilled from `blog_post_tags` during its schema migration and is
reconstructable from the same canonical attachment relation plus Taxonomy
canonical identity.

## Invariants

- tenant identity is `(tenant_id, tag_id)`;
- `use_count >= 0`;
- the projection row `canonical_key` equals the current Taxonomy canonical key;
- Blog-local zero-use terms remain listed;
- zero-use global terms are not listed;
- all Blog-owned mutation paths that add/remove post-tag relations update the
  projection in the same transaction;
- Taxonomy-owned term deletion cascades the derived projection row;
- the public ordering is total and deterministic.

## Alternatives rejected

### In-memory pagination

Rejected because it still performs an unbounded database read and materializes
all terms/relations in the service process.

### Cross-module SQL join

Rejected because Blog would depend on Taxonomy private persistence tables and
would bypass the owner boundary.

### Changing the public tie-break to UUID

Rejected because it would silently change the documented/current tag-list
ordering without solving the underlying bounded-read problem.

## Verification

Source verification must confirm:

- `blog_tag_usage` migration, entity, and backfill exist;
- TagService list path uses database pagination and never loads all usage rows;
- post tag create/update/delete paths update the projection transactionally;
- post deletion updates the projection before relation cascade;
- Taxonomy delete remains the cascade owner for shared/global term removal;
- no Blog query joins or reads Taxonomy private entities for tag listing.

Maintainer-owned runtime evidence remains a separate status and must not be
promoted by source inspection alone.
