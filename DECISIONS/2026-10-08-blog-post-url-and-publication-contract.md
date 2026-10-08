# Blog post URL, publication timestamp, and public locale contract

- Date: 2026-10-08
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-blog` (post lifecycle, slug history, featured image contract); `apps/next-frontend` storefront consumes the contract
- Extends: `DECISIONS/2026-03-28-multilingual-content-contract.md`
- Supersedes: None
- Superseded by: None

## Context

The 2026-10-08 Blog engineering audit found four contract defects in the post
domain:

1. Public list, RSS feed, and related-post reads ignored the request locale and
   used the tenant default locale (H-1).
2. Post slugs were built by an ASCII-only normaliser, so a Cyrillic title without
   an explicit Latin slug could not be saved, while Taxonomy route keys are
   transliterated (H-2).
3. Changing a post slug removed the old public URL without any redirect; Blog did
   not own a slug history, and `CanonicalUrlChanged` is emitted only by
   `rustok-content` orchestration (H-3).
4. `published_at` was overwritten on every publish and cleared on unpublish, so a
   republished post received a new public date, RSS `pubDate`, and JSON-LD
   `datePublished` (H-4).

The audit also found a dead `view_count` field (H-5) and an unvalidated
`featured_image_url` that reached SEO and JSON-LD output (H-10).

The Blog schema is unreleased (no git tag exists), but earlier Blog migrations
are already applied in development databases, so schema changes are added as
new ordered migrations rather than by editing applied migrations.

## Decision

### Public locale

Public Blog reads resolve the locale through `rustok_api::resolve_graphql_locale`
(explicit filter locale, then request context locale, then platform fallback)
and then apply the canonical fallback chain
`requested -> explicit fallback -> en -> first available` from
`rustok-content::locale`. The public GraphQL list no longer falls back to the
tenant default locale when the request carries a locale. Storefront RSS and
related-post reads pass the route locale explicitly.

### Slug normalization

Blog post slugs are global canonical identifiers and stay locale-neutral. They
are normalized through `rustok_taxonomy::normalize_term_route_key`, the same
primitive used for Taxonomy route keys and Blog Category/Tag route keys. Blog
does not keep a local transliteration copy. The 255-byte storage limit applies
after normalization.

### Retired slug history

Blog owns `blog_post_slug_history (tenant_id, slug) -> post_id`, a derived lookup
for retired canonical slugs. Invariants:

- a slug is either the current `blog_posts.slug` of exactly one post or a
  retired-slug record of at most one post in the same tenant, never both;
- claiming a slug for a create or a rename deletes any retired-slug record for it
  in the same transaction;
- renaming a post inserts the previous slug as a retired-slug record in the same
  transaction;
- deleting a post cascades its retired-slug records through a composite foreign
  key `(tenant_id, post_id) -> blog_posts (tenant_id, id)`.

`PostService::get_post_by_slug_with_locale_fallback` resolves the current slug
first and then the retired slug. The returned post always carries its current
canonical slug, and the storefront issues a permanent redirect when they differ.

### Publication timestamp

`blog_posts.published_at` is the first publication time. `publish` sets it only
when it is still empty. `unpublish`, `archive`, and `restore` preserve it. It
is `None` only for posts that were never published. Public date fields therefore
do not move when a post is republished. A later "last changed" value is exposed
through `updated_at`.

### Featured image contract

`featured_image_url` is validated on create and on update (`Patch::Set`) before
persistence. Accepted values are an absolute `http`/`https` URL or a root-relative
path that does not start with `//`. Values are at most 2048 characters and contain
no control characters or surrounding whitespace. Other schemes are rejected.

### Removed counter

`view_count` is removed from the entity, DTO, search projection, and schema
(`m20261008_000031`). It had no writer. Real view analytics belong to a dedicated
analytics owner, not to a column on `blog_posts`.

## Non-goals

- Scheduled publishing, revisions, trash, and draft preview tokens.
- Server-side text search and facets for the post list.
- Redirects in the Leptos storefront; it resolves retired slugs but does not
  redirect. The redirect contract is enforced by the Next storefront route.
- Media-asset references for featured images (a raw validated URL is kept).

## Sources of truth and ownership

- `blog_posts` owns the current canonical slug, status, and `published_at`.
- `blog_post_slug_history` is Blog-owned derived state for redirects only. It is
  not used for authorization or for reads of current content.
- Locale resolution is owned by `rustok-content::locale`; the GraphQL request
  locale is owned by `rustok-api`.

## Invariants

### Allowed states

- Current slug and retired slugs are disjoint per tenant.
- A post that has ever been published keeps the same `published_at` value for the
  rest of its life; only a first publication may set it.

### Forbidden states

- A retired slug that points at a post which currently owns that slug.
- A stored `featured_image_url` with a non-http(s) scheme or with a length above
  2048 characters.

## Data, transaction, and concurrency boundary

- Slug claim, slug retirement, and the post update run in one database
  transaction. The version compare-and-set on `blog_posts` is unchanged.
- The composite foreign key on `blog_post_slug_history` and the primary key
  `(tenant_id, slug)` are the database-enforced invariants.
- Deleting a post is a cascade on the history table; no separate cleanup runs.

## Context dimensions

- Tenant: every slug and history row is tenant-scoped.
- Locale: public reads use the request locale with the canonical fallback chain.
  Slugs are locale-neutral.
- Channel: public visibility is still checked after slug resolution.

## Events and projections

- No new domain events. The Blog search projection is updated through the
  existing full-reindex path. Its `view_count` key is removed.
- `CanonicalUrlChanged` is not emitted by Blog in this decision.

## Failure semantics

- A duplicate slug claim fails with the existing duplicate-slug error.
- A history row that references a missing post is an invariant failure, not a
  silent miss.
- Invalid featured image URLs fail validation before any write.

## Migration and cutover

- `m20261008_000030_create_blog_post_slug_history` creates the table. No backfill
  is needed: existing posts have no retired slugs.
- `m20261008_000031_remove_blog_post_view_count` drops the column. Its `down`
  is intentionally irreversible under the Zero-Legacy Policy.
- Existing posts keep their current `published_at`. Posts that were unpublished
  before this change already have `published_at = NULL`, because the previous code
  cleared it. That value cannot be recovered, so no backfill is attempted; their
  first publication after the change sets a new timestamp.

## Alternatives considered

- Emit `CanonicalUrlChanged` from Blog and rely on `rustok-seo`. Rejected for now:
  `rustok-seo` redirect rules are keyed on that event and its rule owner is
  content orchestration; Blog would need a second owner contract. The Blog-owned
  history keeps the redirect source inside the owner module.
- Store slugs per locale. Rejected by the 2026-03-28 multilingual contract: Blog
  slugs are global canonical identifiers.
- A separate Blog-specific transliteration function. Rejected: duplicates the
  Taxonomy primitive and makes Blog and Taxonomy route keys diverge.
- Keep `view_count` and implement it. Rejected: no analytics owner exists yet.

## Verification

- Rust unit tests in `rustok-blog`: slug transliteration; featured image URL
  acceptance and rejection.
- Migration ordering is covered by the existing migration dependency descriptors.
- Still required before status moves to Implemented: `cargo test -p rustok-blog`,
  PostgreSQL migration smoke for both new migrations, a rename-and-redirect
  integration test (old slug resolves to the current post), and a republish test
  that asserts the first `published_at` is preserved.

## Consequences

- Public URLs survive slug renames in the Next storefront (permanent redirect).
- Russian titles produce readable slugs without a manual Latin slug.
- The public `published_at` is a first-publication timestamp. Clients that used
  it as "last published" must use `updated_at` instead.
- The featured image is a validated URL, not a media reference. Migrating to a
  media-asset reference remains follow-up work.
