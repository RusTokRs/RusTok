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
