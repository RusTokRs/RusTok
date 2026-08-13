# Implementation plan for `rustok-taxonomy`

## Current state

`rustok-taxonomy` owns tenant-scoped dictionary terms, translations, aliases,
canonical keys, and global/module scope rules. It is a vocabulary layer, not
shared product storage: blog, forum, product, and profiles retain their own
attachment tables and public domain contracts.

Term identity is locale-independent. Locale normalization and fallback use the
shared content contract. New consumers must attach terms through an explicit
owner-module relation table.

Taxonomy terms have no soft-deprecated/archive state. A persisted term is the
current canonical term identity. Removal is an actual delete; owner relation
foreign keys and route-key reservations must make deletion effects explicit
rather than hiding an unusable term behind a lifecycle flag. Historical
`archived` Taxonomy translation-change evidence is normalized to `active` by
the retained status-removal migration; real deletions remain `deleted`.

Localized route keys have one lookup namespace per
`tenant + kind + scope + locale`: a translation slug on one term cannot be
shadowed by an alias on another term, and vice versa. Module lookup prefers the
module scope over the global scope and follows requested locale -> explicit
fallback -> platform fallback. Owner transaction lookup uses the same existing
term identity and creates a new module term only when neither a route key nor a
canonical key resolves.

`taxonomy_term_route_keys` is the storage-level route ownership authority. Its
composite primary key is the complete localized route identity
(`tenant + kind + scope_type + scope_value + locale + route_key`) and `term_id`
is the owner. The migration preflights existing translation and alias rows
before creating the registry: same-term duplicate representations are
deduplicated, while a cross-term collision blocks migration with a deterministic
diagnostic instead of choosing a winner. Runtime localized mutation
finalization reconciles translation+alias rows with registry reservations in
the same transaction. Missing reservations are inserted before stale
reservations are released, so a concurrent claimant must win the single
database key or roll its mutation back. Term deletion removes reservations via
the composite tenant/term foreign-key cascade.

Service and consumer route resolution read `taxonomy_term_route_keys` directly.
The old translation-first/alias-second lookup path is not a second authority:
translation and alias tables hold localized content, while the registry owns
route identity and collision serialization.

Category hierarchy is deliberately outside the shared dictionary contract.
Parent/child category edges, ordering, cycle rules, and domain-specific category
metadata belong to the module that owns the category aggregate and its public
contract. Taxonomy must not acquire a generic `parent_id`, category tree, or
polymorphic relation table merely to centralize hierarchy. If a future domain
needs a shared hierarchical vocabulary, that requires a separate demonstrated
kind/ownership decision and explicit migration contract.

`TaxonomyTranslationTargetProvider` is registered by server composition as
`taxonomy/term`. It exposes exact source/target snapshots for `name`, `slug`,
and optional `description`; applies one target locale through resource/source/
target revision CAS; reports exact source/target progress; and reads the
append-only owner change cursor. Existing terms always expose
`TranslationResourceLifecycle::Active`; actual term deletion produces a
`deleted` change record. `slug` remains review-only for machine translation.
Provider apply uses the shared Outbox receipt ledger under owner slug
`taxonomy`, while Taxonomy retains authorization, validation, and the owner
transaction. The provider records durable owner change evidence but does not
claim a global `translation.target.changed` event contract.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- This dictionary module has no module-owned UI. Its owner-neutral Translation
  target SPI is an embedded capability boundary; no Taxonomy-specific UI or
  external transport is implied.

## Open results

1. **Keep dictionary and consumer contracts synchronized.** Update taxonomy
   terms, scope rules, consumer integrations, and manifest metadata atomically.
   **Depends on:** the change-owning consumer module.
   **Done when:** an owning module, rather than taxonomy, owns each attachment
   table and public relation contract.

2. **Expand kinds and lookup semantics only for demonstrated domain pressure.**
   Do not add speculative vocabulary kinds or polymorphic attachment storage.
   The current tag lookup baseline requires locale-aware route ownership,
   module-before-global precedence, shared locale fallback, registry-authority
   lookup, hard-delete-only term removal, and storage-level route-key
   reservation. Category parent/child hierarchy remains owner-domain state
   rather than an implicit taxonomy kind.
   **Depends on:** a concrete domain requirement and scope decision.
   **Done when:** canonical-key, alias/slug, tenant, module-scope, locale,
   deletion, and any newly demonstrated kind semantics are defined and tested.

3. **Maintain dictionary operational guidance.** Add documentation and runbooks
   when a changed vocabulary contract introduces drift or integration recovery
   risk. Route-registry migration failures must be repaired by resolving the
   reported cross-term owner collision, never by deleting an arbitrary winner.
   **Depends on:** an actual runtime or consumer incident class.
   **Done when:** operators can reconcile terms, aliases, registry reservations,
   and owner attachments without inventing shared relation ownership.

4. **Collect production target and route-registry evidence.** Run the canonical
   server migration graph, including the owner-operation receipt dependency and
   retained Taxonomy migrations, plus PostgreSQL concurrent localized-write,
   translation apply, and change-cursor scenarios before treating the route
   registry and `taxonomy/term` target as production-proven under replicas.
   **Depends on:** a production-like PostgreSQL runtime.
   **Done when:** retained migration/backfill, two-writer route-key contention,
   concurrent CAS, and cursor-recovery evidence prove that exactly one route
   owner can commit and the registered provider remains correct under
   multi-replica conditions.

   Source evidence for the two-writer route-key contention portion is now
   executable in `tests/route_registry_contention_postgres.rs`. With
   `RUSTOK_TAXONOMY_TEST_DATABASE_URL` it creates an isolated PostgreSQL schema,
   runs the retained Taxonomy migrations, uses two independent writer
   connections, forces both localized writers past route preflight before
   releasing their translation-row locks, and verifies that the registry key
   admits exactly one commit while the losing translation rolls back. This
   source harness is not recorded runtime evidence by itself.

   Source evidence for translation-target CAS and cursor recovery is now
   executable in `tests/translation_target_postgres.rs`. It requires the
   canonical PostgreSQL schema produced by the canonical server Migrator and
   refuses to run if the owner-operation receipt ledger or required Taxonomy
   tables are absent. The scenarios use unique tenant identities and independent
   single-session connections, then race two applies from the same exact
   source/target snapshot and expected revisions. Exactly one stale-revision
   candidate may commit; the loser must close as a conflict, leaving one
   resource/target revision advance and one durable winning change fact. A
   separate recovery scenario resumes from the create cursor after provider
   reconstruction, applies an exact target, reconstructs again around hard
   deletion, resumes the `deleted` lifecycle change, drains after the latest
   cursor, and verifies progress exposes that latest durable owner cursor.
   Sequential recovery writes are separated across ULID milliseconds for
   deterministic cursor ordering; this does not claim an arbitrary concurrent
   transaction commit-order guarantee.

   `.github/workflows/taxonomy-postgres-evidence.yml` is the retained runtime
   path. It provisions PostgreSQL 16, runs `rustok-migrate up` against the
   ephemeral database, executes both Taxonomy PostgreSQL harnesses, and archives
   migration/test provenance and logs. The source contracts deliberately keep
   `runtime_status` as `not_recorded` until such an exact-head workflow run
   succeeds; source presence alone is not runtime proof.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- Targeted term CRUD, cross slug/alias collision, scope restriction, locale
  fallback, registry-authority lookup, route-registry reservation/release/
  cascade, status-removal migration, and consumer-integration tests.
- `cargo test -p rustok-taxonomy --lib`
- `DATABASE_URL=postgresql://... cargo run --locked -p rustok-migrations --bin rustok-migrate -- up`
- `RUSTOK_TAXONOMY_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-taxonomy --test route_registry_contention_postgres -- --nocapture`
- `RUSTOK_TAXONOMY_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-taxonomy --test translation_target_postgres -- --nocapture`
- Production-like PostgreSQL canonical migration, two-writer route-key
  contention, translation apply CAS, and change-cursor recovery before declaring
  storage concurrency evidence complete.

## Change rules

1. Keep dictionary terms and scope policy in this module.
2. Keep parent/child category hierarchy, ordering, cycle validation, and
   domain-specific category metadata with the owning module unless an explicit
   shared-hierarchy ADR changes ownership.
3. Keep localized route mutations and `taxonomy_term_route_keys` reservations
   in one database transaction; never repair collisions by table-order
   precedence or backend-specific trigger behavior.
4. Do not add a soft-deprecated term state. A term either exists as the current
   identity or is deleted through the explicit owner/storage contract.
5. Update local docs, `rustok-module.toml`, and consumer docs with a taxonomy
   contract change.
6. Update `docs/modules/registry.md` with any ownership or module-status change.
