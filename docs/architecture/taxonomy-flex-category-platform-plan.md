# Taxonomy category ownership and Flex extension plan

**Status:** accepted architecture, staged implementation
**Reviewed:** 2026-08-22

## Decision

RusToK has two separate shared platform capabilities and they must not be collapsed into domain
modules:

1. **`rustok-taxonomy` owns shared classification.** Category identity, hierarchy, localized
   name/slug/description, aliases and canonical presentation belong to Taxonomy so the same category
   can be consumed by Forum, Blog, Product and future modules.
2. **`flex` owns runtime-defined custom-field capability.** Any domain entity may opt in to Flex
   through a minimal adapter. A consumer must not reimplement field definitions, validation,
   localization, generic attached values, schema-builder behavior or custom-field transport.

Flex is opt-in. An entity is not a Flex donor merely because it has a JSON/metadata column.

## Why this replaces the current category boundary

The current repository has a historical Tag-only Taxonomy contract and separate category aggregates
inside Forum, Blog and Product. That produces multiple implementations of hierarchy, localization,
presentation and translation ownership. It also prevents a tenant from defining one shared category
library and reusing category identity across modules.

The accepted target is therefore:

```text
rustok-taxonomy
  category identity
  category hierarchy
  canonical localized copy
  canonical presentation
  aliases / route identity
  Flex extension adapter

forum/blog/product/...
  relation or binding to taxonomy category id
  domain-specific policy/state only
```

Consumer relation/binding tables stay with the consumer. Taxonomy must not become a generic
`owner_type/owner_id` attachment backend.

## Taxonomy category contract

Add `TaxonomyTermKind::Category` as a first-class demonstrated kind. Category has the same
locale-independent identity, tenant/scope rules and localized route ownership as other Taxonomy
terms, plus an explicit hierarchy owned by Taxonomy.

Canonical Category-owned data includes:

- stable UUID and `canonical_key`;
- tenant and `global | module` scope;
- localized `name`, `slug`, `description` and aliases;
- parent/child hierarchy, ordering and cycle/depth invariants;
- canonical presentation such as icon key, color and Media-owned image/cover references;
- revision/change evidence required by Taxonomy Translation and cache consumers.

Presentation data that describes the category itself belongs to Taxonomy. A module-specific
placement can still own an override when there is a demonstrated UX requirement, but an override
must resolve against the canonical Taxonomy value instead of copying it at creation time.

Examples of domain-owned state that does **not** move into Taxonomy:

- Forum moderation/audience/topic-create/reply-create policy and Forum counters;
- Product merchandising, primary/navigation assignment semantics and product-specific projections;
- Blog-specific placement/visibility state;
- any consumer's relation between its own entity and a Taxonomy category.

## Flex contract

Flex is the only runtime custom-fields mechanism. A new module should be able to opt in by providing
an entity registration/storage adapter and permissions, not by building a new custom-fields stack.

The target onboarding shape is conceptually:

```text
register entity type
  -> identify tenant + entity id
  -> expose shared/non-localized donor payload or generic attached storage
  -> use Flex field-definition/value/localization/validation contracts
  -> generic admin schema builder renders the fields
```

The exact Rust adapter API may evolve, but the architectural result is fixed:

- one field-definition contract;
- one type/validation contract;
- one localization contract;
- one attached-value contract;
- one schema-builder/admin contract;
- one permission/governance model;
- no `*_custom_field_engine` or module-local replacement implementation.

Built-in domain fields remain normalized owner fields. Flex is for administrator-defined extension,
not for converting business invariants such as price, SKU, moderation state or route identity into
untyped metadata.

## Accepted Flex consumers

The current runtime has historical donors `user`, `product`, `order` and `topic`. The desired rule is
not to preserve that list mechanically; every donor must have demonstrated product intent.

- `user`: retained; user/profile extension is a demonstrated use case.
- `product`: retained; merchant-defined catalog attributes are a demonstrated use case, while core
  Product invariants remain normalized.
- `order`: retain only for non-critical extension data; payment/inventory/ledger invariants must
  never move into Flex.
- `taxonomy.category`: add after the Taxonomy Category owner exists. This is the extension point for
  tenant-specific category fields beyond the canonical built-ins.
- groups/profiles and future modules: opt in when their product surface explicitly supports custom
  fields.
- `forum.topic`: **not a Flex donor**. Topics have no accepted custom-field product surface. Their
  `metadata` column is Forum-owned internal/domain state and does not imply Flex support.

## Forum topic Flex retirement

Retirement is staged to avoid silently deleting tenant data:

1. Remove `topic` from the runtime `FieldDefRegistry` so new field-definition CRUD is no longer
   exposed.
2. Audit `topic_field_definitions`, `forum_topics.metadata` custom keys and
   `flex_attached_localized_values` rows for `entity_type = 'topic'` on production-like data.
3. If no user-defined data exists, add an owner migration that removes the topic field-definition
   table/cache trigger and any now-unused adapter/model/service source.
4. If user-defined data exists, preserve/export it explicitly and define a reviewed migration before
   dropping storage. Do not silently reinterpret it as Forum domain metadata.
5. Update Flex cache-generation tests from the historical four-donor matrix to the actual live donor
   set in the same cleanup slice.

The first registry-disable step is intentionally safe while legacy tables still exist.

## Category migration sequence

### Phase A — contract and guardrails

- Replace the historical `Tag`-only / flat-vocabulary assumption with an explicit Category kind and
  shared-hierarchy decision.
- Update Taxonomy ownership/contract-matrix negative fixtures so they reject generic polymorphic
  attachment storage but allow the intentional Category hierarchy owner.
- Keep route-key collision, tenant, locale and module/global precedence guardrails.

### Phase B — Taxonomy Category storage

- Add Category kind support and category hierarchy persistence.
- Define ordering, max-depth, move/cycle, delete/reparent and tenant/scope invariants.
- Add canonical presentation with typed Media references and bounded icon/color validation.
- Make the existing Taxonomy Translation provider handle Category localized fields without creating a
  second `forum/category`, `blog/category` or `product/category` Translation owner.
- Add richer localized read projection that preserves `effective_locale` for every resolved term.

### Phase C — Flex Category extension

- Register `taxonomy.category` as an explicit Flex donor.
- Reuse Flex definitions, values, localized values, validation, cache and transport.
- Add category custom-field rendering to the generic admin schema-builder path; Taxonomy must not
  implement a second custom-fields editor.
- Extend field types only through Flex when demonstrated (`Media`, references, rich text, etc.).

### Phase D — consumer migration

Migrate one consumer at a time from fresh `main` with explicit backfill and compatibility evidence.
Preserve existing category UUIDs when possible so owner relations do not require unnecessary ID
rewrites.

For each consumer:

- create/reuse Taxonomy Category rows and localized data;
- move canonical hierarchy/localized/presentation ownership to Taxonomy;
- replace local category identity with a typed Taxonomy category binding/reference;
- retain only consumer-specific policy/projection/placement fields;
- remove duplicate local Translation providers/tables after backfill verification;
- update admin/storefront projections to use Taxonomy `requested_locale` / `effective_locale`;
- run tenant-isolation, route, hierarchy and multilingual/RTL evidence.

Forum is the first migration because the current FORUM-25 Translation work exposed the ownership
conflict. Product and Blog follow with their own domain-specific binding semantics rather than a
blind table rename.

## Forum-specific target

After Forum migration:

```text
taxonomy category
  id / parent / order
  localized name/slug/description
  icon/color/media
  Flex custom fields

forum category binding/policy
  taxonomy_category_id
  moderation and posting policy
  audience policy
  Forum counters/projections
  optional demonstrated Forum presentation override only
```

`forum_category_translations` and `ForumCategoryTranslationTargetProvider` are transitional artifacts
and must be removed after verified backfill. FORUM-25 browser parity must prove the Taxonomy-owned
category projection, not retain evidence for the superseded Forum-owned provider.

## Verification gates

Every implementation slice must keep unrelated CI failures out of scope and add focused evidence for
its boundary. At minimum the completed program must prove:

- Taxonomy Category tenant/scope/route uniqueness and hierarchy cycle/depth/order behavior;
- exact locale + fallback projections with `effective_locale` preserved;
- Taxonomy Translation apply/CAS/progress for Category;
- Flex opt-in registry: unsupported entity types fail closed;
- `forum.topic` is not registered as a Flex donor;
- Category Flex definitions/values are tenant-scoped and multilingual where configured;
- consumer bindings reject cross-tenant Taxonomy category references;
- legacy category UUID/data backfill is deterministic and rollback/recovery is documented;
- Forum mounted multilingual/RTL admin/storefront evidence uses Taxonomy-owned category data;
- no consumer reintroduces a local generic custom-fields engine.

## Change rules

1. Do not add a second category owner to solve a consumer-specific UI problem.
2. Do not add a second custom-fields implementation to solve a donor-specific storage problem.
3. Flex support is explicit opt-in; a metadata column is not opt-in.
4. Taxonomy owns shared Category hierarchy; consumers own only their relationships and domain policy.
5. Media owns binary lifecycle; Taxonomy/Flex store typed Media references, not copied delivery URLs.
6. Translation for canonical category copy is Taxonomy-owned.
7. Keep migrations staged and data-preserving; never drop legacy custom/category data before an audit.
