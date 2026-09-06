# Blog implementation plan — slice 103

Status: `tag_canonical_read_search_projection_source_ready_maintainer_execution_pending`.

## Fresh ownership audit

Slice 102 left one explicit ownership question: post reads resolve tags through `blog_post_tags -> rustok-taxonomy`, while Search projected `blog_posts.metadata.tags`.

The fresh audit resolves the canonical source before changing tag mutation semantics:

- `rustok-taxonomy` owns the shared tag dictionary;
- `rustok-blog` owns post-to-tag attachments in `blog_post_tags`;
- `blog_posts.metadata.tags` is retained only as compatibility metadata written by existing post paths and is not authoritative for reads or Search projection.

This follows the accepted Taxonomy ADR: dictionary ownership is shared, domain attachments remain module-owned.

## Source change

### Blog reads

`load_post_tags_map` now creates an explicit empty tag vector for every requested post ID before loading relations. Therefore a post with zero attached relations resolves to an empty tag list instead of producing a missing map entry that could fall back to stale `metadata.tags` in `PostService`.

Canonical interpretation:

`blog_tag_read_source = blog_post_tags_plus_taxonomy`

### Search projection

`BlogSearchProjector` now requires `blog_post_tags`, `taxonomy_terms`, and `taxonomy_term_translations` in addition to the existing Blog projection tables.

Search tag names are resolved from attached term IDs with this bounded fallback chain:

1. taxonomy translation for the Blog document locale;
2. taxonomy translation for `PLATFORM_FALLBACK_LOCALE`;
3. taxonomy term `canonical_key`.

All Taxonomy joins are constrained by the Blog post tenant. Tag aggregation remains distinct and deterministic. The previous `jsonb_array_elements_text(p.metadata -> 'tags')` source is removed.

The retained PostgreSQL Search harness now stores deliberately stale metadata tags while creating canonical `blog_post_tags` + Taxonomy rows. It also contains a targeted reindex source case where the attached Taxonomy translation changes while metadata remains stale; the expected payload follows Taxonomy, not metadata.

Canonical interpretation:

`blog_search_tag_projection = relation_taxonomy_source_ready_maintainer_execution_pending`

## Deliberate non-scope

This slice does **not** change `TagService::update_tag` or `TagService::delete_tag` transaction semantics.

The audit found two mutation issues, but they remain the next coherent source slice:

- tag update/delete does not yet publish Blog Search reindex in the same transaction as the dictionary mutation;
- `delete_tag` still manually deletes `blog_post_tags` before `TaxonomyService::delete_term`, even though the declared `blog_post_tags.tag_id -> taxonomy_terms.id` FK already uses `ON DELETE CASCADE`.

Changing those mutations belongs in slice 104 after this canonical-source boundary is retained.

## Rollout caveat

This source change is intentionally fail-closed for legacy data. If deployed rows contain `metadata.tags` without equivalent `blog_post_tags` relations, strict canonical reads/Search will expose no tag attachment for those rows.

Before runtime promotion, maintainers must audit deployed data. If metadata-only rows exist, backfill them through an owner migration or another owner-authorized repair before rollout. Slice 103 does not claim that such legacy rows exist, that an audit was executed, or that a backfill was performed.

## Validation boundary

No tests, Cargo commands, Node verifiers, PostgreSQL scenarios, formatting, builds, Clippy, workflows, CI, HTTP execution, runtime validation, or production validation were executed by the implementation agent.

Source/harness/verifier changes are retained as executable evidence only.

## Next cursor

Implement `tag_mutation_atomic_reindex` as slice 104:

1. expose Taxonomy-owned update/delete operations that accept a supplied owner transaction while preserving Taxonomy validation, revision/CAS, translation-change evidence, and permission semantics;
2. make Blog tag update/delete perform the dictionary mutation and `ReindexRequested { target_type: "blog", target_id: None }` in the same transaction;
3. remove Blog's manual pre-delete relation cleanup and rely on the declared FK cascade;
4. retain rollback/source evidence without claiming runtime execution.
