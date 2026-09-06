# `rustok-blog` Documentation

`rustok-blog` is the Blog domain module for publications, Blog Category owner
state, tags, and comment integration. The module owns its persistence where the
Blog domain is authoritative and uses shared platform owners through explicit
contracts.

## Planning cursor

Use [Current Implementation Cursor](./implementation-plan-current.md) for the
live Blog source status and next autonomous cursor. The long
[Implementation Plan](./implementation-plan.md) and standalone slice files are
historical implementation records; they must not override the current cursor
when later bounded migrations retire an earlier design.

## Purpose

- publish the canonical Blog runtime contract for posts, Blog Category owner
  state, and tag relations;
- keep Blog-owned transport surfaces, domain services, and UI packages inside
  the module;
- evolve Blog as a channel-aware and taxonomy-aware domain without shared-table
  ownership drift;
- expose distinct `blog_posts:*` and `blog_categories:*` authority resources.

## Scope

- `PostService`, `CommentService`, `CategoryService`, `TagService`, and the Blog
  state machine;
- Blog-owned post storage, post translations, Blog Category membership/settings,
  and typed relations;
- GraphQL, REST, Leptos admin, and storefront transport surfaces;
- REST handlers consume narrow `BlogHttpRuntime` state constructed from
  `HostRuntimeContext`;
- category REST CRUD under `/api/blog/categories` requires
  `blog_categories:*`;
- `CategoryService::new(db, event_bus)` is the Category service constructor;
- category owner mutations and tenant Blog-scope Search reindex publication
  share one transaction;
- moderation REST uses `blog_posts:manage`;
- channel visibility for publications and integration with `rustok-channel`;
- shared Taxonomy dictionary reuse via `blog_post_tags`, without transferring
  attachment ownership;
- canonical Taxonomy ownership for Blog Category localized copy, routes and
  Category projection.

## Multilingual storage contract

Blog post localization remains owner-local:

- `blog_posts` owns identity, lifecycle, relations, counters, publication state,
  and the locale-neutral canonical route key;
- localized post title, excerpt, body and SEO copy belong to
  `blog_post_translations`;
- tenant locale policy controls resolution only and does not own localized Blog
  fields.

Blog Category localization no longer uses a live Blog translation mirror:

- `blog_categories` remains Blog-owned for module membership, settings, owner
  revision and local command invariants;
- canonical Category localized copy and route history are Taxonomy-owned;
- public/owner Category reads and Category mutation responses project canonical
  Taxonomy state;
- historical migration `000020` backfills donor Category copy into same-ID
  Taxonomy ownership;
- migration `000021` fails closed unless that ownership is present, then
  irreversibly drops `blog_category_translations` and
  `blog_translation_changes`;
- the donor Category translation entity remains crate-private only for the
  historical `000020` upgrade path. It is not a runtime source.

Changing the canonical post route key remains a language-agnostic identity
operation. Localized alternative URLs must be explicit aliases/projections.

## Category Translation / Taxonomy boundary

`BlogCategoryTranslationTargetProvider` is retired and is not registered by the
server host. The provider-era Blog change writer, change entity, PostgreSQL
harness and live donor tables have also been retired in later CAT slices.

Current Category rules:

- create/update commands synchronize canonical Taxonomy copy in the owner
  transaction;
- hierarchy commands synchronize canonical Taxonomy placement;
- delete delegates canonical Category lifecycle cleanup to Taxonomy;
- Blog reads do not fall back to retired donor translation storage;
- post `category_name` projection reads the canonical Taxonomy Category label;
- Translation-control-plane onboarding for Blog Category copy must use the
  canonical Taxonomy owner contract rather than recreating a `blog/category`
  provider.

Historical slice-98 and migration files remain provenance records. Their old
`source_ready_maintainer_execution_pending` language is superseded for this
provider because the provider/harness no longer exists.

## Permission boundary

`Resource::BlogCategories` serializes as `blog_categories`. Built-in roles,
public-read authority, OAuth content scopes, module permission registration,
HTTP preflight, and owner services use this resource. Catalog `categories:*`
and post `blog_posts:*` permissions do not grant Blog Category access.

## Integration

- uses `rustok-taxonomy` as the shared tag dictionary and canonical Blog
  Category projection owner;
- uses `rustok-comments` as the comment runtime contract;
- uses `rustok-profiles` for author presentation;
- uses `rustok-channel` for module-level and publication-level public
  visibility;
- uses `rustok-telemetry` for read/write observability;
- `rustok-blog/admin` embeds the owner-side post SEO panel through the shared
  `rustok-seo` capability contract.

## Contract tests

Current focused contracts cover, among other Blog behavior:

- post lifecycle and locale resolution;
- channel visibility;
- tag/Taxonomy dictionary ownership;
- RBAC enforcement for distinct post/category resources;
- Blog Category create/update/move/delete invariants;
- canonical Taxonomy Category reads and mutation responses;
- Category hierarchy synchronization;
- Category Translation provider retirement;
- donor mirror/journal write retirement;
- physical donor storage retirement after same-ID Taxonomy ownership checks;
- post Category-name projection from canonical Taxonomy state;
- GraphQL read paths, events, Comments, and state-machine behavior.

The retired Blog Category Translation provider PostgreSQL harness is not a live
verification target and must not be recreated merely to satisfy historical
slice text.

## Related documents

- [README crate](../README.md)
- [Current Implementation Cursor](./implementation-plan-current.md)
- [Historical Implementation Plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)

## FFA UI split

Leptos render adapters for admin and storefront live in `admin/src/ui/leptos.rs`
and `storefront/src/ui/leptos.rs`. Crate roots connect module layers and
re-export `BlogAdmin` / `BlogView`. Both packages expose a transport facade:
SSR/hydrate selects native `#[server]` functions and standalone CSR selects the
parallel GraphQL contract. Selected-transport failures are returned directly;
mutations are never retried through another protocol.
