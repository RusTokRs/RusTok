# `rustok-taxonomy` Documentation

`rustok-taxonomy` is the shared classification/vocabulary module of the platform. It owns canonical
term identity, localized route copy and scope rules while consumer modules retain their own entity-to-
taxonomy relations and domain policy.

The current runtime implementation is still Tag-only, but the accepted platform direction now adds a
first-class Taxonomy-owned `Category` kind with shared hierarchy. The staged migration is documented
in [`../../../docs/architecture/taxonomy-flex-category-platform-plan.md`](../../../docs/architecture/taxonomy-flex-category-platform-plan.md).
Historical docs/guards that pin Blog/Forum/Product category hierarchy to consumer modules are
transitional and must be replaced atomically with the Category implementation; they are not the target
architecture.

## Purpose

- publish the canonical shared classification contract;
- keep term/category identity, localized labels/slugs/descriptions, aliases and scope rules inside the module;
- own shared Category hierarchy and canonical presentation once the staged Category migration lands;
- provide domain modules with shared taxonomy identities without reverting to polymorphic shared product storage;
- expose Taxonomy entities such as `taxonomy.category` to the platform Flex capability when runtime custom fields are explicitly enabled.

## Scope

Current implemented scope:

- `taxonomy_terms`, `taxonomy_term_translations`, `taxonomy_term_aliases`, and
  append-only `taxonomy_translation_changes`;
- tenant-scoped term identity and `canonical_key`;
- scope contract for `global` and `module` terms;
- alias-aware lookup and module integration helpers;
- the registered `taxonomy/term` Translation target: exact source/target
  snapshots, field policy, revision/CAS apply, exact progress, and opaque
  owner change-cursor repair;
- generic durable receipt admission through the Core Outbox, scoped under
  owner slug `taxonomy`; Taxonomy still owns validation and its mutation
  transaction;
- no ownership over consumer relation tables such as `blog_post_tags`, `forum_topic_tags`,
  `product_tags` or future category binding tables.

Accepted Category target scope:

- `TaxonomyTermKind::Category`;
- shared parent/child hierarchy, ordering, cycle/depth invariants and category moves;
- localized category `name`, `slug`, `description` and aliases through Taxonomy;
- canonical category presentation such as icon/color and typed Media references;
- Taxonomy-owned Translation behavior for canonical category copy;
- explicit Flex opt-in for administrator-defined category extension fields.

Taxonomy must not own a generic polymorphic `owner_type/owner_id` attachment table. Shared category
identity/hierarchy and consumer attachment ownership are separate concerns.

## Integration

- `rustok-blog`, `rustok-forum`, `rustok-product` and `rustok-profiles` use Taxonomy as shared classification infrastructure;
- consumer attachment/binding ownership and public domain policy remain inside owning modules;
- canonical Category identity/hierarchy/localized copy migrates to Taxonomy instead of being reimplemented by each consumer;
- Blog and Forum tag orchestration enters Taxonomy through the transaction-aware service helper, so module-local term creation records the same revision and change evidence as direct Taxonomy writes;
- locale normalization and fallback must remain synchronized with the shared `rustok-content` contract;
- new taxonomy consumers must use explicit typed module-owned relation/binding tables rather than generic polymorphic attachment storage;
- custom fields are supplied by `flex`, not by a Taxonomy-local custom-field subsystem.

## Verification

Current baseline:

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- `cargo test -p rustok-taxonomy --lib`
- targeted tests for term CRUD, scope rules, alias lookup, consumer-module
  integration helpers, and exact Translation-target apply/replay/change cursor

Category migration adds focused evidence for hierarchy cycle/depth/order behavior, route identity,
tenant/scope isolation, exact locale/effective-locale projection, Translation CAS/progress, typed
consumer bindings and Flex Category extension.

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Taxonomy Category + Flex platform plan](../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)