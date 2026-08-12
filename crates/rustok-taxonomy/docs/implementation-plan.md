# Implementation plan for `rustok-taxonomy`

## Current state

`rustok-taxonomy` owns tenant-scoped dictionary terms, translations, aliases,
canonical keys, and global/module scope rules. It is a vocabulary layer, not
shared product storage: blog, forum, product, and profiles retain their own
attachment tables and public domain contracts.

Term identity is locale-independent. Locale normalization and fallback use the
shared content contract. New consumers must attach terms through an explicit
owner-module relation table.

Localized public route keys have one lookup namespace per
`tenant + kind + scope + locale`: a translation slug on one term cannot be
shadowed by an alias on another term, and vice versa. Public module lookup
prefers the module scope over the global scope, follows requested locale ->
explicit fallback -> platform fallback, and does not resolve deprecated terms.
The transaction-aware owner helper may still reuse an existing deprecated term
identity; reactivation/replacement is an owner lifecycle decision rather than a
public-route lookup side effect.

Route resolution is fail-closed for legacy or concurrently-created ambiguity.
For each locale candidate, translation and alias matches are deduplicated by
term identity: zero distinct terms proceeds to the next locale, one resolves,
and more than one returns a conflict instead of silently preferring one storage
table. A translation slug and alias that both belong to the same term are not
ambiguous. The owner transaction lookup applies the same distinct-term rule
while retaining its broader lifecycle semantics.

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
append-only owner change cursor. `slug` remains review-only for machine
translation. Provider apply uses the shared Outbox receipt ledger under owner
slug `taxonomy`, while Taxonomy retains authorization, validation, and the
owner transaction. The provider records durable owner change evidence but does
not claim a global `translation.target.changed` event contract.

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
   The current tag lookup baseline requires locale-aware slug/alias uniqueness,
   module-before-global precedence, shared locale fallback, active-only public
   resolution, and fail-closed ambiguity handling. Category parent/child
   hierarchy remains owner-domain state rather than an implicit taxonomy kind.
   **Depends on:** a concrete domain requirement and scope decision.
   **Done when:** canonical-key, alias/slug, tenant, module-scope, locale,
   lifecycle, ambiguity, and any newly demonstrated kind semantics are defined
   and tested.

3. **Make the localized route namespace storage-atomic.** Service admission and
   fail-closed lookup protect normal writes and reads, but translation slugs and
   aliases still live in separate tables, so their cross-table uniqueness is
   not yet one database-enforced invariant under concurrent writers. Introduce
   one portable route-key reservation/registry authority rather than relying on
   table-order precedence or backend-specific trigger tricks.
   **Depends on:** an explicit migration/backfill contract for existing route
   keys and PostgreSQL concurrency evidence.
   **Done when:** one database unique key owns
   `tenant + kind + scope + locale + route_key`, both translations and aliases
   reserve/release it transactionally, conflicting legacy rows are detected
   deterministically, and concurrent writers cannot create ambiguity.

4. **Maintain dictionary operational guidance.** Add documentation and runbooks
   when a changed vocabulary contract introduces drift or integration recovery
   risk.
   **Depends on:** an actual runtime or consumer incident class.
   **Done when:** operators can reconcile terms, aliases, and owner attachments
   without inventing shared relation ownership.

5. **Collect production target evidence.** Run the append-only Outbox and
   Taxonomy migrations plus PostgreSQL concurrent apply/change-cursor scenarios
   before enabling the `taxonomy/term` pilot in production.
   **Depends on:** a production-like PostgreSQL runtime.
   **Done when:** retained migration, concurrent CAS, and cursor-recovery
   evidence proves the registered provider under multi-replica conditions.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- Targeted term CRUD, cross slug/alias collision, scope restriction, active-only
  route resolution, fail-closed ambiguity, locale fallback, and
  consumer-integration tests.
- `cargo test -p rustok-taxonomy --lib`

## Change rules

1. Keep dictionary terms and scope policy in this module.
2. Keep parent/child category hierarchy, ordering, cycle validation, and
   domain-specific category metadata with the owning module unless an explicit
   shared-hierarchy ADR changes ownership.
3. Update local docs, `rustok-module.toml`, and consumer docs with a taxonomy
   contract change.
4. Update `docs/modules/registry.md` with any ownership or module-status change.
