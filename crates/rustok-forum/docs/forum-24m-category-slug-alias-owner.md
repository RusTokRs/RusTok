# FORUM-24M category slug alias owner

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24M extends the flat localized category route introduced by FORUM-24L:

```text
/{locale}/forum/c/{slug}
```

It records immutable redirects when one existing category translation changes its slug. The category UUID remains the internal identity, and the alias always resolves back to that same category before the canonical slug is recomputed.

Machine contract:

```text
crates/rustok-forum/contracts/forum-category-slug-alias-owner.json
```

## Historical route namespace

Migration `m20260806_000026_add_forum_category_route_aliases` adds PostgreSQL and SQLite `forum_category_route_aliases` storage with one unique historical route key:

```text
(tenant_id, locale, slug)
```

Each row stores:

- immutable alias identity;
- owning category identity;
- normalized locale and old slug;
- bounded reason `Category slug changed`;
- creation time.

Update and delete triggers make rows append-only. Cross-table guards reject a current translation that attempts to claim an alias key and reject an alias inserted while the same current route still exists.

Historical route keys are deliberately not reusable, even by the category that originally owned them. Allowing reuse would make a previously permanent redirect become canonical again and would permit another category to hijack an indexed or bookmarked route.

## Atomic write composition

The existing public category update contract is unchanged. `CategoryService::update` still accepts `UpdateCategoryInput`; its transactional owner now composes route history before commit.

Both existing slug mutation modes are covered:

1. an explicit `input.slug` change;
2. the existing name-derived slug change when `input.name` is supplied without `input.slug`.

The owner:

1. normalizes the locale, old slug and proposed slug;
2. acquires deterministic route-key locks for old and new keys;
3. verifies exact ownership of the current route;
4. rejects any current or historical claim on the proposed key;
5. updates the translation slug;
6. inserts the immutable old-route alias;
7. publishes the existing Forum projection invalidation;
8. commits all effects together.

An unchanged normalized slug records no alias. Category creation and creation of a new locale translation also consult the same historical reservation owner, preventing an alternate write path from reclaiming an old route.

PostgreSQL uses transaction advisory locks over the normalized tenant/locale/slug key. SQLite relies on its serialized write transaction, with the same owner checks and database triggers.

## Resolution

`ForumCategoryRouteService::resolve` now combines current translation rows and immutable aliases in one bounded candidate set. The shared locale order remains:

1. requested locale;
2. explicit fallback locale;
3. platform fallback locale `en`;
4. one unambiguous first-available category identity.

This ordering is applied to both current and historical candidates. Therefore an exact-locale old route cannot be shadowed by a fallback-locale current category that happens to use the same readable slug.

A current exact route returns `CANONICAL` with no alias identity. A historical route returns `REDIRECT`, the recomputed canonical descriptor and its immutable `alias_id`.

Archived categories remain hidden. An alias that belongs to an archived category returns `FORUM_CATEGORY_ROUTE_NOT_FOUND`; this slice does not define category tombstones or public `410 Gone` behavior.

## Authorization boundary

Alias ownership is not visibility authorization. The owner does not evaluate category audience inheritance, channel visibility, module enablement or SEO publication eligibility. Future GraphQL/native transports must perform the exact category visibility recheck before exposing either a canonical descriptor or redirect.

## Compatibility

This slice does not change `UpdateCategoryInput`, `CategoryResponse`, GraphQL, REST, admin UI, storefront routes, topic routes, hierarchy behavior, projection event schemas, SEO metadata, hreflang or schema.org composition.

Hierarchy still does not participate in the route. Moving a category creates no redirect because the flat canonical path does not change.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, PostgreSQL scenarios, migrations, workflows, HTTP scenarios, browser scenarios or CI were executed while preparing this slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-category-slug-alias-owner.mjs
cargo test -p rustok-forum services::category_route::tests -- --nocapture
cargo test -p rustok-forum --test category_slug_alias_contract -- --nocapture
cargo test -p rustok-forum --test category_slug_alias_sqlite -- --nocapture
cargo test -p rustok-forum --test category_route_identity_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Remaining FORUM-24 scope after FORUM-24M

- visibility-safe category route GraphQL and native transport;
- Rust storefront category route mount and category-link cutover;
- a separate category archive tombstone disclosure policy only if product semantics require it;
- canonical and hreflang document policy;
- Forum-specific SEO composition and matching schema.org semantics;
- Next storefront parity;
- maintainer SQLite, PostgreSQL, HTTP and browser evidence.

The canonical implementation plan remains the single roadmap. Its FORUM-24 ledger entry is not updated by this slice because the connected complete-file writer cannot safely retrieve and replace the full plan losslessly; this document records only the stable FORUM-24M contract and does not create a second backlog.
