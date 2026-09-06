# FORUM-24D topic slug rename owner

FORUM-24D adds one explicit owner command for changing the slug of an existing
localized topic route. The command is separate from the general topic update DTO
so existing REST, GraphQL and internal update clients do not silently acquire a
new route mutation.

## Owner contract

`TopicService::rename_slug` accepts `RenameForumTopicSlugInput { locale, slug }`.
It uses the same `forum_topics:update` ownership authorization as ordinary topic
updates. The locale must already have a topic translation with a non-empty slug.
An empty normalized slug is rejected; this command does not remove routes or
create translations.

Inside one transaction the owner:

1. locks the exact tenant/topic/locale route;
2. normalizes the requested slug;
3. records the old path as an immutable self-target redirect with reason
   `Topic slug changed`;
4. updates the translation slug and topic timestamp;
5. publishes the existing topic projection invalidation; and
6. commits the alias and content change together.

An exact normalized replay returns `changed = false` and does not add another
alias. Alias ownership or payload drift fails closed through the existing route
ledger uniqueness contract.

## Resolution lifecycle

The short topic identity never changes. While the topic is active, an old slug
redirects to the current canonical descriptor. A later merge follows the bounded
canonical merge chain and selects the exact locale, platform fallback locale, or
first available target locale. If the same topic is deleted, its self-target
rename aliases resolve as `gone`; the current slug receives the ordinary
FORUM-24C tombstone.

## Boundaries

This slice does not add REST or GraphQL fields, admin UI, new-locale creation,
slug removal, historical alias backfill, category routes, storefront mounting,
hreflang, schema.org selection, SEO publication policy, or retained runtime
evidence.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-slug-rename-owner.mjs
cargo test -p rustok-forum --test topic_slug_rename_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

The implementation agent did not run these commands.
