# Implementation plan for `rustok-taxonomy`

## Current state

`rustok-taxonomy` owns tenant-scoped dictionary terms, translations, aliases,
canonical keys, and global/module scope rules. It is a vocabulary layer, not
shared product storage: blog, forum, product, and profiles retain their own
attachment tables and public domain contracts.

Term identity is locale-independent. Locale normalization and fallback use the
shared content contract. New consumers must attach terms through an explicit
owner-module relation table.

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
   **Depends on:** a concrete domain requirement and scope decision.
   **Done when:** canonical-key, alias/slug, tenant, module-scope, and locale
   fallback semantics are defined and tested.

3. **Maintain dictionary operational guidance.** Add documentation and runbooks
   when a changed vocabulary contract introduces drift or integration recovery
   risk.
   **Depends on:** an actual runtime or consumer incident class.
   **Done when:** operators can reconcile terms, aliases, and owner attachments
   without inventing shared relation ownership.

4. **Collect production target evidence.** Run the append-only Outbox and
   Taxonomy migrations plus PostgreSQL concurrent apply/change-cursor scenarios
   before enabling the `taxonomy/term` pilot in production.
   **Depends on:** a production-like PostgreSQL runtime.
   **Done when:** retained migration, concurrent CAS, and cursor-recovery
   evidence proves the registered provider under multi-replica conditions.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- Targeted term CRUD, alias lookup, scope restriction, locale fallback, and
  consumer-integration tests.
- `cargo test -p rustok-taxonomy --lib`

## Change rules

1. Keep dictionary terms and scope policy in this module.
2. Update local docs, `rustok-module.toml`, and consumer docs with a taxonomy
   contract change.
3. Update `docs/modules/registry.md` with any ownership or module-status change.
