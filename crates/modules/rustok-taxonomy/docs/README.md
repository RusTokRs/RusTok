# `rustok-taxonomy` Documentation

`rustok-taxonomy` is the shared classification/vocabulary module of the platform. It owns canonical
term identity, localized route copy and scope rules while consumer modules retain their own entity-to-
taxonomy relations and domain policy.

The runtime implements Tags and Categories. The
[Category platform plan](../../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
tracks consumer cutovers to Taxonomy-owned hierarchy and localized copy.

## Purpose

- publish the canonical shared classification contract;
- keep term/category identity, localized labels/slugs/descriptions, aliases and scope rules inside the module;
- own shared Category hierarchy and canonical presentation once the staged Category migration lands;
- provide domain modules with shared taxonomy identities without reverting to polymorphic shared product storage;
- expose Taxonomy entities such as `taxonomy.category` to the platform Flex capability when runtime custom fields are explicitly enabled.

## Storage length boundaries

Taxonomy normalizes route and scope inputs before persistence. Canonical/localized
route keys and aliases are bounded to 120 characters after normalization, while
module `scope_value` is bounded to 64 characters after normalization. Mutation
and owner-read adapters reject values beyond these storage boundaries with typed
validation errors instead of leaking database failures or silently producing
unresolvable scope selectors.
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
- module-scoped Category reorder accepts **exactly** the existing siblings of the
  selected parent (no omissions or extra ids that would silently reparent a
  child); module-scoped sibling insertion shifts positions only for Category
  identities in that module, including the shared `parent_id = NULL` root
  partition;
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
- [Route registry recovery](./route-registry-recovery.md)
- [Taxonomy Category + Flex platform plan](../../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- [Manifest layer contract](../../../../docs/modules/manifest.md)
## Transaction-aware localized name reads

`TaxonomyService::resolve_term_names_in` accepts the caller's `ConnectionTrait` connection, so
consumer-owned aggregate transactions can project names for terms they just created without
committing first. Term and translation queries both retain tenant predicates and use that same
connection. Ordinary `resolve_term_names` delegates to this implementation on the service connection;
consumer modules do not read Taxonomy entities to assemble their responses. The owner-read
regression covers transaction visibility, foreign-tenant absence and rollback.
`TaxonomyOwnerReader::load_scoped_terms_in` and
`TaxonomyService::resolve_term_id_for_module_in` preserve the same connection for
scoped terms and localized route keys. Their existing pool-backed entry points
use those implementations.

## Category copy and presentation

`sync_module_category_copy_in_tx` writes localized copy and hierarchy on the
caller's transaction. Taxonomy retains existing aliases, appends the prior slug
on rename and keeps the canonical presentation row and revision intact.
Historical slugs stay reserved as aliases.
Product category translation uses this operation. Callers that own an exact
icon and color snapshot use `sync_module_category_in_tx`.

The `taxonomy/term` target takes the Category hierarchy lock followed by
the term row lock before checking translation revisions. It uses the same
route history rules as regular Category editing. Applying an approved slug
change keeps the previous localized URL as an alias.

## Translation inventory maintenance

The change journal and content-free inventory baseline cover all Taxonomy
terms within the tenant, including terms whose localized copy belongs to a
consumer module. Both require effective `taxonomy:manage` and
`translations:manage` authority. The journal retains deletion evidence after
the term is gone. The baseline includes active terms in every locale so a
rebuild for one locale pair preserves other terms.

The journal assigns a separate ordered position per tenant. PostgreSQL assigns
positions through a deferred trigger that holds the tenant's journal head row
until commit. SQLite assigns them inside its serialized write transaction.
Inventory sync and progress read the same position. This lets sync resume
after a later-started write commits ahead of an earlier-started write.

Migration `m20261124_000012_order_taxonomy_translation_changes` preserves the
existing entries and backfills their positions. A checkpoint issued before
the migration triggers one paginated replay of retained evidence. A downgrade
to code that only understands UUID cursors requires restoring the matching
Translation checkpoint with the database backup.

Discovery, exact reads, progress and patch operations enforce the registered
module owner's authorization. Discovery cursors refer to visible terms.
Owners supply tenant enablement and field policy. The embedded Blog owner
uses the Category and Tag editor limits and excludes Tag descriptions from
translation. Sparse patches preserve stored descriptions when the request
omits them.
