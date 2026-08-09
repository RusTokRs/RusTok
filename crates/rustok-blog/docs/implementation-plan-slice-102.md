# rustok-blog implementation plan — slice 102 continuation

Status: `tag_list_pagination_owner_bound_source_ready_maintainer_execution_pending`.

This slice continues the canonical current cursor established by slice 101.

## Fresh audit

After the plan actualization, a fresh audit moved outside the execution-gated Comments, category Translation, and storefront fallback tracks and rechecked Blog-owned posts/categories/tags source.

`CategoryService` and `ListCategoriesFilter` already retain an explicit `1..=100` pagination contract. The public Blog tag list did not:

- `ListTagsFilter.per_page` documented no upper bound;
- `TagService::list_tags` used `filter.per_page.max(1)`, allowing arbitrarily large requested page sizes;
- the in-memory offset used `(page - 1) * per_page`, so an extreme `u64` page could overflow arithmetic before `skip`.

This is an owner-service contract gap independent of the execution-gated tracks in the current cursor.

## Slice 102 — owner-bounded tag list pagination

`TagService` now owns the effective tag page-size limit:

```text
1 <= per_page <= 100
```

The service does not trust transport validation. Every direct caller is bounded by `bounded_tag_page_size`, including programmatic callers that construct `ListTagsFilter` without serde/OpenAPI.

`ListTagsFilter` now documents the same contract through Utoipa parameter metadata:

```text
page >= 1
1 <= per_page <= 100
```

This is descriptive transport/schema metadata; the service clamp remains authoritative.

## Overflow-safe page offset

The tag list keeps the existing page normalization (`page >= 1`) but computes the in-memory skip with saturating arithmetic:

```text
page.saturating_sub(1).saturating_mul(per_page)
```

The result is converted to `usize` with a checked conversion and falls back to `usize::MAX` if the platform cannot represent the offset. An extreme page therefore produces an empty page instead of an arithmetic panic/wrap.

## Preserved list semantics

The slice intentionally preserves:

- Blog module-owned tags plus attached global tags visibility;
- usage-count descending order;
- canonical-key tie breaking;
- locale resolution behavior;
- total count before page slicing;
- Taxonomy ownership of tag terms/translations;
- `Resource::Tags` authorization.

It does **not** claim database-side pagination. `list_visible_terms`, usage counts, and translation loading still materialize the eligible tag inventory before sorting/page slicing. The change bounds the returned page and makes offset arithmetic safe; a future DB-side optimization must preserve the current global/module visibility and usage-count ordering exactly.

## Source harness

Two pure unit targets are retained in `services::tag::pagination_tests`:

- `tag_page_size_is_bounded_by_owner_service`;
- `tag_page_offset_saturates_without_arithmetic_overflow`.

They cover zero/default/oversized page sizes plus ordinary and extreme page offsets. They were not executed by the implementation agent.

Machine evidence:

`crates/rustok-blog/contracts/evidence/blog-tag-pagination-source.json`

Fail-closed source guard:

`scripts/verify/verify-blog-tag-pagination-source.mjs`

Focused negative fixture:

`scripts/verify/verify-blog-tag-pagination-source.test.mjs`

## Separate mutation/projection audit

The fresh tag audit also found a distinct ownership question that is deliberately not folded into this pagination fix:

- post reads resolve current tag names through `blog_post_tags -> rustok-taxonomy`;
- Blog Search projection currently derives tag text from `blog_posts.metadata.tags`;
- `TagService::update_tag` changes the module-owned Taxonomy term;
- `TagService::delete_tag` removes Blog relations and then deletes the Taxonomy term;
- neither mutation currently defines how the denormalized Search tag text is refreshed.

That is a separate cross-owner consistency problem involving Blog, Taxonomy, and Search. Solving it requires an explicit canonical source/projection decision; this slice does not copy Taxonomy-private state into Search or add direct cross-module SQL.

## Validation boundary

No tests, Cargo commands, Node verifiers, formatting, builds, PostgreSQL scenarios, browser/HTTP requests, workflows, CI, `git diff --check`, or runtime/production validation were executed.

## Next cursor

Re-audit Blog tag mutation/projection consistency as a separate ownership slice. Determine the canonical source for Search-visible tag text and the atomic/reindex behavior required for `TagService::update_tag/delete_tag` before changing production mutation semantics.
