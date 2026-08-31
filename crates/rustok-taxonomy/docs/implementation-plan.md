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

### TAXONOMY-CAT-3 — canonical Category presentation — COMPLETE

PR #3682 added `taxonomy_category_presentations` as canonical Taxonomy-owned storage with:

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

Retained focused evidence:

- final head `c397dda2bc04a32974077dd2dbfd418797b56e1a` passed `Taxonomy Ownership Boundary` and
  `Taxonomy Lookup Contract`;
- exact-head `Taxonomy PostgreSQL Evidence` run `32572959402` passed the canonical PostgreSQL 16
  migration graph plus direct Category presentation storage guard/same-revision CAS evidence;
- PR #3682 was squash-merged as `7bb105d10fc99cb5271d008d3cb62395dee5cacf`.

### TAXONOMY-CAT-4 — Flex Category donor — COMPLETE

PR #3683 delivered the reusable backend donor foundation. It intentionally extends Flex rather than
adding a Taxonomy-specific custom-fields engine:

- `taxonomy.category` is a namespaced Flex entity type;
- Flex owns generic attached field-definition persistence keyed by tenant and donor entity type;
- Flex owns optional generic shared attached values while reusing the existing localized-value store;
- the server field-definition registry registers `taxonomy.category` when Taxonomy is compiled;
- the server value adapter validates the real owner identity as the same-tenant
  `TaxonomyTermKind::Category` before reading or writing generic Flex rows;
- shared and localized prepared writes/deletes are committed atomically through a host transaction;
- generic definitions reuse the existing validation, event and durable cache-generation contracts;
- exact-locale authoring remains separate from read fallback, so editing one locale cannot seed
  another locale with fallback text.

PR #3684 completed the real Category instance boundary through the existing generic Flex surface:

- Flex owns the attached-value GraphQL read/update/delete port and transport types;
- tenant identity and tenant-default locale come from trusted GraphQL context rather than caller
  payload;
- the server adapter rejects Tags, foreign-tenant Categories and stale UUIDs before any attached Flex
  value read/write;
- shared and localized Category values resolve through the existing generic Flex schema/validation
  path, including requested-locale to tenant-default fallback;
- Taxonomy hard-delete invokes an injected cleanup port inside the owner transaction, while Flex alone
  deletes its generic shared/localized value rows;
- Taxonomy still owns no duplicate field-definition service, validator, localized-value engine or
  custom form/schema-builder implementation.

Retained focused evidence:

- PR #3683 final head `532444698ee3d1451603bd734ef3ef308c718044` passed `Taxonomy Category Flex Donor Contract`
  run `32587184715` and `Taxonomy PostgreSQL Evidence` run `32587184591`, then squash-merged as
  `5f063e5fcc56fa1af7859ede19aa3b345f05d218`;
- PR #3684 exact head `1c9e79a790cac08005b82a73aa44c98a5194f5c0` passed `Taxonomy Category Flex Donor Contract`
  run `32597360518`: source boundary, scoped Rust 1.96 formatting, generic definition/value contracts,
  bounded Taxonomy owner identity, server host compile, generic PostgreSQL donor roundtrip and the
  real Category PostgreSQL transport/hard-delete E2E all succeeded;
- the same #3684 head passed `Taxonomy Ownership Boundary` run `32597360409` and complete
  `Taxonomy PostgreSQL Evidence` run `32597360469`, including canonical PostgreSQL 16 migrations,
  Category presentation CAS, hierarchy contention, route-registry contention, Translation-target
  evidence and the final gate;
- PR #3684 was squash-merged as `4ea8a0362ef9210294750c4c9766787a7191914f`.

**Done:** a tenant can add, edit, localize, resolve and remove custom fields on real Categories through
the platform Flex transport/schema-builder boundary, with tenant/kind ownership and hard-delete
cleanup proved, while Taxonomy implements no second field-definition service, validator or custom
form engine.

### TAXONOMY-CAT-5 — Forum category cutover — IN PROGRESS

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

**Backend cutover status: COMPLETE.** Forum Category canonical identity, localized copy, routes,
aliases, hierarchy, sibling ordering and presentation are Taxonomy-owned. Forum retains typed
membership/binding plus Forum-specific policy, counters, moderation, subscriptions and lifecycle.
The duplicate Forum Category Translation provider is retired, canonical reads/writes/search no longer
consume the donor tables, and migration `m20260824_000031_retire_forum_category_legacy_storage`
removes the old translation/route-alias/change-cursor tables only after a fail-closed same-ID
Taxonomy ownership preflight.

Accepted CAT-5 slices already in `main`:

- PR #3686 added the tenant-safe one-to-one Forum → Taxonomy Category binding seam; PR #3688 added
  deterministic same-UUID backfill of localized copy, routes/aliases, hierarchy and presentation;
  PR #3689 exposed the bounded Taxonomy Category owner projection; PR #3690 added transactional
  Taxonomy Category owner-sync.
- PR #3691 moved Forum Category commands to transactional Taxonomy dual-write. PR #3693 moved public
  Category get/list reads to the Taxonomy projection. PR #3695 exposed the module route-match
  projection used by the Forum route cutover. PR #3696 moved Category tree reads and public mutation
  responses to Taxonomy-backed projections.
- PR #3697 moved Category route reads to the Taxonomy route registry. PR #3698 made Taxonomy the
  authoritative route-write/collision owner. PR #3699 moved append-only Category alias history into
  Taxonomy so Forum stopped reading/writing `forum_category_route_aliases` at runtime.
- PR #3700 removed `forum_category_translations` as a command-copy/placement donor: create/update use
  command input plus exact Taxonomy copy, while move/reorder synchronize structure over Taxonomy-owned
  copy. PR #3701 retired `ForumCategoryTranslationTargetProvider`, Forum Category Translation
  progress/change-cursor ownership and switched operator locale enumeration to Taxonomy.
- PR #3702 moved Search Category locale/candidate enumeration to typed bindings plus Taxonomy
  `available_locales`. PR #3703 moved public Category cursor-page materialization, ordering,
  hierarchy and presentation to a Taxonomy-backed read-model owner wrapper.
- PR #3704 stopped canonical create/update/import writes to the legacy Forum translation mirror.
  PR #3705 removed more than one thousand lines of dead private Category CRUD/locale/tree runtime and
  left the old Category persistence module only with transaction helpers still needed by Forum-owned
  policy/counter/import workflows.
- PR #3706 added irreversible legacy-storage retirement after the deterministic backfill. The
  migration requires every current Forum Category to have a same-ID, same-tenant Taxonomy Category in
  `module/forum` scope, removes obsolete cross-table route guards, then drops
  `forum_category_route_aliases`, `forum_category_translations` and `forum_translation_changes`.
- PR #3708 added the retained mounted multilingual/RTL Playwright evidence source, dedicated config,
  machine-readable execution contract and source verifier. Its contract remains
  `source_ready_maintainer_execution_pending`: it does not claim browser execution or CAT-5
  completion.

Retained focused evidence for the foundation:

- PR #3686 exact head `7e64eb9d3fc3af4d8e7b7ec1063cf45ea8859c58` passed `Forum Taxonomy Category Binding Contract`
  run `32659255474` and `Migration Compatibility` run `32659255365`, then squash-merged as
  `9af3a113c28ecb964dd9ff1737a7ddd69e916f23`;
- PR #3688 exact head `70f2ca9778fa4bd472e6b7f2e2c980013ba2dbef` passed
  `Forum Taxonomy Category Backfill Contract` run `32665020688` and `Migration Compatibility` run
  `32665020664`, then squash-merged as `feeaa0b5a16ea2898fb7b7d222e7b30d55605870`;
- PR #3689 exact head `4ee5fa31bd9f96bada3cb6aea6c3dd8ef846fc53` passed `Taxonomy Lookup Contract` run
  `32668871962`, `Taxonomy Ownership Boundary` run `32668871968` and `Taxonomy PostgreSQL Evidence`
  run `32668871894`, then squash-merged as `5f9c875ab91ec536a08a6b6ec60de7bf315da6f6`;
- PR #3690 exact head `3c33324b2d6389295eb4fe6f0d0a035ad1905f67` passed
  `Taxonomy Category Owner Sync Contract` run `32683541704`, `Taxonomy Ownership Boundary` run
  `32683541614`, `Taxonomy Lookup Contract` run `32683541624` and `Taxonomy PostgreSQL Evidence` run
  `32683541607`, then squash-merged as `0b09edd2a07f19cd0e8cf4820681a3ed73d09c09`.

Later CAT-5 runtime slices include focused source/integration contracts. PR #3708 provides the final
browser runner source, but its merge does not substitute for the still-required mounted browser
parity execution against the prepared Taxonomy-backed fixture.

**Next:** execute the retained PR #3708 mounted multilingual and RTL Forum Category browser packet
against a prepared authenticated admin/storefront fixture, including requested/effective locale
fallback, canonical localized routes, hierarchy/order, presentation and alias redirect. No backend
donor/storage cutover remains.

**Done when:** the backend ownership/storage cutover above remains intact and a successful mounted
multilingual/RTL browser run confirms that Forum Category behavior uses the shared Taxonomy
identity/copy/routes without losing Forum-specific policy.

### Blog consumer cutover — COMPLETE

The Blog Category source/storage cutover is complete through TAXONOMY-CAT-12. Canonical localized
copy, route history, hierarchy projection and Translation ownership are Taxonomy-owned through the
same-ID Blog-to-Taxonomy binding; Blog retains only its typed binding plus module-specific
membership/settings/revision state. The former `blog/category` provider, Blog Category change
journal and Blog-local donor translation storage are retired. The owner-scoped Blog documentation
cursor is actualized through TAXONOMY-CAT-17.

### TAXONOMY-CAT-6 — Product and later consumers — IN PROGRESS

The PostgreSQL Product Category canonical/hierarchy donor cutover is source-complete through
TAXONOMY-CAT-34. CAT-6 remains open for explicitly accepted later-consumer work and for any future
backend-bounded Product compatibility retirement; it does not imply that retained Product domain
state should move into Taxonomy.

Accepted Product slices already in `main`:

- PR #3735 / TAXONOMY-CAT-23 added the PostgreSQL tenant-safe one-to-one
  `product_catalog_category_taxonomy_bindings` seam without runtime cutover;
- PR #3736 / TAXONOMY-CAT-24 added deterministic same-ID Product Category backfill into Taxonomy,
  including localized canonical copy, route ownership, hierarchy and binding-last fail-closed checks;
- PR #3737 / TAXONOMY-CAT-25 closed the post-backfill creation gap by synchronizing every new Product
  Category locale/hierarchy into Taxonomy inside the existing Product transaction before binding,
  event and commit;
- PR #3738 / TAXONOMY-CAT-26 moved the PostgreSQL Product Category list canonical `name`, localized
  `slug` and `parent_id` reads to `TaxonomyOwnerCategoryReader`, while Product retains `code`, `kind`,
  `path` and path ordering. Missing/mismatched binding or owner state fails closed;
- PR #3739 / TAXONOMY-CAT-27 isolated Product-only localized `meta_title` / `meta_description` into
  PostgreSQL `catalog_category_seo_translations`, deterministically backfilled legacy SEO, and kept
  new SEO writes in the same Product transaction before Taxonomy owner-sync;
- PR #3740 / TAXONOMY-CAT-28 stopped new PostgreSQL Product Category creates from writing
  `catalog_category_translations`, while retaining non-PostgreSQL donor reads/writes and keeping
  Product SEO plus Taxonomy owner-sync atomic;
- TAXONOMY-CAT-29 physically retired PostgreSQL `catalog_category_translations` only after same-ID,
  same-tenant Taxonomy identity/locale coverage and exact Product-owned SEO parity proved the donor
  safe to drop. Non-PostgreSQL backends keep the legacy donor read/write compatibility path;
- TAXONOMY-CAT-30 moved PostgreSQL effective-form/schema inheritance and inherited attribute-group
  label ancestry to the Taxonomy Category hierarchy while retaining Product schema/attribute policy;
- TAXONOMY-CAT-31 moved the PostgreSQL schema-directory result order to Taxonomy parent/position
  ordering while retaining Product `path` as a navigation projection;
- TAXONOMY-CAT-32 stopped new PostgreSQL Product Category creates from materializing closure rows;
- TAXONOMY-CAT-33 retired the historical PostgreSQL closure-parity commit invariant while preserving
  Product parent-cycle rejection and truthful one-step rollback reconstruction;
- PR #3746 / TAXONOMY-CAT-34 physically retired PostgreSQL `catalog_category_closure` after the
  hierarchy-consumer, write and invariant cutovers. The focused storage/write/invariant gates and
  Migration Compatibility fresh/N-1 matrix passed on exact head before squash merge as
  `698684e94fbbe273b6b29209aee221d77525bcbc`.

**Current Product cursor: TAXONOMY-CAT-34.** On PostgreSQL, Taxonomy is the canonical Category
identity/localized-copy/route/hierarchy/order owner. Product no longer retains a canonical Category
translation donor or closure storage authority on that backend.

Product continues to own `code`, `kind`, virtual rule state, activation/soft-delete lifecycle,
Product-specific metadata, localized SEO, schema/attribute definitions and assignments,
product/category membership, merchandising semantics and the `parent_id` / `path` / `level`
projections retained for Product navigation/lifecycle contracts. CAT-30/31 changed the canonical
PostgreSQL hierarchy/order source; they did not transfer those Product business contracts into
Taxonomy.

Non-PostgreSQL backends intentionally retain `catalog_category_translations` donor reads/writes and
`catalog_category_closure` hierarchy compatibility until an equivalent tenant-safe Taxonomy cutover
is separately designed and verified.

**Next:** no TAXONOMY-CAT-35 Product slice is currently defined or accepted. Do not infer a new donor
retirement merely from the CAT-34 number. Any next Product slice must start from fresh `main`, name an
explicit backend and ownership boundary, and preserve the Product-owned contracts above. Later
Category consumers continue one at a time under the same typed-binding/backfill/read-write-cutover
evidence rules.

**Product PostgreSQL done when:** the CAT-34 state remains green under focused and migration
compatibility evidence, with Taxonomy canonical ownership and Product policy/navigation ownership
both preserved. CAT-6 as a whole remains open only for separately accepted later-consumer or
backend-compatibility work.

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
Category presentation storage guards/CAS, Category hierarchy concurrency, route-registry contention
and Translation CAS/cursor behavior.

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
- execute Category presentation storage-guard and same-revision CAS evidence;
- execute Category hierarchy contention evidence;
- execute route-registry contention evidence;
- execute Translation-target CAS/change-cursor evidence;
- archive exact-head metadata and logs.

### Recorded route-registry contention evidence

The route-registry contention test target is
`crates/rustok-taxonomy/tests/route_registry_contention_postgres.rs`. The test proves
two-writer route-key contention under real PostgreSQL 16 lock semantics: two independent writer
connections both complete the route preflight before one is released, contention is forced after
the translation row pre-lock, and the route registry primary key is the storage authority that
ensures exactly one writer commits. The losing writer reports concurrent route claim; its
translation update rolls back. The winner's translation and route reservation commit together, and
exactly one durable route owner remains. This is the translation apply CAS boundary for route key
ownership.

Recorded runtime evidence runs:

- Final exact-head pull-request run `32708155467` (HEAD `a102c224888459ddab8ab4875083b656e97a56f3`):
  source boundary, route contention harness, translation apply CAS, and the gate all succeeded.
- Post-merge main run `32712523041` (HEAD `e8d228cd1bd74a3ad42d6a9947114024896daeee`):
  canonical PostgreSQL 16 migrations, route-registry contention, translation apply CAS and gate
  all succeeded. Result 4 is complete for the current runtime input fingerprints.

The `evidence.json` runtime input fingerprints record the exact git object SHAs for all runtime
inputs at the time of the post-merge main run. When any of these inputs changes, the verifier
requires fresh PostgreSQL evidence to be collected and the fingerprints updated.

## Verification

Focused commands for the Category program:

- `cargo xtask module validate taxonomy`
- `node scripts/verify/verify-taxonomy-ownership-boundary-self-test.mjs`
- `node scripts/verify/verify-taxonomy-ownership-boundary.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.test.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.mjs`
- `node scripts/verify/verify-taxonomy-category-flex-donor.mjs`
- `cargo test --locked -p rustok-taxonomy --test category_hierarchy --test category_presentation --test localized_route_lookup --test route_key_registry -- --nocapture`
- `cargo test --locked -p rustok-taxonomy --test owner_identity -- --nocapture`
- `cargo test --locked -p flex --test generic_attached_definitions --test generic_attached_storage -- --nocapture`
- `cargo test --locked -p rustok-taxonomy --test category_presentation_postgres -- --nocapture` with `RUSTOK_TAXONOMY_TEST_DATABASE_URL` set to PostgreSQL;
- `cargo test --locked -p flex --test postgres_generic_attached_storage -- --ignored --nocapture` with `RUSTOK_FLEX_TEST_POSTGRES_URL` set to PostgreSQL;
- `cargo test --locked -p rustok-taxonomy --lib`
- PostgreSQL commands retained in `.github/workflows/taxonomy-postgres-evidence.yml` and
  `.github/workflows/taxonomy-category-flex-donor-contract.yml`.

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
