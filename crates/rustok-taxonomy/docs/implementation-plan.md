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
copy. Category reuses that provider; Forum, Blog and Product must not retain duplicate category
Translation providers after their cutovers.

## Category ownership contract

Taxonomy-owned Category data includes:

- stable UUID and `canonical_key`;
- tenant and `global | module` scope;
- localized `name`, `slug`, `description` and aliases;
- parent/child hierarchy and sibling ordering;
- cycle, maximum-depth and same-scope invariants;
- canonical presentation: bounded semantic icon key, canonical color and typed Media identities;
- revision/change evidence used by Translation/cache consumers;
- opt-in Flex extension capability for tenant-defined fields.

A Category without an explicit hierarchy placement is a root category with default position `0`.
Hierarchy is typed Category storage, not a generic term relation: Tags do not acquire parent/child
semantics merely because they share the Taxonomy term table.

Canonical Category presentation is also separate typed storage. An absent presentation reads as an
empty canonical presentation at revision `0`. Presentation has its own optimistic revision because a
color/icon/image change must not invalidate the Taxonomy term revision used by Translation CAS.
Media remains the binary lifecycle owner: Taxonomy stores only typed Media identities and runtime
composition must validate same-tenant active public images before a write. Delivery URLs, storage
paths and blob lifecycle are never copied into Taxonomy.

Consumer-owned state stays outside Taxonomy. Examples include Forum moderation/audience/posting
policy and counters, Product merchandising/navigation assignment semantics, Blog placement policy,
and every module's typed relation/binding between its domain objects and a Taxonomy category.

## Flex boundary

`flex` is the platform custom-fields capability. Taxonomy must not build a second custom-fields
engine.

After canonical Category presentation is stable, `taxonomy.category` becomes an explicit Flex donor.
Built-in Category identity, hierarchy, localization, routes and canonical presentation remain
normalized Taxonomy fields. Flex supplies administrator-defined extension fields, validation,
localized values, transport and generic admin schema-builder behavior.

Forum Topic remains an intentional Flex donor for optional tenant-defined topic fields. That does not
move Forum business invariants such as route identity, moderation state, category binding, accepted
solution, counters or access policy into Flex.

## Current implementation sequence

### TAXONOMY-CAT-1 — ownership decision and guardrails — COMPLETE

PR #3680 accepted Taxonomy as the canonical shared Category owner and Flex as the only runtime
custom-fields capability. The Taxonomy ownership/kind guardrails allow the intentional Category kind
and typed shared hierarchy while still rejecting generic polymorphic consumer storage.

### TAXONOMY-CAT-2 — Category kind + hierarchy foundation — COMPLETE

PR #3681 added `TaxonomyTermKind::Category`, Taxonomy-owned hierarchy persistence, bounded placement
APIs and storage-level hierarchy enforcement. Category writers are tenant/scope bounded, Tags are
rejected, position is non-negative, depth is capped at 16, and cycle prevention is enforced in both
service and storage boundaries. PostgreSQL hierarchy mutations serialize per tenant so concurrent
opposite moves cannot both commit an invalid cycle.

Retained focused evidence:

- `Taxonomy Ownership Boundary` and `Taxonomy Lookup Contract` passed on the final PR head;
- exact-head `Taxonomy PostgreSQL Evidence` run `32571095910` passed source contract, canonical
  PostgreSQL 16 migrations, Category hierarchy contention, route-registry contention evidence,
  Translation-target CAS/change-cursor evidence and the final gate;
- PR #3681 was squash-merged as `8746be7d5adcee0fd33005cb90065b92e3ba2cda`.

### TAXONOMY-CAT-3 — canonical Category presentation — IN PROGRESS

PR #3682 is the focused presentation slice. It adds `taxonomy_category_presentations` as canonical
Taxonomy-owned storage with:

- `icon_key` — normalized bounded ASCII kebab-case design token, maximum 64 bytes;
- `color` — normalized lower-case `#rrggbb` or `#rrggbbaa`, accepting short/long hex input only;
- `image_media_id` and `cover_media_id` — typed Media identities, never copied delivery URLs;
- independent presentation `revision` with full-replacement compare-and-swap semantics;
- empty revision `0` when no canonical presentation row exists;
- normalized no-op writes that do not advance presentation revision;
- Category-only read/write APIs; Tags cannot acquire Category presentation;
- an owner-neutral `TaxonomyCategoryMediaReferenceValidator` boundary. Runtime composition must
  delegate validation to the Media public-image owner contract and reject cross-tenant or non-public
  assets. Taxonomy does not add a hard compile/runtime dependency on Media merely to store optional
  identities.

The presentation revision is deliberately distinct from `taxonomy_terms.revision`. Translation
resource/source/target CAS remains about localized text; a presentation change must not invalidate a
text proposal. Consumer-specific presentation overrides remain future binding policy and must layer
over canonical Taxonomy values rather than copying them when a binding is created.

Focused evidence for this slice:

- `cargo test --locked -p rustok-taxonomy --test category_hierarchy --test category_presentation --test localized_route_lookup --test route_key_registry -- --nocapture`;
- `Taxonomy Lookup Contract` includes the new presentation suite and source paths;
- `Migration Compatibility` validates appended migration
  `m20260822_000011_create_taxonomy_category_presentations`;
- `Taxonomy PostgreSQL Evidence` applies the canonical migration graph on PostgreSQL 16.

**Done when:** the exact PR head passes the focused presentation/lookup tests and the canonical
PostgreSQL migration evidence, with no Forum/Blog/Product cutover or Flex donor work mixed in.

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
Category hierarchy concurrency, route-registry contention and Translation CAS/cursor behavior.

Checked-in evidence snapshots are retained provenance, but runtime-input changes intentionally make
them stale. Staleness must trigger a fresh PostgreSQL run; it must not prevent the runtime job from
starting. The compatibility source wrapper may bridge only known superseded historical plan
assertions and stale runtime fingerprints while validating the current Category plan markers. Any
structural verifier failure remains fatal. The workflow gate remains closed unless the current-head
PostgreSQL runtime job succeeds.

The runtime job must continue to:

- check out the exact PR head/push SHA;
- assert Rust `1.96.0`;
- apply the canonical server Migrator to PostgreSQL 16;
- execute Category hierarchy contention evidence;
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
- `cargo test --locked -p rustok-taxonomy --test category_hierarchy --test category_presentation --test localized_route_lookup --test route_key_registry -- --nocapture`
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
