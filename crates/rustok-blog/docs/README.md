# `rustok-blog` Documentation

`rustok-blog` is the domain module for publication, Blog category, and comment
scenarios. The module owns its persistence and uses shared platform contracts
only across explicit boundaries.

All Blog comment lifecycle operations consume the public `CommentsThreadPort`
with typed actor, locale, deadline, idempotency where required, and error
semantics. Comments lifecycle events are consumed by Blog's durable idempotent
reply-count projection, which publishes `BlogPostUpdated` in the same projection
transaction.

## Planning cursor

Use [Current Implementation Cursor](./implementation-plan-current.md) for the
live Blog source status and next autonomous cursor. The long
[Implementation Plan](./implementation-plan.md) is the historical baseline and
embedded implementation log; its inline current-state and next-results sections
predate the later standalone continuation slices and must not be treated as the
live cursor without the current-cursor document.

## Purpose

- publish the canonical Blog runtime contract for posts, categories, and tag relations;
- keep Blog-owned transport surfaces, domain services, and UI packages inside the module;
- evolve the Blog as a channel-aware and taxonomy-aware domain without shared storage;
- expose distinct `blog_posts:*` and `blog_categories:*` authority resources.

## Scope

- `PostService`, `CommentService`, `CategoryService`, `TagService`, and the Blog state machine;
- Blog-owned storage for posts, translations, categories, and typed relations;
- GraphQL, REST, Leptos admin, and storefront transport surfaces;
- REST handlers consume narrow `BlogHttpRuntime` state with explicit DB/event bus handles; `controllers::axum_router` builds that state from `HostRuntimeContext`;
- category REST CRUD under `/api/blog/categories` requires `blog_categories:*`;
- `CategoryService::new(db, event_bus)` is the only category service constructor;
- category update/delete and tenant Blog-scope reindex publication share one transaction;
- moderation REST surface `POST /api/blog/comments/{id}/moderate` uses `blog_posts:manage`;
- channel visibility for publications and integration with `rustok-channel`;
- shared taxonomy dictionary reuse via `blog_post_tags`, without transferring attachment ownership;
- observability via `rustok-telemetry` read-path metrics and instrumented service methods.

## Multilingual storage contract

Blog follows the platform language-agnostic storage model:

- `blog_posts` owns identity, lifecycle, relations, counters, publication state,
  and the canonical route key;
- `blog_posts.slug` is an explicitly locale-neutral canonical route identifier.
  It is stable across requested locales and must not contain translated display
  copy;
- localized post title, excerpt, body, and SEO copy belong to
  `blog_post_translations`;
- localized category name, slug, and description belong to
  `blog_category_translations`;
- `blog_categories.revision` is the positive owner resource revision for a
  category, while `blog_category_translations.revision` is the positive exact
  locale revision for its localized copy;
- `blog_translation_changes` is Blog's append-only, content-free owner change
  journal. Its `category` rows provide the opaque cursor used by Translation
  inventory repair and progress framing;
- post and category translation locale columns use the platform-safe
  `VARCHAR(32)` contract after
  `m20260721_000005_expand_blog_locale_storage_columns`;
- tenant default/effective locale controls resolution only and does not own any
  localized Blog field.

Changing the canonical post route key is a language-agnostic identity operation.
A localized alternative URL must be modeled as an explicit alias/projection; it
must not silently redefine ownership of `blog_posts.slug`.

## Translation target boundary

`BlogCategoryTranslationTargetProvider` registers the exact `blog/category`
owner target through the server host registry. It supports bounded discovery,
exact resource reads, patch validation, resource/source/target revision CAS,
durable owner-operation receipt replay, exact progress, and append-only change
cursors.

- The exposed fields are public `name` (AI-exportable plain text), public
  review-only `slug` (not AI-exportable), and optional public `description`
  (AI-exportable plain text). Their maximum sizes are 255, 255, and 1000
  characters respectively.
- Runtime fallback never counts as an exact target value. Source and target
  rows are always addressed with canonical `TenantLocale` values.
- Apply delegates to `CategoryService::apply_exact_translation_in_tx`; it
  validates the localized slug, performs owner CAS, records a `category`
  change, and publishes the Blog Search reindex request. The provider completes
  or replays the shared owner-operation receipt in that same transaction.
- The host constructs this provider with the durable `OutboxTransport` because
  target registration happens before the general event runtime is available.
  The adapter never reads or writes Blog storage from `rustok-translation`.
- Taxonomy-owned Blog tags and all Blog post fields remain outside this pilot.
  Post title/body/SEO onboarding requires the separate editorial richtext
  revision and segment-materialization wave.

## Permission boundary

`Resource::BlogCategories` serializes as `blog_categories`. Built-in roles,
public-read authority, OAuth content scopes, module permission registration,
HTTP preflight, and owner services use this resource. Catalog `categories:*`
and post `blog_posts:*` permissions do not grant Blog category access.

## Integration

- uses `rustok-taxonomy` as a shared vocabulary for tag identity;
- uses `rustok-comments` as a comment runtime contract;
- uses `rustok-profiles` for author presentation;
- the server GraphQL host binds `ProfileSummaryLoader` to the current anonymous,
  authenticated-human, or trusted-service audience before Blog resolves
  `authorProfile`; restricted, hidden, blocked, missing, and cross-tenant profile
  summaries are omitted before localized profile/tag loading, without per-author
  privacy reads;
- standalone/custom GraphQL hosts must attach the same audience-bound loader;
  `ProfileSummaryLoader::new` is anonymous and fail-closed by default;
- uses `rustok-channel` for module-level and publication-level public visibility;
- uses `rustok-telemetry` for read/write observability;
- `rustok-blog/admin` embeds the owner-side post SEO panel through the shared `rustok-seo` capability contract.

## Contract Tests

Tests in `tests/contract_surface.rs`, `tests/module.rs`, and `tests/integration.rs` cover:

- **Post lifecycle**: create → draft → publish → archive → restore
- **Locale resolution**: normalize → requested → en → first available
- **Channel visibility**: typed `blog_post_channel_visibility` allowlists, empty = global
- **Taxonomy sync**: Blog tags ↔ `rustok-taxonomy` vocabulary
- **RBAC enforcement**: distinct post/category resources and denied cross-resource grants
- **Category invariants**: mandatory event bus, tenant parent/translation scope, slug validation, pagination cap
- **Category Translation target**: migration `up/down/up`, exact-locale CAS,
  idempotent replay, cursor evidence, exact progress, and transactional Search
  reindex outbox publication
- **GraphQL read paths**: public vs authenticated channel gating and request-scoped profile author-card privacy
- **Events**: Blog post lifecycle and category-triggered Search reindex
- **Comments**: thread, locale resolution, status transitions, RBAC
- **State machine**: BlogPost and CommentStatus transitions

## Verification

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- `node scripts/verify/verify-blog-category-search-reindex.mjs`
- `cargo test -p rustok-blog translation_target --lib`
- targeted tests for lifecycle, category authority, outbox rollback, Search refresh,
  channel visibility, request-scoped author-summary filtering, and public/admin read
  paths

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
