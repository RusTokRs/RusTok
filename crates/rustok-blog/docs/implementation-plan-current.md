# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_slice_103`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` remains the long historical baseline and embedded implementation log, but its inline `Current state`, completed-slice list, and `Next results` stop before the later continuation series and must not be used as the live cursor without this file.

The continuation series is authoritative for source work after the historical baseline. Slice 101 establishes this current-cursor boundary. Slice 103 is the latest production/source behavior slice.

## Re-audit basis

The source continuation through slice 103 retains these planning corrections/results:

- remote Comments transport source exists; retained execution evidence remains maintainer-owned;
- cached public Comments snapshot source exists; runtime evidence remains pending;
- storefront comment-form fallback is not applicable because the active storefront has no public Comments write surface;
- Blog category Translation PostgreSQL evidence source exists and waits for maintainer execution;
- Blog tag list pagination is owner-bounded and overflow-safe;
- Blog tag read/Search authority is now `blog_post_tags + rustok-taxonomy`, not `blog_posts.metadata.tags`.

None of these source states promote runtime evidence.

## Current source tracks

### Comments remote transport and host composition

Canonical interpretation:

`remote_comments_transport = source_implemented_maintainer_execution_pending`

The latest audit/relay boundary is slice 97. Source-row and immutable recovery-audit retention remain intentionally gated: do not advance that source work before retained maintainer execution of slices 95–97.

Do **not** interpret historical `remote transport remains pending` text as a request to implement another transport, listener, retry lane, or relay.

### Blog category Translation target

Canonical interpretation:

`category_translation_postgres = source_ready_maintainer_execution_pending`

Slice 98 retains real migration `up -> down -> up`, concurrent same-revision CAS and change-cursor recovery source evidence. Do not reopen that source scaffolding before maintainer execution.

### Storefront Comments fallback

Canonical interpretations:

`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`

`comment_form_fallback = not_applicable_no_storefront_write_surface`

The legacy `hide_comment_form` token remains compatibility vocabulary only; it is not authorization to invent a storefront write surface.

### Blog tag list pagination

Slice 102 retains:

`tag_list_pagination = source_ready_maintainer_execution_pending`

The owner service enforces `1 <= per_page <= 100`; page-offset arithmetic is overflow-safe; visibility, usage-count ordering, locale resolution and total-count semantics are unchanged. This does **not** claim database-side pagination.

### Blog canonical tag read/Search projection

Slice 103 resolves the source-ownership question exposed by slice 102.

Canonical interpretation:

`tag_canonical_projection = source_ready_maintainer_execution_pending`

The retained authority is:

- `rustok-taxonomy` owns the shared dictionary;
- `rustok-blog` owns post attachments in `blog_post_tags`;
- `blog_posts.metadata.tags` remains compatibility metadata but is not a canonical read or Search projection source.

Blog reads seed an explicit empty tag vector for every requested post ID, so an empty relation set no longer resurrects stale metadata tags.

Search requires `blog_post_tags`, `taxonomy_terms`, and `taxonomy_term_translations` and resolves names using document locale -> `PLATFORM_FALLBACK_LOCALE` -> canonical key. The PostgreSQL source harness deliberately retains stale metadata while proving that projected tags follow attached Taxonomy rows.

Runtime promotion must include a deployed-data audit for metadata-only legacy rows. If such rows exist, backfill owner relations before rollout. No legacy-data audit or backfill result is claimed by slice 103.

Mutation atomicity remains deliberately separate:

`tag_mutation_atomic_reindex = next_source_gap`

Slice 103 does **not** change `TagService::update_tag/delete_tag`. The next source slice must make dictionary mutation and Blog-scope reindex atomic without violating Taxonomy dictionary ownership, and must remove the redundant manual pre-delete relation cleanup in favor of the declared FK cascade.

## Remaining execution-owned results

1. Execute retained Comments transport/composition, PostgreSQL, restart/ambiguity, canonical relay and cached-snapshot evidence at an exact revision.
2. Execute slices 95–97 before defining terminal Blog source-row and immutable recovery-audit retention.
3. Execute slice 98 PostgreSQL evidence before advancing Blog category Translation readiness.
4. Execute slice 102 tag-pagination evidence.
5. Execute slice 103 Blog/Search canonical tag projection evidence and audit deployed data for metadata-only legacy tags before runtime promotion.
6. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit evidence already retained by the historical plan.
7. Execute Blog article richtext cutover/backfill/browser evidence already retained by the historical plan.

## Superseded historical cursor phrases

The following historical phrases are not live instructions:

- `remote transport remains pending`;
- `cached snapshot and comment-form fallback remain planned`;
- `PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement`;
- `then implement the remote network transport`.

## Validation boundary

No tests, Cargo commands, Node verifiers, PostgreSQL/Redis/TCP scenarios, browser targets, formatting, Clippy, builds, workflows, CI, HTTP execution, runtime validation, or production validation were executed by the implementation agent while producing slices 101–103.

## Next cursor

Implement slice 104: `tag_mutation_atomic_reindex`.

Use a Taxonomy-owned supplied-transaction mutation API, then commit Blog tag update/delete and `ReindexRequested { target_type: "blog", target_id: None }` in the same transaction. Remove the manual pre-delete `blog_post_tags` cleanup and rely on the declared FK cascade. Preserve existing Blog `tags:*` authorization semantics and do not promote runtime evidence.
