# ADR: Taxonomy owns shared Categories; Flex owns runtime custom fields

## Status

Accepted — 2026-08-22

## Context

The original Taxonomy rollout intentionally started with `TaxonomyTermKind::Tag` and kept category
hierarchy inside Blog, Forum and Product. That was a conservative Phase 1 boundary, but subsequent
product work demonstrated that categories are not domain-local vocabulary: tenants need one shared
category library that can be reused across Forum, Blog, Product and future modules.

The current split duplicates category identity, hierarchy, localized copy, presentation and
Translation ownership across modules. It also creates pressure to implement runtime-defined category
fields independently in each consumer.

RusToK already has a separate shared custom-fields capability, `flex`, whose purpose is to let domain
entities opt in to administrator-defined fields without reimplementing definitions, validation,
localization and transport.

## Decision

### 1. `rustok-taxonomy` becomes the canonical Category owner

Add a first-class `TaxonomyTermKind::Category` with Taxonomy-owned:

- stable locale-independent identity and `canonical_key`;
- tenant plus `global | module` scope;
- localized name, slug, description and aliases;
- shared category hierarchy, ordering and cycle/depth rules;
- canonical category presentation such as icon/color and typed Media references;
- revision/change evidence and Taxonomy Translation ownership.

Consumer modules keep only typed relations/bindings and domain-specific state. Taxonomy does not
receive a generic polymorphic `owner_type/owner_id` attachment table.

This decision **supersedes the category-specific part** of
`DECISIONS/2026-03-29-taxonomy-module-scope-aware-terms.md` and any later plan/guardrail text that
requires Taxonomy to stay flat or pins category hierarchy/translation ownership to Forum, Blog or
Product. The existing Tag route/scope/locale/tenant contracts remain valid and should be reused for
Category where applicable.

### 2. `flex` is the only runtime custom-fields capability

Runtime-defined fields are a platform capability, not a per-module implementation detail. Any domain
entity may opt in through a minimal bounded adapter/registration and its owner storage/permission
contract.

A Flex-enabled module must reuse the common field-definition, type validation, localization,
attached-value, cache, transport and admin schema-builder contracts. Module-local replacements are
not allowed merely because a donor needs a new field type or storage shape; reusable behavior belongs
in Flex.

Flex remains opt-in. A `metadata`/JSON column does not automatically make an entity a donor.
Built-in business invariants remain normalized owner fields.

### 3. Taxonomy Category opts in to Flex

After the Category owner exists, register `taxonomy.category` as an attached Flex consumer for
administrator-defined extension fields. Canonical Category fields remain Taxonomy-owned built-ins;
Flex stores only extension fields.

### 4. Forum Topic remains an explicit Flex consumer

Forum Topic is intentionally extensible through Flex. Administrators may define optional topic
fields for tenant-specific workflows, classification or presentation without requiring a Forum code
extension.

This does **not** move Forum business invariants into Flex. Topic lifecycle/status, category binding,
route identity, authoring content, moderation state, counters, accepted-solution semantics, access
policy and other Forum-critical state remain normalized Forum-owned fields.

The existing `topic_field_definitions`, `forum_topics.metadata`, attached localized values and
`FieldDefRegistry` registration remain live. Future work should simplify Topic onboarding toward the
same minimal reusable Flex donor adapter used by other consumers, not remove Topic support or build a
Forum-specific custom-field engine.

## Consequences

Positive:

- one category identity/hierarchy/localization/presentation model can be reused across modules;
- one Translation owner handles canonical category copy;
- one Flex capability extends Categories, Products, Users, Topics, Groups and other explicit donors;
- adding custom fields to a new module becomes adapter work instead of another field engine;
- Forum Topic can support tenant-specific extension without polluting the normalized topic schema;
- module-specific policy remains in the module that understands it.

Tradeoffs:

- existing Forum/Blog/Product category tables require staged migration/backfill;
- historical Taxonomy ownership guardrails and contract-matrix fixtures must be rewritten with the
  Category implementation;
- Product and Forum have category-specific policy/projection state that must become bindings rather
  than being blindly moved into Taxonomy;
- Flex donor onboarding still has too much per-donor plumbing and should be reduced using
  `taxonomy.category` as the next reference integration.

## Migration constraints

- Preserve existing category UUIDs when possible.
- Do not drop local category translations/hierarchy until Taxonomy backfill and consumer read/write
  cutover are verified.
- Do not register a duplicate `forum/category`, `blog/category` or `product/category` Translation
  provider after Taxonomy is canonical.
- Keep consumer relation/binding tables tenant-safe and module-owned.
- Media remains the binary lifecycle owner; Taxonomy/Flex reference Media identities.
- Topic Flex fields must remain optional extension data and must not replace normalized Forum
  invariants.
- Update focused source/runtime guards atomically with each ownership cutover.

## Verification

The completed migration must prove:

- Taxonomy Category route/scope/tenant uniqueness and hierarchy cycle/depth/order semantics;
- exact locale/fallback behavior with `effective_locale` preserved;
- Taxonomy Translation CAS/progress/change-cursor behavior for Category;
- cross-tenant consumer category bindings are rejected;
- `taxonomy.category` Flex definitions/values are tenant-scoped and localized when configured;
- `forum.topic` remains a registered Flex donor and unsupported entity types still fail closed;
- Topic custom fields cannot overwrite or substitute normalized Forum business fields;
- Forum mounted multilingual/RTL category surfaces consume Taxonomy-owned localized category data.

## Follow-up plan

See `docs/architecture/taxonomy-flex-category-platform-plan.md` for the staged implementation and
consumer migration sequence.
