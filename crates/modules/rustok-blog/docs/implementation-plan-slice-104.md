# Blog implementation plan — slice 104

Status: `tag_mutation_atomic_reindex_source_ready_maintainer_execution_pending`.

## Cursor

Slice 103 made `blog_post_tags + rustok-taxonomy` authoritative for Blog tag reads and Search projection. It intentionally left one source gap: `TagService::update_tag/delete_tag` could mutate the Taxonomy dictionary without committing a Blog Search invalidation in the same transaction, and delete manually removed Blog relations before deleting the Taxonomy term even though the declared FK already cascades.

Slice 104 closes that source gap without changing the public `TagService::new(DatabaseConnection)` constructor or bypassing Taxonomy ownership.

## Taxonomy owner transaction boundary

`rustok-taxonomy` now exposes a narrow module-term mutation boundary:

- `update_module_term_in_tx`;
- `delete_module_term_in_tx`.

The caller must supply:

- a `DatabaseTransaction`;
- tenant ID and term ID;
- `TaxonomyTermKind`;
- the module slug;
- the caller `SecurityContext`.

The Taxonomy owner rechecks that the term is a module-scoped term for that exact module and kind. It retains Taxonomy permissions rather than relying only on Blog permissions:

- update requires Taxonomy `Update` and `Read`, matching the successful permission set of the previous standalone update/response path;
- delete requires Taxonomy `Delete`.

The owner boundary preserves localized slug uniqueness, translation revision CAS, term revision CAS, and `translation_changes` evidence inside the supplied transaction.

Canonical interpretation:

`taxonomy_module_term_mutation = owner_supplied_transaction_source_ready`

## Blog atomic update/delete

`TagService::update_tag` and `TagService::delete_tag` still perform their existing Blog `tags:*` ownership checks before the mutation.

They now open one Blog transaction and call the Taxonomy owner boundary inside it. Before commit they write one canonical root event using:

`TransactionalEventBus::publish_root_in_tx`

with:

`ReindexRequested { target_type: "blog", target_id: None }`.

Therefore the dictionary mutation, Taxonomy translation-change evidence, and Blog Search invalidation are committed or rolled back together.

Canonical interpretation:

`tag_mutation_atomic_reindex = source_ready_maintainer_execution_pending`

No optional event bus field or alternate `TagService` constructor was added.

## Delete cascade

The Blog migration already declares:

`blog_post_tags.tag_id -> taxonomy_terms.id ON DELETE CASCADE`.

The old `TagService::delete_tag` manually deleted `blog_post_tags` before calling Taxonomy delete. That pre-delete is removed. The Taxonomy term deletion now owns the transaction and the database FK removes Blog attachment rows atomically with it.

Canonical interpretation:

`tag_delete_relation_cleanup = declared_fk_cascade`

## Retained source harness

`crates/rustok-blog/tests/taxonomy_tags.rs` now includes executable-no-run cases for:

- successful tag rename + durable Blog-scope reindex in one committed path;
- forced `sys_events` unavailability causing tag rename/revision rollback;
- tag delete removing Taxonomy term and Blog relation through the declared FK cascade while retaining a durable reindex event.

The harness installs `SysEventsMigration` because slice 104 writes canonical outbox rows rather than relying on the test-only memory transport for the reindex signal.

These cases are source evidence only. They were not executed by the implementation agent.

## Non-scope

Slice 104 does not:

- change Search projection SQL introduced by slice 103;
- change Blog tag list pagination introduced by slice 102;
- change Taxonomy global-term mutation behavior;
- add a new event type;
- add a database migration;
- promote FFA/FBA or runtime readiness;
- claim that the legacy metadata-only data audit from slice 103 was executed.

## Validation boundary

No tests, Cargo commands, Node verifiers, SQLite/PostgreSQL scenarios, formatting, builds, Clippy, workflows, CI, HTTP execution, Search execution, outbox relay execution, runtime validation, or production validation were executed by the implementation agent.

## Next cursor

The tag source line is source-complete through slice 104:

- bounded list pagination: slice 102;
- canonical relation/Taxonomy read and Search source: slice 103;
- atomic dictionary mutation + Blog reindex: slice 104.

Do not add another tag mutation scaffolding slice without new evidence. Continue only from a fresh broad Blog source audit outside execution-gated tracks, or from maintainer execution results that unlock an explicit follow-up.
