# Blog post URL, publication timestamp, and public locale contract

- Date: 2026-10-08
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-blog` (post lifecycle, slug column, featured image contract); `rustok-content` (canonical URL and alias registry, including the Blog post route); `apps/next-frontend` storefront consumes the contract
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
3. Changing a post slug removed the old public URL without any redirect. The
   retired route was not recorded in the canonical route registry (H-3).
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

### Slug routes and redirects

The Blog post route is owned by the canonical URL registry of `rustok-content`
(`canonical_url` and `url_alias`). Blog does not keep its own slug table. Blog
writes the registry only through `CanonicalUrlWriter`, the single writer that
also serves content orchestration. Rules:

- The route is `canonical_post_route(slug)` = `/modules/blog?slug={slug}`. It is
  defined once in `rustok-blog` and reused by content orchestration and the SEO
  target projection.
- Blog slugs are global canonical identifiers. Their routes are stored under
  `CANONICAL_POST_ROUTE_LOCALE`, which is `rustok_api::PLATFORM_FALLBACK_LOCALE`.
  `resolve_route` falls back to that locale for every requested locale.
- Create claims the route. `release_alias_route_in_tx` releases any retired alias
  of the route in every locale, and `apply_canonical_url_mutations` then makes it
  canonical. A retired route therefore never shadows a live slug.
- Rename releases the new route and makes the new canonical route. The previous
  route is passed as an alias, so the writer stores it as a `url_alias` and emits
  `CanonicalUrlChanged`. `rustok-seo` turns that event into a redirect when the
  tenant enables submodule redirects and automatic slug-change redirects.
- Delete removes every canonical and alias row of the post through
  `remove_target_routes_in_tx`, in the same transaction as the post delete, and
  publishes `UrlAliasPurged`.
- Public reads resolve the current slug first. If none matches, they resolve the
  route through `CanonicalUrlService::resolve_route` with the request locale. The
  returned post always carries its current slug, so the storefront issues a
  permanent redirect when they differ. A route that now belongs to another module
  (for example a demoted post served as a forum topic) resolves to no Blog post.
- Content orchestration follows the same rules: promote claims the Blog route
  and retires the topic routes, and demote retires the Blog route in the global
  locale.

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
- `canonical_url` and `url_alias` (owned by `rustok-content`) hold every route,
  including the Blog post route. They serve redirects only. They are not used for
  authorization or for reads of current content.
- Locale resolution is owned by `rustok-content::locale`; the GraphQL request
  locale is owned by `rustok-api`.

## Invariants

### Allowed states

- A Blog route is canonical for at most one post. A retired route is an alias of
  at most one target, across all locales.
- A post that has ever been published keeps the same `published_at` value for the
  rest of its life; only a first publication may set it.

### Forbidden states

- A retired alias of a route that is also the live canonical route of a target.
- A stored `featured_image_url` with a non-http(s) scheme or with a length above
  2048 characters.

## Data, transaction, and concurrency boundary

- Route claim, rename, and delete write the registry in the same database
  transaction as the post write. The version compare-and-set on `blog_posts` is
  unchanged.
- The registry has no foreign key to `blog_posts`. Delete removes the post's
  routes explicitly through `remove_target_routes_in_tx`.

## Context dimensions

- Tenant: every route row is tenant-scoped.
- Locale: the Blog route is stored under the platform fallback locale. Public
  reads use the request locale through `resolve_route`, with the canonical
  fallback chain. Slugs are locale-neutral.
- Channel: public visibility is still checked after slug resolution.

## Events and projections

- No new domain event type. Rename emits `CanonicalUrlChanged` and
  `UrlAliasPurged` through the writer. Delete emits `UrlAliasPurged`.
- The Blog search projection is updated through the existing full-reindex path.
  Its `view_count` key is removed.

## Failure semantics

- A duplicate slug claim fails with the existing duplicate-slug error.
- A route whose live canonical belongs to another target cannot be released; the
  release fails with a validation error.
- A canonical route that references a missing post is an invariant failure, not a
  silent miss.
- Invalid featured image URLs fail validation before any write.

## Migration and cutover

- The unreleased `blog_post_slug_history` table and its migration
  `m20261008_000030` are removed before release. Its migration
  `m20261008_000031` now depends on `m20260924_000029`.
- Posts that predate the registry write have no canonical row. A rename of such a
  post still records the old route as an alias, and their current slug resolves
  directly, so no backfill is needed.
- `m20261008_000031_remove_blog_post_view_count` drops the column. Its `down`
  is intentionally irreversible under the Zero-Legacy Policy.
- Existing posts keep their current `published_at`. Posts that were unpublished
  before this change already have `published_at = NULL`, because the previous code
  cleared it. That value cannot be recovered, so no backfill is attempted; their
  first publication after the change sets a new timestamp.

## Alternatives considered

- A Blog-owned slug history table. Rejected: it duplicated the canonical URL
  registry that `rustok-content` already owns and that `rustok-seo` already
  consumes. Blog writes through the registry instead, so one source of truth
  holds every public route.
- Reject a retired slug on create. Rejected: a new post must be able to take a
  slug that an older post retired. The claim releases the old alias explicitly.
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
  `cargo test -p rustok-content`, `cargo test -p rustok-content-orchestration`,
  a PostgreSQL migration smoke for `m20261008_000031`, a rename-and-redirect
  integration test (old route resolves to the current post), a claim test (a new
  post takes a retired slug), a delete-purge test, and a republish test that
  asserts the first `published_at` is preserved.

## Consequences

- Public URLs survive slug renames in the Next storefront (permanent redirect).
  The redirect source is the registry alias, which is the same record that
  content orchestration and SEO use.
- Russian titles produce readable slugs without a manual Latin slug.
- The public `published_at` is a first-publication timestamp. Clients that used
  it as "last published" must use `updated_at` instead.
- The featured image is a validated URL, not a media reference. Migrating to a
  media-asset reference remains follow-up work.
