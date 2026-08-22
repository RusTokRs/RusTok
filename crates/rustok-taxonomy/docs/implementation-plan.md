# Implementation plan for `rustok-taxonomy`

## Current state

`rustok-taxonomy` is the canonical shared classification capability. It owns stable term identity,
localized copy and routes, aliases, scope rules and the shared Category model. Consumer modules keep
their own relations/bindings and domain policy; Taxonomy must not become a generic polymorphic
`owner_type/owner_id` attachment store.

The accepted kind surface is:

- `Tag` — flat shared vocabulary used by Blog, Forum, Product, Profiles and other consumers;
- `Category` — shared hierarchical classification reused across Forum, Blog, Product and future
  modules.

This plan follows `DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md` and
`docs/architecture/taxonomy-flex-category-platform-plan.md`. Those decisions supersede historical
Tag-only/flat-vocabulary text that kept category identity, hierarchy and canonical translations in
consumer modules.

Term identity is locale-independent. Locale normalization and fallback use `rustok-content`.
Localized route keys have one storage namespace per `tenant + kind + scope + locale`, with
`taxonomy_term_route_keys` as the route ownership authority. Taxonomy terms retain hard-delete
semantics rather than a hidden archived lifecycle.

`TaxonomyTranslationTargetProvider` remains the single Translation owner for canonical Taxonomy
copy. Category must reuse that provider; Forum, Blog and Product must not retain duplicate category
Translation providers after their cutovers.

## Category ownership contract

Taxonomy-owned Category data includes:

- stable UUID and `canonical_key`;
- tenant and `global | module` scope;
- localized `name`, `slug`, `description` and aliases;
- parent/child hierarchy and sibling ordering;
- cycle, maximum-depth and same-scope invariants;
- canonical presentation such as icon, color and typed Media references;
- revision/change evidence used by Translation/cache consumers;
- opt-in Flex extension capability for tenant-defined fields.

A Category without an explicit hierarchy placement is a root category with default position `0`.
Hierarchy is typed Category storage, not a generic term relation: Tags do not acquire parent/child
semantics merely because they share the Taxonomy term table.

Consumer-owned state stays outside Taxonomy. Examples include Forum moderation/audience/posting
policy and counters, Product merchandising/navigation assignment semantics, Blog placement policy,
and every module's typed relation/binding between its domain objects and a Taxonomy category.

## Flex boundary

`flex` is the platform custom-fields capability. Taxonomy must not build a second custom-fields
engine.

After the Category owner is stable, `taxonomy.category` becomes an explicit Flex donor. Built-in
Category identity, hierarchy, localization, routes and canonical presentation remain normalized
Taxonomy fields. Flex supplies administrator-defined extension fields, validation, localized values,
transport and generic admin schema-builder behavior.

Forum Topic remains an intentional Flex donor for optional tenant-defined topic fields. That does not
move Forum business invariants such as route identity, moderation state, category binding, accepted
solution, counters or access policy into Flex.

## Current implementation sequence

### TAXONOMY-CAT-1 — ownership decision and guardrails — COMPLETE

PR #3680 accepted Taxonomy as the canonical shared Category owner and Flex as the only runtime
custom-fields capability. The Taxonomy ownership/kind guardrails now allow the intentional Category
kind and typed shared hierarchy while still rejecting generic polymorphic consumer storage.

**Done when:**

- ADR and platform plan are canonical;
- `Tag` and the accepted `Category` kind are the only demonstrated kinds;
- ownership verifiers reject generic consumer attachment storage but no longer reject typed Category
  hierarchy;
- Forum Topic remains Flex-enabled.

### TAXONOMY-CAT-2 — Category kind + hierarchy foundation — IN PROGRESS

Add `TaxonomyTermKind::Category` to the existing generic term/route/Translation contract and add
Taxonomy-owned hierarchy persistence.

Required behavior:

- child and parent are Category terms in the same tenant;
- child and parent have the same scope type/value;
- Tags are rejected by Category hierarchy APIs;
- self-parent and cycles are rejected;
- hierarchy depth is bounded to 16;
- sibling position is non-negative;
- root placement is explicit when stored, while a missing placement reads as root position `0`;
- tenant-composite foreign keys prevent cross-tenant hierarchy corruption;
- existing Tag lookup/route semantics remain unchanged.

Focused evidence:

- `cargo test --locked -p rustok-taxonomy --test category_hierarchy --test localized_route_lookup --test route_key_registry -- --nocapture`
- `Taxonomy Ownership Boundary`
- `Taxonomy Lookup Contract`
- canonical migration graph on PostgreSQL through `Taxonomy PostgreSQL Evidence`.

**Done when:** the focused Rust and ownership contracts pass on the exact PR head and the canonical
PostgreSQL migration/runtime evidence passes for the changed runtime inputs.

### TAXONOMY-CAT-3 — canonical Category presentation — PLANNED

Add typed canonical presentation without introducing module-specific copies:

- `icon_key` or equivalent bounded icon identity;
- validated canonical color representation;
- Media-owned image and cover references;
- read projections with clear canonical/override semantics.

Media remains the binary lifecycle owner. Taxonomy stores Media identity, not copied delivery URLs.
Module-specific presentation overrides are allowed only for demonstrated UX needs and must resolve
against the canonical Taxonomy value rather than copying it at binding creation time.

**Done when:** presentation is typed, tenant-safe and reusable by all consumers without local
`forum_category.icon`, `blog_category.image`, etc. becoming competing canonical owners.

### TAXONOMY-CAT-4 — Flex Category donor — PLANNED

Register `taxonomy.category` as an explicit Flex donor through the smallest reusable adapter contract.
Use the existing Flex definition/value/localization/validation/cache/transport stack and generic admin
schema builder.

**Done when:** a tenant can add shared or localized custom fields to Categories without Taxonomy
implementing a second field-definition service, validator or custom form engine.

### TAXONOMY-CAT-5 — Forum category cutover — PLANNED

Forum is the first consumer migration because FORUM-25 exposed the ownership conflict.

Migration rules:

- preserve existing Forum category UUIDs as Taxonomy term UUIDs where possible;
- backfill canonical localized copy, routes, hierarchy and presentation into Taxonomy;
- replace Forum category identity with a typed Taxonomy Category binding/reference;
- keep only Forum-specific policy, counters and demonstrated presentation overrides in Forum;
- remove `forum_category_translations` and `ForumCategoryTranslationTargetProvider` only after
  deterministic backfill/read-write cutover evidence;
- make Forum admin/storefront consume Taxonomy `requested_locale`/`effective_locale` projections;
- keep Topic Flex support independent of Category migration;
- complete mounted multilingual/RTL browser parity only against Taxonomy-owned Category data.

**Done when:** Forum no longer owns a duplicate canonical category entity or Translation provider and
all Forum category behaviors use the shared Taxonomy identity without losing Forum-specific policy.

### TAXONOMY-CAT-6 — Blog/Product and later consumers — PLANNED

Migrate each consumer separately from fresh `main`; do not combine unrelated category models into one
large cutover. Preserve module-specific bindings and policy, reuse Taxonomy identity/hierarchy/copy,
and validate tenant isolation and route semantics for every consumer.

Product and Blog follow Forum, but their navigation/merchandising/placement semantics remain their own
bounded contracts rather than being blindly moved into Taxonomy.

## Lookup and Translation invariants

The existing route and Translation machinery remains authoritative for both demonstrated kinds:

- `taxonomy_term_route_keys` serializes localized slug/alias ownership;
- module scope is preferred before global scope where module lookup semantics apply;
- requested locale -> explicit fallback -> platform fallback remains the presentation order;
- `requested_locale` and `effective_locale` must both survive owner projections;
- localized authoring never copies fallback content into the target locale;
- hard delete releases canonical/route identities through owner-controlled persistence semantics;
- Translation applies use resource/source/target revision CAS and durable owner change cursors.

A richer bounded resolver must preserve each resolved term's `effective_locale`; consumers must not
label fallback text with the requested/content locale when Taxonomy resolved another locale.

## PostgreSQL evidence policy

`Taxonomy PostgreSQL Evidence` is the production-like runtime gate for the canonical migration graph,
route-key contention and Translation CAS/cursor behavior.

Checked-in evidence snapshots are retained provenance, but runtime-input changes intentionally make
them stale. Staleness must trigger a fresh PostgreSQL run; it must not prevent the runtime job from
starting. The source phase may tolerate only the specific "runtime input changed since recorded
evidence" condition. Any structural verifier failure remains fatal. The workflow gate remains closed
unless the current-head PostgreSQL runtime job succeeds.

The runtime job must continue to:

- check out the exact PR head/push SHA;
- assert Rust `1.96.0`;
- apply the canonical server migrator to PostgreSQL 16;
- execute route-registry contention evidence;
- execute Translation-target CAS/change-cursor evidence;
- archive exact-head metadata and logs.

## Verification

Focused commands for the Category program:

- `cargo xtask module validate taxonomy`
- `node scripts/verify/verify-taxonomy-ownership-boundary-self-test.mjs`
- `node scripts/verify/verify-taxonomy-ownership-boundary.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.test.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.mjs`
- `cargo test --locked -p rustok-taxonomy --test category_hierarchy --test localized_route_lookup --test route_key_registry -- --nocapture`
- `cargo test --locked -p rustok-taxonomy --lib`
- PostgreSQL commands retained in `.github/workflows/taxonomy-postgres-evidence.yml`.

Consumer cutovers add their own focused owner, migration, transport, multilingual/RTL and browser
evidence. Unrelated/common workspace CI failures are not a reason to expand a Category PR's scope.

## Change rules

1. Taxonomy owns shared Category identity, hierarchy, localized copy, aliases/routes and canonical
   presentation.
2. Consumer modules own typed bindings/relations and domain-specific policy/state.
3. Do not add generic polymorphic `owner_type/owner_id` consumer persistence to Taxonomy.
4. Do not create duplicate Forum/Blog/Product category Translation providers after Taxonomy is the
   canonical owner.
5. Flex is the only runtime custom-fields engine; `taxonomy.category` opts in rather than rebuilding
   definitions/validation/localization/transport.
6. Built-in business invariants remain normalized owner fields even for Flex-enabled entities.
7. Preserve category UUIDs during consumer backfills where possible.
8. Never drop legacy category data before deterministic backfill and read/write cutover evidence.
9. Media owns binary lifecycle; Taxonomy/Flex store typed Media references.
10. Every slice starts from fresh `main`, stays narrow, and fixes only failures caused by its own
    boundary.

## References

- [`DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md`](../../../DECISIONS/2026-08-22-taxonomy-category-flex-ownership.md)
- [`docs/architecture/taxonomy-flex-category-platform-plan.md`](../../../docs/architecture/taxonomy-flex-category-platform-plan.md)
- [`docs/route-registry-recovery.md`](./route-registry-recovery.md)
- [`../README.md`](../README.md)
