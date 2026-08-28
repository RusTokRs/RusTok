# rustok-blog canonical implementation cursor

Status: `canonical_source_cursor_actualized_through_taxonomy_cat_17_docs`.

This document is the canonical **current** source cursor for `rustok-blog`.
`crates/rustok-blog/docs/implementation-plan.md` and the standalone
`implementation-plan-slice-*.md` files are historical implementation records.
They remain useful for provenance, but statements in them about a live Blog
Category Translation provider, Blog Category translation donor tables, or a
pending slice-98 PostgreSQL execution gate are superseded by this file. The
owner-scoped documentation cleanup that followed the source cutover is complete
through TAXONOMY-CAT-17.

## Current Category ownership

The Blog Category migration to canonical Taxonomy is source-complete through
TAXONOMY-CAT-12. CAT-13..CAT-17 actualize the owner-scoped planning, Translation,
registry, database-map, and long-form documentation around that completed source
boundary; they do not move the production cutover past CAT-12.

Canonical interpretation:

`blog_category_taxonomy_cutover = source_complete_through_cat12`

`blog_category_documentation_cursor = owner_scoped_actualized_through_cat17`

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
  needed by the historical `000020` upgrade backfill;
- CAT-13: actualize the canonical Blog planning cursor and active Blog README
  surfaces, retire orphaned provider-era PostgreSQL evidence/verifier sources,
  and guard the post-cutover source boundary;
- CAT-14: actualize cross-owner Taxonomy/Flex planning and the central database
  map while preserving the accepted no-duplicate-provider ownership ADR;
- CAT-15: actualize central/module Translation plans and the machine-readable
  Translation surface registry so `blog_categories` is `excluded` /
  `not_registered` and `taxonomy_terms` remains the canonical registered owner;
- CAT-16: actualize the central module registry and remove the obsolete
  Blog-specific Category Translation recovery/readiness gate;
- CAT-17: align the long-form Blog plan's live ownership summary and former
  Translation-pilot section with canonical Taxonomy ownership and add a focused
  exact-head guard against provider-era drift.

Focused exact-head contracts for the completed continuation cover canonical
commands, mutation responses, reads, post category-name projection, hierarchy,
delete lifecycle, donor-storage retirement, `rustok-blog --lib` compilation with
warnings denied, and the CAT-13..CAT-17 owner/documentation boundaries.

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

The owner-scoped Blog Category cleanup is complete through CAT-17. Active Blog,
Translation, Taxonomy/database-map, central module-registry, and long-form Blog
ownership surfaces now describe canonical Taxonomy ownership without treating a
second `blog/category` provider, donor tables, or provider PostgreSQL evidence as
live readiness contracts.

Historical migrations and standalone slice records remain provenance and may
name retired provider/storage concepts in historical context. Any future stale
live claim discovered outside these owner-scoped surfaces is a new independent
documentation gap and must be handled from a fresh `main` under the owning
module's boundary.

## Next cursor

There is no predeclared Blog Category Translation cleanup slice after CAT-17.
Continue only from a fresh repository audit that identifies a new independent
source, registry, or live-documentation gap. Do not manufacture work by
reopening CAT-1..CAT-17, by recreating the retired Blog Category Translation
provider, or by treating historical migration/slice provenance as a live
contract.
