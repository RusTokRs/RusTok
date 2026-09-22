# Multilingual content contract for `blog` / `pages` / `comments`

- Date: 2026-03-28
- Status: Accepted

## Context

After splitting `blog`, `pages` and `comments` into their own tables, it turned out that the
multilingual model itself still diverges between modules:

- locale codes are normalized differently;
- fallback locale in the read-path behaves inconsistently;
- `comments` lived on its own custom locale resolution, not on the shared helper;
- `pages` did not perform fallback during slug lookup by locale;
- slug policy already differs between domains:
  - `blog` uses a global canonical slug;
  - `pages` use a locale-aware slug at the translation level;
  - `forum` still remains on the legacy model and must be brought to an explicit contract before cutover.

If this is not fixed now, the new split will simply proliferate incompatible
locale/slug semantics.

## Decision

### 1. Locale normalization is unified for all content-like modules

Canonical normalization rule:

- trim the input locale;
- `_` is replaced with `-`;
- value is lowercased;
- empty locale and explicitly invalid values are rejected.

The shared helper lives in `rustok-content::locale` and is reused by all
content-like modules instead of local copies.

### 2. Locale fallback order is fixed

Canonical locale resolution order:

1. requested locale;
2. explicit fallback locale, if passed by the calling layer;
3. platform fallback locale `en`;
4. first available locale.

This rule is mandatory for `blog`, `pages`, `comments` and any future
forum-owned storage after migration off `NodeService`.

### 3. `rustok-content` remains the shared owner for locale helpers

`rustok-content` does not own the domain tables of `blog/pages/comments`, but remains the
canonical place for:

- locale normalization helpers;
- locale fallback helpers;
- shared rich-text body contracts.

### 4. Slug policy must be explicit at the domain level

The platform does not impose a single slug mode for all entities.

Two modes are allowed, but each domain must explicitly choose one:

- `canonical/global slug`
  - one slug per entity regardless of locale;
  - suitable for `blog post` and other canonical publication entities;
- `locale-aware slug`
  - slug lives at the translation level;
  - suitable for `pages` and other locale-routed surfaces.

Mixing both modes within the same entity is not allowed.

### 5. `forum` must choose slug/locale contract before storage cutover

Before moving `forum` to forum-owned persistence, the following must be explicitly decided:

- will the topic slug remain locale-aware;
- how should forum topic lookup use the tenant fallback locale;
- will forum share page-like or blog-like slug semantics.

This decision must be made before the forum storage split is completed, not after.

## Consequences

- `blog`, `pages`, `comments` must use shared locale helpers from `rustok-content`.
- Any new content-like module must not introduce its own locale normalization logic.
- `pages` slug lookup must use the same fallback contract as the other read-paths.
- `comments` must not have a separate locale resolution policy.
- The split-plan backlog must separately hold the decision on `forum` slug/locale semantics.
