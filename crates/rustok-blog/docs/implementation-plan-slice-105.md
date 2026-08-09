# rustok-blog implementation plan — slice 105

Status: `post_category_name_projection_source_ready_maintainer_execution_pending`.

## Why this slice exists

The broad source audit after slice 104 found a DTO/read parity gap outside the execution-gated Comments, Category Translation readiness, and tag tracks.

`PostResponse` and `PostSummary` both expose `category_name`, but all three owner read paths assigned `None` even when `blog_posts.category_id` referenced an existing localized Blog category:

- detail via `build_post_response`;
- authenticated/admin list via `list_posts_with_locale_fallback`;
- public/storefront list via `list_public_visible_with_locale_fallback`.

The Category translation tables and locale policy are already production source. This slice only projects that already-owned data into the existing post DTO field.

## Accepted source contract

`post_category_name_projection = source_ready_maintainer_execution_pending`

Canonical identity remains:

`category_identity_source = blog_posts.category_id`

Canonical localized label source is:

`category_name_source = blog_category_translations.name`

The owner projection is tenant-bound and uses the shared locale resolver:

1. requested locale;
2. caller-supplied tenant fallback locale when present;
3. `PLATFORM_FALLBACK_LOCALE`;
4. first available translation.

A post with no category or a category without any translation keeps `category_name = None`.

## Batch/read behavior

List paths collect the page's category IDs, deduplicate them, and issue one tenant-scoped category-translation query for the page. They do not call `CategoryService::get` once per post and do not introduce an N+1 category read.

Detail uses the same helper for its single optional category ID.

No post pagination, visibility, tag projection, author projection, Search SQL, or DTO schema changes are part of this slice.

## Source evidence

Production source:

- `crates/rustok-blog/src/services/post.rs`

DTO contract:

- `crates/rustok-blog/src/dto/post.rs`

Executable source harness:

- `crates/rustok-blog/tests/post_category_name_projection.rs`

The harness retains one localized category attached to a published post and requires the same translated `category_name` across:

- detail with requested/fallback locale resolution;
- authenticated list;
- public visible list.

Suggested maintainer command:

`cargo test -p rustok-blog --test post_category_name_projection`

Machine evidence:

- `crates/rustok-blog/contracts/evidence/blog-post-category-name-projection-source.json`

Fail-closed source guard:

- `scripts/verify/verify-blog-post-category-name-projection-source.mjs`

Focused guard self-test:

- `scripts/verify/verify-blog-post-category-name-projection-source.test.mjs`

## Explicit non-changes

This slice does **not**:

- change Category create/update/delete/Translation write semantics;
- promote slice 98 PostgreSQL Category Translation readiness;
- change Search Blog projector SQL;
- change GraphQL, HTTP, native, or storefront schemas;
- add a database migration;
- change post/category permissions;
- promote FFA or FBA evidence;
- claim runtime, database, browser, HTTP, Search, workflow, CI, or production execution.

## Result

The existing `category_name` field is now a real Blog owner projection rather than a permanent `None` placeholder on post reads.

No additional category-name projection scaffolding is required after this slice. The next autonomous source slice must return to a fresh broad Blog audit rather than reopening this read projection or the execution-gated Category Translation readiness track.
