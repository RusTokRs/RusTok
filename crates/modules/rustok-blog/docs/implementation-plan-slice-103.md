# rustok-blog implementation plan — slice 103 continuation

Status: source_verified_no_compile / source-ready / maintainer execution pending.
Compile policy: not_run_by_request. Runtime status: not_run.
Tests, Cargo, formatting, and CI were not run by the implementation agent; no runtime result is claimed.

## Retained source contract

- file
- TaxonomyOwnerReader
- load_term_names_strict_for_module(
- resolve_name_for_locale_chain(&locale_chain)
- .load_scoped_terms(
- Status: `tag_canonical_read_search_projection_source_ready_maintainer_execution_pending`.
- `blog_tag_read_source = blog_post_tags_plus_taxonomy`
- `blog_search_tag_projection = relation_taxonomy_source_ready_maintainer_execution_pending`
- This slice does **not** change `TagService::update_tag` or `TagService::delete_tag` transaction semantics.
- metadata-only rows
- Implement `tag_mutation_atomic_reindex` as slice 104
- No tests, Cargo commands, Node verifiers
- the tag_canonical_projection track token
- `global_tag_search_invalidation = source_complete_maintainer_execution_pending`
