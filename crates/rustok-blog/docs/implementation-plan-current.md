# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_taxonomy_cat_12`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` and the standalone
`implementation-plan-slice-*.md` files are historical implementation records.
They remain useful for provenance, but statements in them about a live Blog
Category Translation provider, Blog Category translation donor tables, or a
pending slice-98 PostgreSQL execution gate are superseded by this file.

## Current Category ownership

The Blog Category migration to canonical Taxonomy is source-complete through
TAXONOMY-CAT-12.

Canonical interpretation:

`blog_category_taxonomy_cutover = source_complete_through_cat12`

The retained ownership boundary is:

- `rustok-taxonomy` owns canonical Blog Category localized copy, route history,
  and the Taxonomy Category projection used by Blog public/owner reads;
- Blog Category create/update commands synchronize canonical Taxonomy state in
  the owner transaction;
- Blog public `get`/`list`, post `category_name` projection, and mutation
  responses read canonical Taxonomy state rather than the retired Blog
  translation mirror;
- Category hierarchy mutations synchronize the Taxonomy hierarchy in the same
  Blog owner transaction;
- Category delete delegates canonical lifecycle cleanup to Taxonomy;
- `blog_categories` remains Blog-owned for module membership, settings, owner
  revision and local command invariants. CAT-12 does **not** transfer or drop
  that table or the typed Taxonomy binding.

### Completed Category continuation

The continuation after the historical Blog cursor is:

- CAT-1..CAT-4: establish typed Taxonomy ownership/binding and canonical read
  seams;
- CAT-5..CAT-6: synchronize Category hierarchy/structure to Taxonomy;
- CAT-7: return Category update responses from canonical Taxonomy;
- CAT-8: retire `BlogCategoryTranslationTargetProvider` and host registration;
- CAT-9: retire writes to the Blog Translation change journal;
- CAT-10: retire live `blog_category_translation` mirror reads/writes and the
  compatibility bridge from Category commands;
- CAT-11: append irreversible migration
  `m20260828_000021_retire_blog_category_legacy_storage` after the historical
  Taxonomy backfill, fail closed unless same-ID Taxonomy ownership is present,
  then drop `blog_category_translations` and `blog_translation_changes`;
- CAT-12: remove the inert Translation bridge module and unregistered change
  entity, while retaining only the crate-private donor translation entity
  needed by the historical `000020` upgrade backfill.

Focused exact-head contracts for the completed continuation cover canonical
commands, mutation responses, reads, post category-name projection, hierarchy,
delete lifecycle, donor-storage retirement and `rustok-blog --lib` compilation
with warnings denied.

## Superseded Category Translation pilot

Slice 98 is a historical source record for the former `blog/category`
Translation-target pilot. Its proposed provider, PostgreSQL harness, Blog change
journal and execution evidence are **not** a live readiness gate anymore.

Canonical interpretation:

`blog_category_translation_provider = retired`

`blog_category_translation_postgres_evidence = superseded_by_taxonomy_cutover`

Do not recreate or execute the retired Blog provider/harness merely to satisfy
slice-98 language. The production provider source, provider tests, change
writer, change entity, donor journal and donor translation storage have been
retired in later bounded CAT slices. Historical migration files and historical
slice documents remain immutable upgrade/provenance records.

Any Translation-control-plane onboarding for Blog Categories must now target the
canonical Taxonomy owner contract. It must not restore direct Blog Category
localized storage or a second `blog/category` provider.

## Other retained Blog source tracks

The Category migration does not reopen unrelated source-complete tracks from the
previous cursor. Their latest retained source states remain:

- `remote_comments_transport = source_implemented_maintainer_execution_pending`;
- `canonical_outbox_relay_postgres_evidence_source_ready_maintainer_execution_pending`;
- `cached_public_comments_snapshot = source_ready_maintainer_execution_pending`;
- `comment_form_fallback = not_applicable_no_storefront_write_surface`;
- `tag_list_pagination = source_ready_maintainer_execution_pending`;
- `tag_canonical_projection = source_ready_maintainer_execution_pending`;
- `tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`;
- `post_category_name_projection = source_complete_canonical_taxonomy_read`.

For tags, Taxonomy remains the shared dictionary owner and Blog retains
`blog_post_tags` attachment ownership. For Comments, the execution-owned
transport/restart/relay evidence remains separate from Category Taxonomy work.

## Remaining execution-owned results

The retained maintainer/runtime evidence backlog is now limited to tracks whose
source still exists and whose result has not been superseded:

1. Execute the retained Comments transport/composition, restart/ambiguity,
   canonical relay and cached-snapshot evidence at an exact revision.
2. Execute the retained tag pagination, canonical tag projection and tag
   mutation/outbox rollback/delete-cascade evidence before runtime promotion.
3. Audit deployed data for metadata-only legacy tag rows before canonical tag
   projection rollout; backfill owner relations if such rows exist.
4. Execute category CRUD/Search refresh/canonical navigation/mounted rate-limit
   evidence that remains applicable to the current Taxonomy-backed Category
   implementation.
5. Execute the Blog article richtext cutover/backfill/browser evidence already
   retained by the historical plan.

There is **no** remaining execution item for the retired Blog Category
Translation provider or its deleted PostgreSQL harness.

## Documentation follow-up

Blog-owned live documentation is expected to describe the post-CAT-12 boundary.
Cross-cutting Translation/database overview documents may contain historical
provider-era wording until their own owner-scoped cleanup slice is merged; such
wording must not override this Blog cursor or current production source.

## Next cursor

The next bounded source task is a cross-cutting documentation actualization:
remove live claims that Blog still registers `BlogCategoryTranslationTargetProvider`
or owns `blog_category_translations` / `blog_translation_changes`, while
preserving historical migrations and slice-98 provenance.

After that, continue only from a fresh repository audit that identifies a new
independent source gap. Do not manufacture work by reopening CAT-1..CAT-12 or
by recreating the retired Blog Category Translation provider.
