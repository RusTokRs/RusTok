# Search owns projection storage; the module owns the documents and the refresh width

- Date: 2026-10-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform / search team, with every projecting module as source owner
- Extends: `2026-10-25-search-audience-declared-corpus.md`
- Supersedes: None
- Superseded by: None

## Context

`rustok-search` used to contain `forum_projector.rs`: the only writer of
`search_documents` for an external module. It hard-coded the Forum slug, the
Forum entity-type allowlist (`forum_category`, `forum_topic`, `forum_reply`), its
own staging table (`forum_search_projection_stage`) and the rule "a topic refresh
rebuilds the whole tenant".

The capability it implemented is not Forum-specific: every projecting module
(Blog, Pages, Docs, Forum, and the integration modules) publishes documents
through the same `SearchProjectionSource` trait from `rustok-core`, and every one
of them needs the same transactional scope replacement. Keeping one module's copy
made the other sources second-class, forced Search to know a module name, and
made a second writer for any new module the only way to add one.

The ownership question this raised is: where should the storage driver live? The
rows it writes are `search_documents`, a table owned by Search migrations and read
by the Search engine, the Search GraphQL API and the Search diagnostics. Moving
the SQL into a module crate would put Search storage under module ownership and
would force every module to reimplement the same transactional replacement.

## Decision

1. Search owns projection storage and the transaction that replaces a scope.
   There is exactly one storage driver, `SearchProjectionDriver`, and it is
   module-agnostic.
2. A module owns (a) which documents exist and what they contain, through
   `SearchProjectionSource::list_public_documents` / `load_public_entity`, and
   (b) how much a single-entity refresh has to replace, through
   `SearchProjectionSource::refresh_scope`.
3. The driver derives everything else from the source it was constructed with:
   it validates documents and deletes rows against `source.source_module()`, and
   it resolves refresh width through `source.refresh_scope(entity_type)`.
4. `SearchProjectionRefreshScope` is a closed enum (`Entity`, `Tenant`) with
   `Entity` as the default. A source that aggregates child documents into one
   scope returns `Tenant`; nothing in Search infers that from an entity-type name.
5. No driver code may contain a module slug, a module entity-type list, or a
   module-named SQL identifier.

## Sources of truth and ownership

- canonical owner of the stored documents: `rustok-search` (`search_documents`,
  migrations, `SearchProjectionDriver`);
- canonical owner of one module's document set and refresh width: the module
  crate that implements `SearchProjectionSource`;
- canonical owner of the event ordering, inbox claim and owner-revision
  checkpoint for the Forum projection: still `rustok-search`
  (`forum_reconciliation`, `forum_inbox`, `forum_owner_checkpoint`,
  `forum_contract_ingress`). This is acknowledged Forum-specific orchestration
  that has not been inverted yet; see "Non-goals". The Blog author refresh is part of
  that orchestration surface: the Blog driver is driven from `ingestion` and from
  `ForumProjectionReconciler`, not from a Blog-owned worker;
- derived state: the Search documents themselves; the source tables stay
  authoritative.

## Invariants

- A source can only write documents whose `tenant_id` matches the request and
  whose `source_module` matches its own `source_module()`; a foreign or
  non-public document fails the whole scope replacement.
- A scope replacement is atomic and can never remove another module's documents.
- A bounded page that returns a non-advancing cursor fails closed.
- A document whose entity type is blank or longer than 64 bytes fails closed.

### Allowed states

- `Entity` refresh: exactly the documents returned for the changed entity are
  replaced.
- `Tenant` refresh: the whole `(tenant, source_module)` scope is rebuilt from the
  source's current public state.

### Forbidden states

- A driver that hard-codes a module slug, an entity-type list or a module-named
  stage table.
- A module that widens a refresh by mutating Search state; widening is a
  declaration (`refresh_scope`), not a side effect.
- Two writers for the same `(tenant, source_module)` scope.

## Data, transaction, and concurrency boundary

A scope replacement is one database transaction on the caller's connection:

1. `CREATE TEMP TABLE search_projection_stage (LIKE search_documents INCLUDING
   DEFAULTS) ON COMMIT DROP`;
2. bounded pages from `source.list_public_documents` are validated and inserted
   into that stage table;
3. `DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = $2`
   removes only this module's previous scope;
4. the staged rows are inserted into `search_documents` and the transaction
   commits.

`ON COMMIT DROP` keeps the stage table session-local, so the fixed name cannot
collide between two owners rebuilding at the same time, and a rollback leaves the
previous scope untouched. A single-entity refresh uses the same shape without the
stage table: delete the exact `(tenant, source_module, entity_type, document_id)`
rows, insert the entity's locale documents, commit.

The driver itself takes no advisory lock and owns no queue; a scope must have one
writer, which the owning module's ingestion path is responsible for. Cross-process
serialisation for the Forum projection lives in `forum_inbox`
(`pg_try_advisory_xact_lock`), not here.

## Context dimensions

- tenant: every query and every validation is tenant-scoped; a document for
  another tenant fails closed;
- source module: the only scope predicate used for deletes and validation, taken
  from the source rather than from a caller argument;
- entity: `(entity_type, document_id)` for `Entity` refreshes; `entity_type` is
  owner-defined but must be non-empty and bounded;
- locale: a source returns one document per locale; the driver never derives a
  locale and stores whatever the source supplies, bounded to 16 bytes;
- channel/audience: not a driver dimension. Channel and audience eligibility stay
  in the storefront read path and the module's own projection decisions.

## Events and projections

The driver is the projection writer, not the event owner. Event handling stays
with the module ingestion path:

- full rebuild: tenant-level events and explicit tenant reindex requests rebuild
  the module scope through `rebuild_tenant`;
- single entity: a category-level reindex calls `refresh_entity`, which consults
  `refresh_scope` first and escalates to a tenant rebuild when the owner says the
  entity aggregates other documents (Forum topics own their approved replies);
- module disable and tenant deletion call `delete_tenant`, which deletes only
  that module's rows.

Search's own core scopes (`node`, `product`) and the Blog scope keep their own
projectors; the driver never touches them because every delete is
`source_module`-scoped.

## Failure semantics

Everything fails closed:

- non-PostgreSQL backend: the driver refuses to run
  (`SearchProjectionDriver requires PostgreSQL backend for <module>`) instead of
  degrading to a partial write;
- a page larger than `MAX_SEARCH_PROJECTION_PAGE_SIZE` or one that returns the
  same cursor twice: `Validation`, transaction rolled back;
- a foreign tenant, a foreign `source_module`, a non-public document, a blank
  entity type, an oversized entity type, an empty or oversized document key, an
  empty locale or status: `Validation`, transaction rolled back;
- a source that changes the target identity mid-refresh
  (`document.entity_type` / `document.document_id` no longer match the request):
  `Validation`, transaction rolled back;
- an entity refresh that returns more than `MAX_TARGET_ENTITY_DOCUMENTS` locale
  documents: `Validation`, transaction rolled back.

A failed rebuild therefore keeps the previous scope, and a failed entity refresh
keeps the previous entity documents, because the delete and the insert share one
transaction.

## Migration and cutover

- `crates/modules/rustok-search/src/forum_projector.rs` is deleted;
  `projection_driver.rs` replaces it, and `mod projection_driver;` replaces
  `mod forum_projector;` in the crate root.
- `ForumSearchProjector` call sites (`ingestion`, `forum_reconciliation`,
  `forum_owner_checkpoint`) now hold `SearchProjectionDriver`.
- The stage table rename (`forum_search_projection_stage` to
  `search_projection_stage`) needs no data migration: the table is created and
  dropped inside one transaction and never persists.
- `pages_projector.rs` and `docs_projector.rs` are deleted: they were copies of the
  driver with `'pages'`/`'docs'` hard-coded and one staging table each. Pages and Docs
  now use `SearchProjectionDriver` like Forum, which removed 677 lines of duplicated
  SQL. The driver gained the one capability those copies had and it did not:
  `delete_entity`, for a lifecycle event that reports an entity as unpublished or
  deleted.
- `blog_projector.rs` is deleted as well: Blog is stored through the same driver from
  `rustok_blog::BlogSearchProjectionSource`. The two behaviours that used to differ are
  now the owner's decision, not Search's: the indexed corpus is `published` and stamped
  exactly like every public Blog read, and the author name comes from the Profiles
  `ProfileSummaryReader` port instead of a `LEFT JOIN users`, so a profile whose
  visibility forbids public presentation can no longer leak its account name into
  Search. `projector_core.rs` (content/product) is the single remaining private writer.
- The author projection is a `Tenant` rebuild, not a new refresh scope: a source cannot
  enumerate "the documents of one author" when the author is a related owner entity, and
  Search must not filter stored rows by a payload key it does not own. Widening it is
  the ingestion owner's statement (`refresh_blog_author_scope`), and the optional
  `ForumProjectionReconciler::with_blog_source` makes the Forum sweeper do the same for
  the tenant-wide inbox scopes. A dependency-scoped refresh would need a reference index
  on `search_documents`; that is recorded as a future option, not as a gap in this
  decision.
- `SearchProjectionSource::refresh_scope` has a default, so all fourteen existing
  sources compile unchanged and keep `Entity` semantics; only Forum overrides it.
- Contract keys that named the deleted file were re-pointed
  (`search_projector` to `search_projection_driver`, `forum_projector` to
  `search_projection_driver`, `existing_forum_projector_reused` to
  `existing_projection_driver_reused`), and the guards that pinned the old
  symbols were updated in the same change.
- Rollback: restore the deleted file and the module declaration; no persisted
  state depends on the new names.

## Alternatives considered

1. **Keep `forum_projector.rs` and add a second driver per module.** Rejected:
   it multiplies the same transaction four times and makes the storage contract
   drift per module.
2. **Move the storage driver into `rustok-forum` (and each module).** Rejected:
   `search_documents` is Search storage; a module crate writing it directly
   creates two owners for one table and requires a `rustok-forum` ->
   `rustok-search` dependency edge that does not otherwise exist.
3. **Keep the entity-type allowlist in the driver and extend it per module.**
   Rejected: it is exactly the "Search knows module names" coupling being
   removed. Validation now uses `source_module()` plus a bounded entity type,
   and the owner's own loader rejects unknown entity types.
4. **Infer the refresh width from the entity type** (`forum_topic` means
   rebuild). Rejected: it hides an owner decision in a Search-side string
   comparison. `refresh_scope` makes the same rule explicit and overridable.
5. **Move the Forum orchestration files in the same change.** Deferred, not
   rejected: it is a separate inversion (see Non-goals) with a much larger
   blast radius across guards.

## Non-goals

- Moving `search_documents` or its migrations into a module crate.
- Inverting the remaining Forum-specific orchestration in this change. The
  follow-up packet is to keep the storage, inbox and reconciler in Search but
  drive them from a module-supplied scope policy (`SearchProjectionSource` plus
  an event-to-scope policy object), so the Forum names leave `rustok-search`
  without a `rustok-forum` -> `rustok-search` dependency edge or a second
  storage implementation.
- Changing the storefront read path; that is governed by
  `2026-10-25-search-audience-declared-corpus.md`.

## Verification

Not compiled and not tested in the sandbox that produced this decision: no Rust
toolchain is available there, and the change is statically reviewed only. The same
holds for the Pages/Docs cutover: it was reviewed against the deleted files line by
line and pinned by guards, not executed.

Pinned by `node scripts/verify/verify-forum-search-projection.mjs`,
`verify-forum-search-rebuild-scope-preservation.mjs`,
`verify-forum-approved-reply-search.mjs`,
`verify-forum-search-canonical-route-reindex-harness.mjs`,
`verify-forum-search-account-deletion-redaction.mjs`,
`verify-forum-search-inbox-sweeper.mjs` and
`npm run verify:search:audience`. The Rust source-text contracts
`search_scope_preservation_contract`, `forum_approved_reply_projection_contract`
and `forum_projection_sweeper_contract` pin the same invariants from inside the
crate.

## Consequences

- Adding a projecting module is now: implement `SearchProjectionSource`,
  optionally override `refresh_scope`, register the source. No Search change.
- Search no longer needs a per-module guard exception, and the guards assert the
  driver by behaviour (transaction plus `source_module` scope) instead of by a
  module name.
- Two guards that had drifted from the code
  (`verify-forum-search-projection`, `verify-forum-search-inbox-sweeper`) now
  pass, because their markers were re-pointed at the real owners.
- The Forum-named orchestration files remain the largest remaining
  module-specific surface in `rustok-search`; this ADR records that explicitly
  instead of hiding it.

## References

- `crates/libs/rustok-core/src/search_projection.rs`
- `crates/modules/rustok-search/src/projection_driver.rs`
- `crates/modules/rustok-forum/src/search_projection.rs`
- `crates/modules/rustok-search/tests/search_scope_preservation_contract.rs`
- `crates/modules/rustok-search/tests/forum_approved_reply_projection_contract.rs`
