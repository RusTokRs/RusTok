# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_slice_103`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` remains the long historical baseline and embedded implementation log, but its inline `Current state`, completed-slice list, and `Next results` stop before the later continuation series and must not be used as the live cursor without this file.

The continuation series is authoritative for source work after the historical baseline. Slice 101 establishes this current-cursor boundary. Slice 103 is the latest production/source behavior slice.

## Re-audit basis

The source continuation through slice 103 retains the following planning corrections and independent Blog source results:

- the remote Comments transport is no longer an unimplemented source item;
- the cached public Comments snapshot is no longer merely planned;
- the storefront comment-form fallback is not an implementation target because the active storefront has no public Comments write surface;
- Blog category Translation PostgreSQL migration/concurrent-CAS/change-cursor evidence source is already retained and waits for maintainer execution;
- Blog tag list pagination is owner-bounded and overflow-safe;
- Blog tag reads and Search projection now use `blog_post_tags + rustok-taxonomy` as the canonical source rather than `blog_posts.metadata.tags`.

None of these source states promote runtime evidence.

## Current source tracks

### Comments remote transport and host composition

The remote Comments source implementation exists. The retained continuation chain covers the typed transport boundary, `TcpJsonCommentsTransport`, TCP server/listener and host selection, user delegation and authorization, key/keyring lifecycle, schedule persistence/audit, canonical event admission, source retry/dead-letter/recovery ownership, restart/ambiguous-commit evidence sources, and the canonical `rustok-outbox` relay evidence source.

Canonical interpretation:

`remote_comments_transport = source_implemented_maintainer_execution_pending`

Do **not** interpret historical `remote transport remains pending` text as a request to implement another transport, listener, retry lane, or relay.

The latest audit/relay source boundary is slice 97:

`canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`.

Source-row and immutable recovery-audit retention remain intentionally gated: do not advance that source work before retained maintainer execution of slices 95–97.

### Blog category Translation target

The `blog/category` target production source is present. Slice 98 adds the isolated PostgreSQL evidence source for:

- real migration `up -> down -> up`;
- concurrent same-revision CAS with one winner and one conflict;
- change-cursor recovery across provider/owner reconstruction and delete lifecycle.

Canonical interpretation:

`category_translation_postgres = source_ready_maintainer_execution_pending`

Do **not** reopen PostgreSQL migration, concurrent CAS, or ordinary cursor-recovery source scaffolding. After maintainer execution, record the result in the active Translation readiness view. Broader production enablement remains a separate Translation-owner decision.

### Storefront Comments fallback

Slice 99 implements one Blog-owned cached public Comments snapshot policy shared by GraphQL and native SSR. Successful approved public reads refresh the bounded cache best-effort. Only `ExternalService` and `Timeout` may consume an exact valid stale snapshot; stale hits preserve `UNAVAILABLE` / `TIMEOUT` and expose `cachedSnapshot=true`.

Canonical interpretation:

`cached_public_comments_snapshot = source_ready_maintainer_execution_pending`

Slice 100 re-audits the storefront write surface and proves that the active package is read-only. There is no public comment form, textarea, submit handler, GraphQL storefront mutation, or native create-comment server function.

Canonical interpretation:

`comment_form_fallback = not_applicable_no_storefront_write_surface`

The legacy `hide_comment_form` token remains compatibility vocabulary in the existing FBA registries; it is not authorization to invent a new storefront write surface.

### Blog tag list pagination

A fresh source audit after slice 101 found that `TagService::list_tags` accepted an arbitrarily large `per_page` and used unchecked page-offset multiplication, unlike the already bounded Blog category list contract.

Slice 102 makes the owner service authoritative for the response bound and arithmetic safety:

`tag_list_pagination = source_ready_maintainer_execution_pending`

The retained contract is:

- `1 <= per_page <= 100` in the owner service;
- matching Utoipa parameter metadata in `ListTagsFilter`;
- saturating `u64` page-offset arithmetic plus checked `usize` conversion;
- unchanged visibility, usage-count ordering, locale resolution and total-count semantics.

This does **not** claim database-side pagination. Eligible tag terms, usage counts and translations are still materialized before the existing usage-count sort/page slice.

### Blog canonical tag read/Search projection

Slice 103 resolves the cross-owner source question identified by slice 102:

`tag_canonical_projection = source_ready_maintainer_execution_pending`

The accepted ownership boundary is now source-locked:

- `rustok-taxonomy` owns the shared tag dictionary;
- `rustok-blog` owns post attachments in `blog_post_tags`;
- `blog_posts.metadata.tags` remains compatibility metadata, but is not a canonical Blog read or Search projection source.

Blog reads seed an explicit empty tag vector for each requested post ID. Therefore an empty relation set no longer becomes a missing map entry that can resurrect stale `metadata.tags` through the existing response fallback path.

Blog Search now requires `blog_post_tags`, `taxonomy_terms`, and `taxonomy_term_translations` and resolves attached names through document locale -> `PLATFORM_FALLBACK_LOCALE` -> canonical key. Taxonomy joins are tenant-constrained and tag aggregation remains distinct/deterministic. The retained PostgreSQL harness deliberately keeps stale metadata while expecting projection from relation/Taxonomy rows.

Runtime promotion must audit deployed data for metadata-only legacy rows. If such rows exist, backfill owner relations before rollout. Slice 103 does not claim that the audit or a backfill was executed.

Mutation semantics intentionally remain separate:

`tag_mutation_atomic_reindex = next_source_gap`

Slice 103 does **not** change `TagService::update_tag/delete_tag`. The next source slice must define a Taxonomy-owned supplied-transaction mutation API, commit the dictionary mutation and Blog-scope reindex together, and remove Blog's redundant manual pre-delete relation cleanup in favor of the declared FK cascade.

## Remaining execution-owned results

The concrete retained execution results remain maintainer-owned:

1. Execute the retained Comments transport/composition, PostgreSQL, restart/ambiguity, canonical relay, and cached-snapshot evidence at an exact revision.
2. Execute slices 95–97 before defining terminal Blog source-row and immutable recovery-audit retention.
3. Execute slice 98 PostgreSQL evidence before advancing the Blog category Translation readiness result.
4. Execute slice 102 tag pagination source/unit evidence before promoting runtime validation.
5. Execute slice 103 Blog read/Search canonical tag projection evidence and audit deployed data for metadata-only legacy rows before runtime promotion.
6. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit evidence already retained by the historical plan.
7. Execute the Blog article richtext cutover/backfill/browser evidence already retained by the historical plan.

A future source implementation must follow the explicit current cursor. It must not manufacture work by reopening a source-complete or not-applicable track.

## Superseded historical cursor phrases

The following phrases may remain in the historical baseline as records of earlier state, but they are superseded as live instructions:

- `remote transport remains pending`;
- `cached snapshot and comment-form fallback remain planned`;
- `PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement`;
- `then implement the remote network transport`.

The continuation slice files and machine evidence remain the source of detailed ownership/non-claim history. This file defines the current planning cursor.

## Validation boundary

No tests, Cargo commands, Node verifiers, PostgreSQL/Redis/TCP scenarios, browser targets, formatting, Clippy, builds, workflows, CI, HTTP execution, runtime validation, or production validation were executed by the implementation agent while producing slices 101–103.

## Next cursor

Implement slice 104: `tag_mutation_atomic_reindex`.

Use a Taxonomy-owned supplied-transaction update/delete path, then commit Blog tag mutation and `ReindexRequested { target_type: "blog", target_id: None }` in the same transaction. Remove the manual pre-delete `blog_post_tags` cleanup and rely on the declared FK cascade. Preserve existing Blog `tags:*` authorization semantics and do not promote runtime evidence.
