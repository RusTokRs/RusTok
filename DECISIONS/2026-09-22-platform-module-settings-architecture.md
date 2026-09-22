# Platform and module settings architecture

- Date: 2026-09-22
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform-architecture, rustok-modules, configuration-owning modules
- Extends: 2026-08-20-static-module-lifecycle-revision, 2026-08-06-module-release-rollback-safety, 2026-04-05-multilingual-db-storage-parallel-localized-records
- Supersedes: None
- Superseded by: None

## Context

RusToK currently has several mechanisms called settings or configuration:

- deployment YAML/environment configuration loaded by the server host;
- tenant `platform_settings` JSON categories;
- static/native module settings declared by `rustok-module.toml` and stored in
  `tenant_modules.settings`;
- dynamic artifact settings stored under admitted installation and settings-instance
  identities;
- owner-specific configuration such as Search, event-delivery, Iggy, SEO, and tenant
  locale policy;
- localized static-setting values stored outside the base JSON document;
- UI copy that describes settings to an operator.

These mechanisms do not yet form one complete contract. Static settings survive a
tenant disable because the `tenant_modules` row is retained, but re-enable does not
first prove that the retained value satisfies the active package schema. The current
static rollout guard receives the active schema as both predecessor and candidate, so
it does not prove N/N+1 compatibility. Generic `platform_settings.schema_version` is
always written as `1`, several categories have no complete typed validator or runtime
consumer, and the generic email category has historically admitted secret material.

Cross-module behavior also needs an explicit three-way distinction. An unconditional
dependency controls module lifecycle. A consumer-owned integration setting expresses
tenant intent to use a provider. A contextual capability requirement exists only for
one entity or operation. Effective availability is a derived policy decision; it is
not the stored boolean/domain fact and it is not permission for a consumer to read the
provider's private settings or `tenant_modules` row.

Settings labels and help copy, localized business copy stored as a setting value, and
locale-scoped behavior are three different concerns. Treating all three as translated
JSON would create ambiguous fallback, leak sensitive values, and make schema upgrades
unsafe.

The platform therefore needs one architecture that preserves owner boundaries while
standardizing identity, lifecycle, compatibility, localization, and verification.

## Decision

RusToK adopts a single settings contract with owner-specific persistence and behavior.
There is no universal untyped settings authority.

Every mutable setting belongs to exactly one semantic owner. The owner publishes a
typed schema or command contract, validates and normalizes writes, owns runtime
resolution, and defines restart/reload, event, cache, and migration semantics. Shared
platform code may provide schema validation, revision, receipt, localization, secret,
and transport primitives, but it does not acquire the setting's business semantics.

The canonical vocabulary is:

| Term | Meaning |
| --- | --- |
| deployment configuration | Host/node/process input selected before runtime composition; not tenant data |
| setting | Durable operator or tenant intent owned by one platform or domain owner |
| normalized settings document | Complete owner-validated value persisted for one exact schema digest |
| settings schema digest | Canonical digest of the exact schema that admitted the normalized document |
| settings revision | Monotonic owner concurrency revision, not a package version |
| integration policy | Consumer-owned intent for an optional provider relationship |
| effective integration decision | Derived result of consumer intent, provider availability, module/channel policy, and authorization |
| localized setting value | Explicitly admitted translatable string leaf stored outside the locale-neutral base document |
| settings UI copy | Fluent message IDs and bundles describing fields, options, help, and validation; never business data |
| locale-scoped overlay | Typed behavioral configuration whose meaning differs by locale; not a translation row |
| secret handle | Opaque reference to secret material owned by the canonical secret boundary |

### Settings classes and owners

| Class | Canonical owner | Persistence rule |
| --- | --- | --- |
| Deployment configuration | Host or deployment component that consumes it | Typed config snapshot; environment/YAML is an input adapter |
| Global operational control plane | Owning platform component | Owner-specific durable state with explicit activation/restart semantics |
| Tenant platform setting | Owning platform/domain module | Owner-specific typed aggregate; generic storage is allowed only as an owner-neutral adapter behind that owner |
| Static/native module setting | `rustok-modules` for lifecycle/persistence, declaring module for semantics | Static tenant settings aggregate plus exact package schema digest |
| Dynamic artifact setting | `rustok-modules` artifact control plane | Stable data-owner and settings-instance identity plus admitted immutable schema digest |
| Domain configuration | Domain module | Domain table/aggregate when configuration participates in domain invariants |
| User preference | Preference/profile owner | User-scoped typed state; never mixed into tenant module settings |
| Secret material | Secret owner/provider | Secret bytes remain outside settings; settings persist only a handle and safe status facts |

`apps/server` remains a composition and transport host. It must not own business
validation merely because a GraphQL resolver or YAML adapter is located there.

### Static/native settings aggregate

For a static/native module, the canonical owner boundary is the `rustok-modules`
service. The durable aggregate comprises:

- `module_static_tenant_lifecycle` for the monotonic revision and active execution
  claim;
- `tenant_modules` for explicit tenant enablement intent and the normalized settings
  document;
- an exact `settings_schema_digest` and a settings state of `not_applicable`, `ready`,
  or `migration_required`;
- localized value rows, source-locale provenance, and change cursors when the package
  admits localized fields;
- owner-operation receipts and transactional outbox facts.

These records change through one owner transaction. No consumer module or host
transport may read or write `tenant_modules` directly to derive settings behavior.

A settings-bearing module materializes a complete normalized document when settings
are first written or the module is first enabled. Defaults are therefore initial
materialization inputs, not a forever-live fallback layered under a persisted partial
override. A later package default change does not silently change a tenant's already
materialized behavior. It requires an explicit compatible migration, an operator
change, or a documented owner policy that is itself represented in the schema.

A module with no settings persists `not_applicable`; it does not create an empty
synthetic settings authority.

### Lifecycle semantics

| Operation | Required behavior |
| --- | --- |
| Disable | Stop effective serving/work through module policy; retain settings, localized values, domain data, schema identity, and audit history |
| Edit while disabled | Allowed through the owner when the installed/admitted schema is available; it changes dormant intent and causes no runtime side effect |
| Enable | Validate and normalize the retained or explicitly supplied document against the active schema before any pre-enable hook; commit settings, enablement, revision, receipt, and policy/outbox facts atomically |
| Re-enable after package update | Run the same active-schema and dependency/integration preflight; never pass stale unvalidated JSON to a lifecycle hook |
| Uninstall/remove from composition | Retain native/static authoritative settings and data unless an owner-specific destructive contract exists; absence from composition is not purge authority |
| Reset | Replace the document with freshly normalized current defaults through an explicit revision-CAS command; reset is not purge |
| Purge | Separate privileged preview/apply operation with retention and recovery evidence; dynamic artifact settings follow the accepted artifact purge contract, while no generic native/static purge is promised |

The owner may expose one reviewed enable-with-settings command so an invalid dormant
document can be repaired atomically. UI must not force an operator to enable invalid
settings merely to gain permission to edit them.

### Package update and rollback

Every settings-bearing release carries an immutable settings schema and canonical
digest. Update preflight loads the actual predecessor schema, candidate schema, and
current normalized document.

Automatic update is allowed only when the current value is accepted by both N and
N+1 and every intermediate reader/writer/transport/worker state is compatible. Before
mixed serving begins, the owner persists a compatibility guard bound to both schema
digests and the rollback-window identity. Every concurrent settings write revalidates
against the real intersection and CASes the same owner revision.

The following changes require explicit maintenance unless compatibility is proven by
the schemas without transforming the current document:

- removing or renaming a persisted key;
- changing a value type or semantic unit;
- narrowing an enum/range below a current value;
- adding a required value without an admissible deterministic default;
- changing sensitivity, secret-handle, localization, or scope classification;
- changing an optional integration policy in a way that invalidates provider state;
- disjoint predecessor and candidate schemas.

Maintenance migration fences settings writers and module work, creates the required
recovery point, produces one exact normalized target document, records an idempotent
receipt, and advances a monotonic point of no return before a one-sided write. It does
not install an arbitrary module-supplied transform executable into lifecycle. Removing
unknown keys silently or falling back to old decoders is forbidden.

Rollback never restores settings independently of code. It is eligible only when the
resulting document is accepted by the predecessor or when a separately authorized
settings recovery operation restores a compatible retained snapshot under the accepted
release-safety contract.

### Dependencies, integrations, and contextual capabilities

Static dependencies in `modules.toml`, `rustok-module.toml`, and
`RusToKModule::dependencies()` are reserved for the minimal unconditional kernel: the
consumer cannot enable or provide any valid core operating mode without the provider.
Compile linkage, an optional UI/feature, or a provider needed by only some domain data
does not justify a static edge. Enabling a module requires only this minimal closure.

Tenant integrations and contextual capability requirements are not static dependency
edges. The consumer owns a typed integration policy, preferably an enum when degraded
and required behavior differ. When the requirement varies by entity or operation, its
domain owner persists a typed fact/snapshot instead of a generic module setting. The
effective decision is derived from:

1. consumer policy or typed contextual requirement;
2. consumer and provider effective module policy;
3. provider registration/readiness;
4. channel binding and other relevant context;
5. authorization and maintenance state.

The derived decision is exposed through an owner-defined port or policy projection. A
consumer must not query another module's `tenant_modules` row or private settings.
Provider absence affects only the feature or operation that requires it; it does not
disable unrelated consumer behavior. Provider disable is rejected only for a genuine
active required binding, in-flight operation, or retained owner invariant, not merely
because the consumer module is enabled.

Every static dependency must carry owner-reviewed evidence that no provider-free core
mode exists, that the provider is required by the whole consumer rather than one
feature or data subtype, and that disable/retention behavior is defined. Mechanical
agreement among manifests and runtime metadata is necessary but not semantic proof.
This decision classified the two over-constraints below; it does not certify any other
existing static edge without the same review. The Blog edge has since been removed;
the Commerce edge remains open implementation work.

Blog publications, categories, and tags are valid without Comments. The former static
`blog -> comments` lifecycle edge has been removed. Blog owns
its comments-surface policy; Comments owns threads, bodies, and moderation. An enabled
or required Blog comments mode may require Comments for comment operations, while Blog
itself remains available when that capability is disabled or unavailable according to
the selected policy.

For Blog/Forum and Reactions, Reactions can remain optional. A consumer setting selects
the desired engagement behavior, while effective availability additionally requires
the Reactions owner and the producer provider to be available. Missing required
capability fails closed; an explicitly documented optional mode may hide or degrade the
surface without changing the stored intent. Degradation must not fabricate an empty or
zero reaction state. Blog and Forum never enable or disable the Reactions module; they
own only their respective surface policies.

Digital products and digital order lines do not require shipping or Fulfillment.
Physical lines carry a typed fulfillment requirement into cart/order snapshots, and a
mixed cart forms delivery groups only for those lines. The current static
`commerce -> fulfillment` edge is likewise a cutover gap: Commerce remains available
for digital-only flows, while physical checkout fails closed if its required
Fulfillment capability is unavailable. Current source already admits
`product_type = "Digital"`, but several Product/Cart/Commerce paths synthesize the
`default` shipping profile when none is present; that fallback is part of the same
zero-legacy cutover and must not survive for digital lines.

### Localization

Settings localization has three separate contracts:

| Concern | Storage and resolution |
| --- | --- |
| Admin field/option/help copy | Stable Fluent message IDs declared by the module settings presentation metadata and resolved from module-owned UI bundles using the host effective locale |
| Translatable business copy stored as a setting | Explicit stable field IDs mapped to admitted string leaves; exact-locale parallel rows, source-locale provenance, target CAS, sensitivity fences, and Translation owner ports |
| Locale-dependent behavior | Typed locale-scoped overlay with explicit fallback and cache identity; never stored as translated copy |

Schema keys, enum values, booleans, numbers, URLs, identifiers, secret handles, and
policy selectors are locale-neutral. A field is not translatable merely because its
Rust/JSON type is `string`.

Localized fields must opt in with a stable field ID. Sensitive paths and descendants
are never localizable. Fallback is a read projection only and never counts as exact
translation coverage. Removing or renaming a localized field requires a migration that
accounts for its exact locale rows and change history.

### Secrets and sensitive values

Settings documents must not contain plaintext passwords, private keys, tokens, or
provider credentials. The owner stores an opaque secret handle plus non-sensitive facts
such as configured state and handle revision. Secret set/rotate/clear are separate
write-only commands. Reads never return placeholder secret bytes whose empty/non-empty
meaning controls preservation.

Sensitivity metadata is mandatory for any path containing private operational data and
is enforced by validation, localization, audit, event, export, snapshot, and UI layers.

### Events, projections, and runtime activation

A successful settings mutation atomically commits:

- the normalized document and schema digest;
- the owner revision and cleared execution claim;
- the idempotency receipt;
- a content-free settings-changed outbox fact when downstream reconciliation is
  required.

Events identify owner, scope, schema digest, revision, and safe changed-field IDs; they
do not carry settings values or secrets. Consumers reload through the owner read port
and use revision-aware idempotent reconciliation. Cache, search, worker, schedule, and
runtime reload behavior must be declared by the setting owner.

Each setting declares one activation mode: immediate, reconciled asynchronous,
restart-required, next-request, or next-enable. A successful save must not claim that
runtime behavior changed when the active runtime still uses bootstrap configuration.

## Sources of truth and ownership

- The semantic owner is the module/component that consumes and enforces the setting.
- `rustok-modules` owns static/dynamic module settings lifecycle primitives and
  persistence boundaries, not every module's business meaning.
- The active package descriptor owns schema identity; persisted normalized state owns
  the selected value.
- Effective integration and runtime activation views are derived projections.
- YAML/environment values are deployment inputs, not hidden tenant fallbacks once a
  durable owner setting exists.
- UI schemas, GraphQL DTOs, caches, Translation resources, and audit views are
  projections/adapters, not alternate write authorities.

## Invariants

### Allowed states

- No materialized document for a never-configured, never-enabled module.
- A `ready` document bound to the exact active schema digest.
- A retained dormant `ready` document while the module is disabled.
- `migration_required` while a retained document cannot be admitted by the active
  schema; the module remains non-serving until repaired.
- A real N/N+1 compatibility guard during a bounded rollout window.
- Localized exact-locale rows tied to stable field IDs and owner revisions.
- An enabled consumer with an unavailable optional provider while provider-free core
  behavior remains available and only affected operations are denied/degraded.
- A digital-only Commerce flow with no Fulfillment module or fulfillment rows.

### Forbidden states

- Enabled settings-bearing module with an absent, unknown, or unvalidated schema
  digest.
- Lifecycle hook execution with stale settings that failed active-schema validation.
- Consumer direct reads of another owner's settings or tenant-module persistence.
- Stored integration intent treated as proof of provider availability.
- Static dependency edge justified only by an optional feature, UI contribution, or a
  subset of domain entities/operations.
- Synthetic shipping profile, delivery group, or fulfillment row for a digital line.
- Settings event committed best-effort after the owning transaction.
- Plaintext secret material in settings JSON, logs, events, receipts, translation rows,
  or exports.
- Localized enum/config identifiers or implicit translation of every string.
- Unknown keys silently ignored, removed, or preserved through a compatibility decoder.
- Unknown schema keywords silently ignored while the UI or runtime appears to enforce
  them.
- Last-write-wins mutation without an explicit accepted owner contract.

## Non-goals

- This decision does not require one physical table for every settings class.
- It does not turn domain configuration into generic JSON when relational invariants or
  queries require owner tables.
- It does not make deployment configuration hot-reloadable by default.
- It does not add a generic native/static data purge contract.
- It does not make every optional module relationship configurable.

## Data, transaction, and concurrency boundary

Static settings, enablement intent, owner revision, receipt, and required outbox facts
commit atomically under the `(tenant_id, module_slug)` lifecycle claim. Artifact
settings use their accepted installation/settings-instance aggregate. Owner-specific
platform/domain settings use an equivalent owner revision and receipt boundary.

All mutable writes require authenticated actor/tenant context, a non-nil idempotency
identity, and expected revision. Database uniqueness and tenant-composite foreign keys
enforce scope identity. JSON shape validation supplements rather than replaces
relational constraints needed by the domain.

## Context dimensions

- Tenant is mandatory for tenant settings and absent only for explicitly global
  operational settings.
- Channel participates only through a declared channel overlay/policy projection.
- Locale participates in UI copy, localized-value reads, or typed locale overlays; it
  is not inferred inside a consumer.
- Principal and permission context are mandatory for writes and sensitive reads.
- Policy, maintenance, release, trace, correlation, and idempotency identities are
  propagated through owner commands.
- Timezone, currency, and unit dimensions are explicit when they alter meaning.

## Events and projections

Settings-change events use the canonical transactional outbox. Effective policy,
localized Translation resources, caches, schedules, and operator views are rebuildable
or reconcilable projections with revision/cursor semantics. A projection failure does
not create a second source of truth and must be observable and retryable.

## Failure semantics

Invalid, stale, incompatible, unauthorized, or dependency-incomplete settings fail
closed with actionable typed diagnostics. A disabled or unavailable optional provider
uses only the consumer owner's documented fail-closed or degraded mode. Unknown runtime
activation outcome is reported as pending/reconciliation-required, not success.

## Migration and cutover

Implementation performs a zero-legacy cutover:

1. Inventory every persisted/read setting, runtime consumer, and static dependency
   edge; classify whether the provider is unconditional, tenant-selected, or required
   only by specific domain data/operations.
2. Delete decorative settings with no canonical consumer or implement their owner path.
3. Freeze one manifest schema vocabulary, reject unknown schema keywords, and migrate
   current `shape`/`additional_properties` declarations to the canonical typed form.
4. Move generic platform categories to their semantic owners; remove unsupported
   categories and fallback chains.
5. Add schema digest/state and materialize normalized static settings atomically.
6. Replace direct cross-module `tenant_modules` reads with owner ports/effective
   decisions.
7. Add the missing Blog-owned comment-surface policy over the optional Comments port.
   Remove the remaining `commerce -> fulfillment` edge after introducing typed
   capability requirements; migrate callers, UI, tests, and composition atomically.
8. Install real predecessor/candidate compatibility guards and maintenance migration.
9. Move secrets to handles and scrub/rotate historical material.
10. Add Fluent presentation metadata and opt-in localized value metadata.
11. Rename non-canonical internal keys to `snake_case` atomically across schema, runtime,
   UI, fixtures, and docs.

There are no compatibility aliases for current internal camelCase keys. Persisted
development data is migrated deterministically or explicitly classified as disposable.

## Alternatives considered

### One global JSON settings table

Rejected because it centralizes storage while leaving validation, runtime behavior,
secrets, concurrency, and migration ownerless.

### Delete settings when a module is disabled

Rejected because disable is reversible availability intent, not destructive data
authority.

### Keep partial overrides and always merge current package defaults

Rejected because a package update would silently change tenant behavior and make
rollback/configuration evidence ambiguous.

### Store all localized variants inside settings JSON

Rejected because it mixes locale-neutral policy with translated copy, defeats exact
locale CAS/progress, and increases secret/localization leakage risk.

### Represent optional or contextual capability as a hard dependency

Rejected when the consumer has a valid provider-free mode or only some domain objects
require the provider. Hard dependencies remain appropriate only when no valid core
consumer mode exists without the provider.

## Verification

Required evidence includes:

- manifest rejection of every unknown schema/presentation/localization keyword;
- database constraints and transaction rollback for value, digest, revision, receipt,
  and outbox atomicity;
- disable/edit/enable/re-enable tests proving retained settings and active-schema
  validation before hooks;
- N/N+1 compatible, one-sided, disjoint, rollback, process-loss, and concurrent-write
  tests using two real schemas;
- dependency/capability matrices for Blog with/without Comments, Blog/Forum Reactions,
  and digital-only, physical-only, and mixed Commerce/Fulfillment flows;
- manifest/runtime dependency synchronization after semantic classification, with the
  validator discovering the canonical `src/module.rs` implementation layout;
- direct-SQL boundary guards preventing consumers from reading another owner's
  settings rows;
- UI Fluent coverage plus exact-locale localized-value CAS/fallback/sensitivity tests;
- secret-handle tests proving no secret bytes reach DB JSON, responses, events, logs,
  receipts, or translation storage;
- activation-mode tests distinguishing saved, active, restart-required, and
  reconciliation-required states;
- repository-wide search proving removed keys and fallback paths are gone.

## Consequences

- Settings become safer but package updates must carry explicit compatibility evidence.
- Disabled modules keep recoverable configuration without remaining active.
- Owners may use different physical schemas, but every surface follows the same
  lifecycle, revision, localization, secret, and verification contract.
- The current generic platform settings and direct tenant-module readers are migration
  work, not reference implementations.
- Admin can explain desired, validated, effective, and active state separately instead
  of presenting one misleading boolean or JSON blob.

## Relation to Comments-to-Blog projection decisions

This decision does not supersede
[Comments-to-Blog Reply Count Projection](./2026-07-16-comments-blog-event-projection.md)
or
[Comments-to-Blog Public Snapshot Invalidation](./2026-09-22-comments-blog-public-snapshot-invalidation.md).
Those decisions continue to govern Comments-owned lifecycle events and Blog-owned
derived state whenever the Comments capability is active or retained events are being
reconciled. They do not require Comments to be a Blog lifecycle dependency and do not
make Blog publication serving contingent on provider availability.
