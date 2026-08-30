# Taxonomy category ownership and Flex extension plan

**Status:** accepted architecture, staged implementation
**Reviewed:** 2026-08-30

## Decision

RusToK has two separate shared platform capabilities and they must not be collapsed into domain
modules:

1. **`rustok-taxonomy` owns shared classification.** Category identity, hierarchy, localized
   name/slug/description, aliases and canonical presentation belong to Taxonomy so the same category
   can be consumed by Forum, Blog, Product and future modules.
2. **`flex` owns runtime-defined custom-field capability.** Any domain entity may opt in to Flex
   through a minimal adapter. A consumer must not reimplement field definitions, validation,
   localization, generic attached values, schema-builder behavior or custom-field transport.

Flex is opt-in. A domain entity is a Flex donor only when the product explicitly allows runtime
extension; a JSON/metadata column alone does not imply support.

## Implementation status

The architecture remains staged across consumers, but Blog is no longer a pending Category owner
migration. The Blog migration is complete through TAXONOMY-CAT-12: canonical localized copy, route
history, hierarchy projection and Translation ownership are Taxonomy-owned; Blog retains its typed
binding plus module-specific membership/settings/revision state. The former Blog Category Translation
provider, live donor mirror/journal, and their runtime source files are retired. Historical Blog
backfill/migration records remain only for upgrade provenance.

Forum established the first consumer migration precedent and its backend ownership/storage cutover is
complete. TAXONOMY-CAT-5 remains open only for the retained mounted multilingual/RTL browser packet to
be executed against prepared authenticated admin/storefront fixtures; the browser source already
exists and no backend donor/storage cutover remains.

Product PostgreSQL follows the same ownership model and is source-complete through
TAXONOMY-CAT-34. Taxonomy owns canonical Product Category identity, localized copy, routes,
hierarchy and ordering on PostgreSQL. Product retains Product-specific policy/state including
navigation projections, SEO, lifecycle, schema/attribute semantics, product/category membership and
merchandising. PostgreSQL `catalog_category_translations` and `catalog_category_closure` no longer act
as canonical donor/hierarchy storage; non-PostgreSQL backends intentionally retain their donor and
closure compatibility paths until an equivalent tenant-safe cutover is separately designed and
verified.

No TAXONOMY-CAT-35 Product slice or next Category consumer is currently accepted by this plan. A
future consumer migration must be named explicitly and start from fresh `main` with its own typed
binding/backfill/read-write-cutover evidence rather than being inferred from the previous CAT number.

## Why this replaces the current category boundary

The repository historically had a Tag-only Taxonomy contract and separate category aggregates inside
Forum, Blog and Product. That produced multiple implementations of hierarchy, localization,
presentation and translation ownership. It also prevented a tenant from defining one shared category
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
- Blog-specific membership/settings/revision state;
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

The current runtime donors `user`, `product`, `order` and `topic` are intentional extension surfaces.
The list is not automatic: every future donor still needs demonstrated product intent.

- `user`: retained; user/profile extension is a demonstrated use case.
- `product`: retained; merchant-defined catalog attributes are a demonstrated use case, while core
  Product invariants remain normalized.
- `order`: retained only for non-critical extension data; payment/inventory/ledger invariants must
  never move into Flex.
- `forum.topic`: retained as an explicit extension surface. Administrators may add optional custom
  topic fields, while Forum-owned topic lifecycle, category, route identity, moderation, counters,
  authoring and other business invariants remain normalized Forum fields.
- `taxonomy.category`: active explicit Flex donor after TAXONOMY-CAT-4. It is the extension point for
  tenant-specific Category fields beyond the canonical built-ins and reuses the generic Flex
  definition/value/localization/validation/transport boundary.
- groups/profiles and future modules: opt in when their product surface explicitly supports custom
  fields.

For `forum.topic`, the existing `topic_field_definitions`, `forum_topics.metadata` donor payload and
localized attached values remain live Flex infrastructure. They must be migrated toward the same
minimal reusable donor adapter as other consumers rather than removed or expanded into a second
Forum-specific custom-field implementation.

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
- Use the Category onboarding to simplify the donor adapter contract for existing consumers,
  including `forum.topic`, instead of adding another donor-specific service stack.

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

Forum established the first migration precedent. Its backend cutover is complete, with only the
prepared mounted multilingual/RTL browser execution still pending. Blog has completed this consumer migration through
TAXONOMY-CAT-12. Product PostgreSQL completed its canonical localized-copy and hierarchy donor
retirement through TAXONOMY-CAT-34 while deliberately retaining Product-owned navigation/policy state
and non-PostgreSQL compatibility. No later consumer is selected by this plan; selecting one is a
separate accepted planning decision.

## Forum-specific target

After Forum category migration:

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

forum topic
  normalized Forum business fields
  + optional Flex custom fields
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
- `forum.topic` remains registered and its custom fields cannot replace normalized Forum invariants;
- Category Flex definitions/values are tenant-scoped and multilingual where configured;
- consumer bindings reject cross-tenant Taxonomy category references;
- legacy category UUID/data backfill is deterministic and rollback/recovery is documented;
- Forum mounted multilingual/RTL admin/storefront evidence uses Taxonomy-owned category data;
- Product PostgreSQL remains at the CAT-34 ownership boundary without reviving retired translation or
  closure authority, while non-PostgreSQL compatibility remains explicit;
- no consumer reintroduces a local generic custom-fields engine.

## Change rules

1. Do not add a second category owner to solve a consumer-specific UI problem.
2. Do not add a second custom-fields implementation to solve a donor-specific storage problem.
3. Flex support is explicit product opt-in; metadata storage by itself is not opt-in.
4. Taxonomy owns shared Category hierarchy; consumers own only their relationships and domain policy.
5. Flex fields may extend a donor but must not replace normalized domain invariants.
6. Media owns binary lifecycle; Taxonomy/Flex store typed Media references, not copied delivery URLs.
7. Translation for canonical category copy is Taxonomy-owned.
8. Keep migrations staged and data-preserving; never drop legacy category data before a verified cutover.
