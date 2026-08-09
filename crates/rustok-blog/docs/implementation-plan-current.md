# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_slice_105`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` remains the long historical baseline and embedded implementation log, but its inline `Current state`, completed-slice list, and `Next results` stop before the later continuation series and must not be used as the live cursor without this file.

The continuation series is authoritative for source work after the historical baseline. Slice 101 establishes this current-cursor boundary. Slice 105 is the latest production/source behavior slice.

## Re-audit basis

The source continuation through slice 105 retains the following planning corrections and independent Blog source results:

- the remote Comments transport is no longer an unimplemented source item;
- the cached public Comments snapshot is no longer merely planned;
- the storefront comment-form fallback is not an implementation target because the active storefront has no public Comments write surface;
- Blog category Translation PostgreSQL migration/concurrent-CAS/change-cursor evidence source is already retained and waits for maintainer execution;
- Blog tag list pagination is owner-bounded and overflow-safe;
- Blog tag reads and Search projection use `blog_post_tags + rustok-taxonomy` rather than `blog_posts.metadata.tags` as the canonical source;
- Blog tag update/delete retain Taxonomy mutation and Blog-scope Search reindex in one owner transaction;
- Blog post detail/authenticated-list/public-list reads now populate the existing localized `category_name` DTO field from Blog-owned category translations instead of returning a permanent `None` placeholder.

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

The accepted ownership boundary is source-locked:

- `rustok-taxonomy` owns the shared tag dictionary;
- `rustok-blog` owns post attachments in `blog_post_tags`;
- `blog_posts.metadata.tags` remains compatibility metadata, but is not a canonical Blog read or Search projection source.

Blog reads seed an explicit empty tag vector for each requested post ID, so an empty relation set cannot resurrect stale metadata tags. Blog Search requires `blog_post_tags`, `taxonomy_terms`, and `taxonomy_term_translations` and resolves attached names through document locale -> `PLATFORM_FALLBACK_LOCALE` -> canonical key.

Runtime promotion still must audit deployed data for metadata-only legacy rows. If such rows exist, backfill owner relations before rollout. No audit/backfill result is claimed.

### Blog tag mutation atomic reindex

Slice 104 closes the mutation consistency gap exposed by slice 103:

`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`

`rustok-taxonomy` exposes narrow module-term update/delete functions that accept a supplied `DatabaseTransaction`, tenant/term identity, term kind, module slug, and caller security context. The Taxonomy owner rechecks module scope and term kind and preserves Taxonomy update/read/delete permissions, localized slug uniqueness, translation revision CAS, term revision CAS, and translation-change evidence.

`TagService::update_tag` and `TagService::delete_tag` keep their existing Blog `tags:*` checks, then execute the Taxonomy mutation and:

`ReindexRequested { target_type: "blog", target_id: None }`

through `TransactionalEventBus::publish_root_in_tx` before committing the same transaction.

Canonical delete relation cleanup is:

`tag_delete_relation_cleanup = declared_fk_cascade`

The old manual `blog_post_tags` pre-delete is removed. The existing `blog_post_tags.tag_id -> taxonomy_terms.id ON DELETE CASCADE` relation owns cleanup atomically with the Taxonomy term delete.

The retained source harness covers successful rename + durable reindex, forced outbox failure rollback, and delete cascade + durable reindex. None of those cases were executed by the implementation agent.

The Blog tag source line is source-complete through slice 104. Do not add another tag mutation scaffolding slice without new evidence.

### Blog post category-name projection

The fresh broad audit after slice 104 found that the existing `PostResponse.category_name` and `PostSummary.category_name` fields were permanent `None` placeholders in Blog owner reads even when `blog_posts.category_id` referenced an existing localized Blog category.

Slice 105 closes that read parity gap:

`post_category_name_projection = source_ready_maintainer_execution_pending`

Canonical identity and localized label sources are:

- `blog_posts.category_id` for the category identity;
- `blog_category_translations.name` for the localized name.

Detail, authenticated list, and public visible list now project the existing field. List paths collect and deduplicate the current page's category IDs and use one tenant-scoped translation query for the page rather than calling category reads per post.

The shared locale resolver preserves requested locale -> caller-supplied tenant fallback -> platform fallback -> first available semantics. A post without a category, or a category with no translations, retains `category_name = None`.

This is a read projection only. It does not change Category create/update/delete or Translation write semantics, does not promote the slice 98 Category Translation PostgreSQL readiness result, does not alter Search SQL, and does not change GraphQL/HTTP/native DTO schemas.

The Blog post category-name projection line is source-complete through slice 105. Do not add another category-name scaffolding slice without new evidence.

## Remaining execution-owned results

The concrete retained execution results remain maintainer-owned:

1. Execute the retained Comments transport/composition, PostgreSQL, restart/ambiguity, canonical relay, and cached-snapshot evidence at an exact revision.
2. Execute slices 95–97 before defining terminal Blog source-row and immutable recovery-audit retention.
3. Execute slice 98 PostgreSQL evidence before advancing the Blog category Translation readiness result.
4. Execute slice 102 tag pagination source/unit evidence before promoting runtime validation.
5. Execute slice 103 Blog read/Search canonical tag projection evidence and audit deployed data for metadata-only legacy rows before runtime promotion.
6. Execute slice 104 tag mutation/outbox rollback/delete-cascade harness and then Search projection evidence for rename/delete behavior.
7. Execute slice 105 post category-name detail/authenticated-list/public-list source harness before promoting runtime validation.
8. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit evidence already retained by the historical plan.
9. Execute the Blog article richtext cutover/backfill/browser evidence already retained by the historical plan.

A future autonomous source slice must start from a fresh broad repository audit and identify a genuinely new independent source gap outside the execution-gated tracks above. It must not manufacture work by reopening a source-complete or not-applicable cursor.

## Superseded historical cursor phrases

The following phrases may remain in the historical baseline as records of earlier state, but they are superseded as live instructions:

- `remote transport remains pending`;
- `cached snapshot and comment-form fallback remain planned`;
- `PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement`;
- `then implement the remote network transport`.

The continuation slice files and machine evidence remain the source of detailed ownership/non-claim history. This file defines the current planning cursor.

## Validation boundary

No tests, Cargo commands, Node verifiers, SQLite/PostgreSQL/Redis/TCP scenarios, browser targets, formatting, Clippy, builds, workflows, CI, HTTP execution, Search execution, outbox relay execution, runtime validation, or production validation were executed by the implementation agent while producing slices 101–105.

## Next cursor

No independent production source gap is claimed after slice 105.

Continue only after a fresh broad Blog source audit finds another gap outside the execution-gated tracks above, or after maintainers provide execution results that unlock one of their explicit follow-ups.
