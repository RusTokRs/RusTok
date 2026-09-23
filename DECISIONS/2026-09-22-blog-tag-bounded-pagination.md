# Bounded Blog tag pagination through a derived usage projection

- Date: 2026-09-22
- Decision status: Accepted
- Implementation status: Implemented
- Owners: rustok-blog
- Extends: None
- Supersedes: None
- Superseded by: None

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

## Sources of truth and ownership

- `blog_post_tags` owns post-to-tag attachment facts.
- `rustok-taxonomy` owns canonical term identity, scope, and route keys.
- `blog_tag_usage` is a pure read-side derived projection owned by `rustok-blog`.

## Invariants

### Allowed states

- `use_count >= 0`.
- Projection row `canonical_key` matches Taxonomy `canonical_key`.
- Total ordering deterministically determined by `use_count DESC`, `canonical_key ASC`, `tag_id ASC`.

### Forbidden states

- Unbounded queries or loading all terms into application memory.
- Cross-module direct joins on Taxonomy private tables.
- Negative `use_count`.

## Non-goals

- Does not transfer ownership of Taxonomy terms to Blog.
- Does not change public API ordering.

## Data, transaction, and concurrency boundary

- Attachment mutations and `blog_tag_usage` updates execute in the same transaction.
- Post deletion decrements tag usage before deleting the post.
- Foreign key constraints maintain tenant-level referential integrity.

## Context dimensions

- Isolated per `tenant_id`.

## Events and projections

- Invalidation and rebuild are deterministic from `blog_post_tags` and Taxonomy terms.
- Reindex signals emitted on tag changes.

## Failure semantics

- Underflow in usage counts triggers an internal invariant failure and transaction abort.
- Tag list pagination fails closed if the database query fails.

## Migration and cutover

- Migration creates `blog_tag_usage` and backfills rows from existing `blog_post_tags`.

## Alternatives considered

- In-memory pagination (rejected: unbounded memory and database read).
- Cross-module SQL joins across private tables (rejected: violates ownership boundary).
- Changing public tie-break to UUID (rejected: alters public ordering without bounding reads).

## Verification

- `cargo test -p rustok-blog --lib services::tag::pagination_tests`
- `npm run verify:module-reference-contract`

## Consequences

- Tag pagination executes in bounded database queries regardless of total catalog size.
- Clean isolation between Blog attachment logic and Taxonomy canonical identity.
