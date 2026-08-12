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

Route resolution is fail-closed for legacy or externally-corrupted ambiguity.
For each locale candidate, translation and alias matches are deduplicated by
term identity: zero distinct terms proceeds to the next locale, one resolves,
and more than one returns a conflict instead of silently preferring one storage
table. A translation slug and alias that both belong to the same term are not
ambiguous. The owner transaction lookup applies the same distinct-term rule
while retaining its broader lifecycle semantics.

New writes additionally use `taxonomy_term_route_keys` as the storage-level
reservation authority. Its composite primary key is the complete localized
route identity (`tenant + kind + scope_type + scope_value + locale + route_key`)
and `term_id` is the owner. The migration preflights existing translation and
alias rows before creating the registry: same-term duplicate representations
are deduplicated, while a cross-term collision blocks migration with a
deterministic diagnostic instead of choosing a winner. Runtime localized
mutation finalization reconciles translation+alias rows with registry
reservations in the same transaction. Missing reservations are inserted before
stale reservations are released, so a concurrent claimant must win the single
database key or roll its mutation back. Term deletion removes reservations via
the composite tenant/term foreign-key cascade.

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
   resolution, fail-closed ambiguity handling, and storage-level route-key
   reservation. Category parent/child hierarchy remains owner-domain state
   rather than an implicit taxonomy kind.
   **Depends on:** a concrete domain requirement and scope decision.
   **Done when:** canonical-key, alias/slug, tenant, module-scope, locale,
   lifecycle, ambiguity, and any newly demonstrated kind semantics are defined
   and tested.

3. **Maintain dictionary operational guidance.** Add documentation and runbooks
   when a changed vocabulary contract introduces drift or integration recovery
   risk. Route-registry migration failures must be repaired by resolving the
   reported cross-term owner collision, never by deleting an arbitrary winner.
   **Depends on:** an actual runtime or consumer incident class.
   **Done when:** operators can reconcile terms, aliases, registry reservations,
   and owner attachments without inventing shared relation ownership.

4. **Collect production target and route-registry evidence.** Run the retained
   Outbox and Taxonomy migrations plus PostgreSQL concurrent localized-write,
   translation apply, and change-cursor scenarios before treating the route
   registry and `taxonomy/term` target as production-proven under replicas.
   **Depends on:** a production-like PostgreSQL runtime.
   **Done when:** retained migration/backfill, two-writer route-key contention,
   concurrent CAS, and cursor-recovery evidence prove that exactly one route
   owner can commit and the registered provider remains correct under
   multi-replica conditions.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- Targeted term CRUD, cross slug/alias collision, scope restriction, active-only
  route resolution, fail-closed ambiguity, locale fallback, route-registry
  reservation/release/cascade, migration preflight, and consumer-integration
  tests.
- `cargo test -p rustok-taxonomy --lib`
- Production-like PostgreSQL two-writer route-key contention before declaring
  storage concurrency evidence complete.

## Change rules

1. Keep dictionary terms and scope policy in this module.
2. Keep parent/child category hierarchy, ordering, cycle validation, and
   domain-specific category metadata with the owning module unless an explicit
   shared-hierarchy ADR changes ownership.
3. Keep localized route mutations and `taxonomy_term_route_keys` reservations
   in one database transaction; never repair collisions by table-order
   precedence or backend-specific trigger behavior.
4. Update local docs, `rustok-module.toml`, and consumer docs with a taxonomy
   contract change.
5. Update `docs/modules/registry.md` with any ownership or module-status change.
