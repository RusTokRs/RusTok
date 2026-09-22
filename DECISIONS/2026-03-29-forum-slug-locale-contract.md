# Forum slug/locale contract after content split

- Date: 2026-03-29
- Status: Accepted

## Context

After moving `forum` to module-owned storage, one last open split
question remained: how exactly locale fallback and slug semantics combine on the forum read-path.

The base multilingual ADR already fixed the common rules:

- locale normalization goes through the shared helper from `rustok-content`;
- fallback order is the same for content-like domains:
  `requested -> explicit fallback -> en -> first available`;
- `forum` must not remain on an implicit legacy slug/locale model after cutover.

At the same time, the live forum code already shows two distinct entities:

- category translations indeed carry their own `slug`;
- topic translations store `slug` alongside the translation, but when creating a new locale
  translation it is copied from the seed translation and is not used as a separate
  locale-routed lookup key;
- the public forum API is currently ID-based and does not promise `get_by_slug` / list-by-slug
  routing for either categories or topics.

This needs to be fixed explicitly so that the split can be closed without false assumptions.

## Decision

### 1. Shared locale contract is mandatory for `forum` as well

`rustok-forum` uses the shared locale normalization/fallback helpers from
`rustok-content` for category/topic/reply read-path.

All forum read surfaces with locale-sensitive data must consistently
return:

- `requested_locale`;
- `effective_locale`;
- `available_locales`.

This rule applies to both detail and list DTO/GraphQL surfaces.

### 2. Category slug is a locale-aware translation field

`forum_category_translation.slug` is considered a locale-aware slug at the translation level.

Consequences:

- `CategoryResponse` and `CategoryListItem` return the slug of the same resolved translation
  as `name` / `description`;
- when adding a new locale translation, the category slug may differ from other
  locales;
- if a category lookup by slug is added later, it must use the same
  locale fallback contract, not bypass it.

### 3. Topic slug remains a stable thread label

`forum_topic_translation.slug` is not currently considered a separate locale-routed slug
contract.

Current semantics:

- topic slug is set when creating the topic;
- when adding a new locale translation, the slug is by default copied from the
  seed translation;
- the slug acts as a stable thread label in responses, not as a promised
  locale-aware route key.

This preserves compatibility of current DTO/storefront surfaces without introducing
a non-existent public routing contract.

### 4. Public forum contract remains ID-based

At the current stage, `forum` does not provide a canonical public lookup by slug.

Consequences:

- the split-track is considered closed without additional topic/category slug lookup;
- if `get_by_slug` or a slug-routed storefront path is added later, it will
  be a separate product/API change and a separate contract decision;
- such a future lookup must explicitly choose one of the two models:
  `locale-aware slug` or `stable canonical slug`, and not mix them within
  a single entity.

## Consequences

- The multilingual ADR for `forum` is considered closed by a separate domain-specific ADR;
- `rustok-content` remains the shared owner of locale helpers, but not the storage-owner of forum;
- docs and public contract of `rustok-forum` must describe category/topic slug
  semantics separately;
- the split-track `blog / forum / pages off rustok-content` no longer depends on
  an implicit forum slug/locale arrangement.

## 2026-08-06 amendment — FORUM-24L category route identity

FORUM-24L exercises the future category lookup anticipated above without changing
the category slug model:

- the transport-neutral canonical category path is
  `/{locale}/forum/c/{slug}`;
- `slug` remains a locale-aware translation field and the category UUID remains
  internal identity;
- category hierarchy is not embedded in the canonical path, so hierarchy moves do not change this route;
- reverse lookup uses the shared order
  `requested -> explicit fallback -> en -> first available`;
- first-available lookup is accepted only when all remaining active candidates
  belong to one category identity; cross-category ambiguity fails closed;
- an exact requested route owned by an archived category does not fall through
  to another locale or category;
- route identity does not authorize public disclosure; audience, channel and
  module visibility remain required at a future transport boundary.

This amendment supersedes the ID-only statement in section 4 for category route
identity only. It does not add a GraphQL field, REST endpoint, storefront mount,
category alias history, hreflang or SEO publication policy. The later FORUM-24
topic-route slices define their own separate stable-short-identity contract.

## 2026-08-06 amendment — FORUM-24M category slug history

FORUM-24M defines the history rule for the flat localized category route:

- changing either an explicit category slug or the existing name-derived slug
  records the old `(tenant, locale, slug)` key in an immutable redirect ledger;
- current and historical route keys share one namespace;
- a historical key cannot be reused by another category or reclaimed by its
  original category;
- category creation and new translation creation consult the same reservation
  owner, so alias protection cannot be bypassed through a second write path;
- exact-locale aliases participate in the same locale precedence as current
  translations and therefore precede fallback-locale current routes;
- the alias target is the same category identity and its current canonical slug
  is recomputed rather than copied into history;
- archived categories remain undisclosed through both current and historical
  routes;
- alias resolution still does not authorize visibility or SEO publication.

This amendment adds no hierarchy-derived path, move redirect, category tombstone,
GraphQL field, REST endpoint, storefront mount, hreflang or SEO publication rule.
