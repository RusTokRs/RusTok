---
id: doc://docs/architecture/settings.md
doc_type: current_and_target_contract
status: current
owner: platform-architecture
canonical_for:
  - platform-settings-map
  - module-settings-lifecycle
  - settings-localization
language: markdown
---
# Settings and Configuration Architecture

This document is the canonical map for platform and module settings. It records
current executable truth separately from the accepted target in
[`2026-09-22-platform-module-settings-architecture.md`](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md).

The target is accepted and partially implemented. A row marked **gap** below must not
be described as already safe merely because a schema, UI form, or helper type exists.

## Why settings need their own architecture

"Settings" currently covers deployment input, tenant policy, module configuration,
domain configuration, secrets, translated copy, and optional integrations. Those have
different owners and lifecycle semantics. One generic JSON table cannot make them one
domain.

The unifying contract is not one table. It is one vocabulary and one set of rules for:

- semantic ownership and typed validation;
- scope and effective context;
- defaults, materialization, and precedence;
- revision, idempotency, and atomic events;
- disable, re-enable, update, rollback, reset, and purge;
- unconditional dependencies, tenant integrations, and contextual capability
  requirements;
- localization and secrets;
- runtime activation and operator-visible state.

## Canonical vocabulary

| Term | Use |
| --- | --- |
| Deployment configuration | Typed process/node input loaded by a host before or during composition |
| Durable setting | Operator/tenant intent persisted by its semantic owner |
| Normalized document | Complete value accepted by one exact schema digest |
| Desired state | Persisted operator intent |
| Effective state | Desired state after dependency, policy, channel, maintenance, and provider evaluation |
| Active state | State actually used by the current runtime generation |
| Integration policy | Consumer-owned intent to use an optional provider |
| Capability requirement | A typed fact that makes a provider necessary for one tenant feature, entity, or operation without making it necessary for the whole consumer module |
| Localized value | Exact-locale business copy stored in parallel owner rows |
| UI copy | Fluent label/help/error text; not a persisted setting value |
| Secret handle | Opaque reference; secret bytes never belong to settings JSON |

An Admin screen should show desired, effective, and active state separately whenever
they can differ. `saved = active` is not a safe default assumption.

## Current implementation inventory

Reviewed from `main@e1fae26ac8737cf4af4cfbdad2d396105e710df8`.

| Surface | Current source/persistence | Current owner boundary | Status |
| --- | --- | --- | --- |
| Server deployment configuration | `apps/server/src/common/settings.rs`, YAML/environment, `ServerRuntimeContext` | Host composition | Live; typed in many areas, but not a tenant settings source |
| Generic tenant platform settings | `platform_settings(tenant_id, category, settings, schema_version)` and `apps/server::SettingsService` | Server-owned generic service | **Gap:** mixed semantic owners, partial validation, `schema_version = 1`, non-atomic change event |
| Event delivery / Iggy | Dedicated settings services and tables | Dedicated platform control-plane services | Live owner-specific direction; activation can require restart |
| Search settings | `rustok-search::SearchSettingsService` and `search_settings` | Search owner | Live owner-specific reference direction |
| Static/native module settings | `[settings]` in `rustok-module.toml`; value in `tenant_modules.settings` | Manifest schema resolved by host, normalization/persistence through `rustok-modules` | Partially live |
| Static lifecycle concurrency | `module_static_tenant_lifecycle` revision/claim plus owner receipts | `rustok-modules` | Live for toggle/settings/recovery serialization |
| Static localized setting values | `module_static_localized_settings`, source-locale and change tables | `rustok-modules`, exposed to Translation by a neutral provider | Infrastructure live; no production module manifest currently opts in |
| Dynamic artifact settings | settings instance, stable data owner, admitted schema digest, recovery/purge contracts | `rustok-modules` artifact control plane | Stronger owner model; production recovery/fence evidence remains incomplete |
| SEO settings | Manifest schema plus typed `SeoModuleSettings` runtime normalization | SEO semantics, but reads `tenant_modules` directly | **Gap:** persistence boundary bypass |
| Forum engagement selection | `forum.use_reactions` in `tenant_modules.settings` | Forum semantics | Runtime consumer is live through the canonical tenant-module runtime API; Reactions availability remains a separate effective-capability decision |
| Blog manifest settings | `use_reactions` in `tenant_modules.settings` | Blog owns reaction-surface intent; `rustok-modules` owns static lifecycle/persistence | Live runtime consumer; setting is canonical tenant intent and does not imply Reactions availability |
| Blog reaction selection | `use_reactions` plus Blog reaction-subject provider | Blog owns subject/presentation intent; Reactions owns state | Runtime intent gate is live; provider/effective availability remains separate from stored intent |
| Email generic settings | `platform_settings.email` | Generic server path while Email runtime reads bootstrap config | **Gap:** saved value is not authoritative; historical secret material may exist |
| Nested manifest schema vocabulary | Owner validator supports `properties`/`items`; current SEO manifest uses `shape`/`additional_properties` | Host manifest adapter | **Gap:** unknown TOML schema keywords are ignored, so the generic editor/validator does not enforce the declared nested shape |
| Cross-module capability graph | `blog -> comments` removed; `commerce -> fulfillment` remains tracked separately | Module composition | Blog comment access is an optional capability and is no longer an unconditional module dependency |
| Digital/physical fulfillment requirement | Product accepts `product_type = "Digital"`; Product/Cart/Commerce shipping paths normalize a missing profile to `default` | Product owns product kind; Cart/Order own snapshots; Fulfillment owns shipping execution | **Gap:** no canonical typed fulfillment requirement, so digital lines can be forced through synthetic shipping identity/grouping |
| Blog dependency declarations | Root `modules.toml` and package/runtime metadata now agree on `content`, `taxonomy`, `outbox`, `channel`, and `profiles` | Composition manifest, package manifest, runtime metadata | Synchronized; Comments is an optional runtime capability rather than a static dependency |

### Current static settings data flow

| Phase | Current behavior | Consequence |
| --- | --- | --- |
| Schema discovery | Server parses `[settings]` and maps to `ModuleSettingSpec` | Schema is available to Admin and write validation |
| First toggle | `tenant_modules` can be inserted with `{}` | Defaults are not necessarily materialized on enable |
| Settings save | Active schema validates and fills defaults, then owner revision/CAS persists the full JSON | Saved documents are normalized at write time |
| Disable | Only `enabled` changes; settings JSON remains | Configuration survives disable |
| Edit while disabled | Generic owner writer rejects optional modules that are not effectively enabled; Admin disables the form | Invalid dormant settings cannot be repaired before enable |
| Re-enable | Dependency checks and hooks run with the retained JSON | **Gap:** retained JSON is not revalidated against the active schema before the pre-enable hook |
| Static rollout write | An observing checkpoint creates an in-memory guard | **Gap:** the active schema is passed as both N and N+1, so compatibility is not proven |
| Uninstall/purge | Tenant disable is not deletion; accepted release policy retains native/static data/settings | Safe retention, but operator state needs clearer presentation |

## Settings ownership matrix

Use this table before adding a key or table.

| Question | If yes | If no |
| --- | --- | --- |
| Does it change process topology, bind address, or node credentials? | Deployment configuration owned by the host/deployer | Continue |
| Does it participate in a domain invariant or need relational queries? | Owner domain aggregate/table | Continue |
| Is it tenant intent for a static/dynamic module package? | Module settings owner path | Continue |
| Is it a user-only preference? | Profile/preference owner | Continue |
| Is it secret material? | Store bytes in secret provider and persist only a handle | Continue |
| Is it only field/help/option text? | Fluent UI bundle | Continue |
| Is it translatable business copy? | Explicit localized field registration and parallel exact-locale rows | Continue |
| Is it locale-dependent behavior rather than copy? | Typed locale-scoped overlay | Do not add an unowned generic key |

## Target source-of-truth model

### Platform settings

`platform_settings` is not a semantic owner. Every surviving category must be moved
behind the component that consumes it:

| Current category | Target owner/action |
| --- | --- |
| `email` | `rustok-email`; separate non-secret tenant policy from deployment/provider credentials |
| `i18n` | `rustok-tenant` locale policy; do not create a second locale source |
| `oauth` | `rustok-auth` typed owner contract |
| `rate_limit` | Platform HTTP/runtime rate-limit owner with explicit live/restart semantics |
| `features` | Split by the owner of each feature; do not retain a global bag of booleans |
| `general` | Keep only keys with named consumers and a typed owner; delete decorative keys |

The table may remain only as an owner-neutral persistence adapter if every category is
registered by a real owner with typed schema, revision, runtime resolver, and migration.
The server GraphQL layer is an adapter, not the owner registry.

### Static/native module settings

The target aggregate uses the existing lifecycle owner and adds exact settings state:

| Fact | Authority |
| --- | --- |
| Module/package schema | Exact active package descriptor and canonical schema digest |
| Selected normalized value | Owner-controlled tenant settings record |
| Concurrency | `module_static_tenant_lifecycle.revision` and active claim |
| Idempotency | Shared owner-operation receipt |
| Enablement intent | Explicit tenant override/inherited policy in the modules owner |
| Effective availability | Modules policy decision, including dependencies/channel/maintenance |
| Localized values | Static localized settings owner rows keyed by stable field ID and exact locale |
| Runtime activation | Owner-declared activation mode and reconciliation status |

The persisted settings state is one of:

| State | Meaning | Can serve? |
| --- | --- | --- |
| `not_applicable` | Package has no settings schema | Yes, subject to module policy |
| `ready` | Full normalized document matches the stored active schema digest | Yes |
| `migration_required` | Retained value is not admitted by the active package schema | No |

Defaults are applied when a document is first materialized. Persisted settings are not
partial overrides over whatever defaults happen to ship later. A changed package
default therefore cannot silently change a configured tenant.

### Owner-specific domain settings

Use a domain table instead of module JSON when the setting:

- has child collections or ordering;
- needs tenant-composite foreign keys;
- participates in uniqueness or other database constraints;
- changes atomically with domain data;
- needs independent revisions or permissions;
- is queried on a hot path where JSON inspection would become the domain model.

Module settings may select a high-level policy, but they must not duplicate the
domain-owned configuration graph.

## Lifecycle state matrix

| Operation | Value retention | Validation | Runtime side effect | Destructive authority |
| --- | --- | --- | --- | --- |
| Save while enabled | Replace by revision CAS | Active schema and owner semantics | Per declared activation mode | None |
| Save while disabled | Replace dormant intent by revision CAS | Installed/admitted active schema | None until enable unless explicitly documented | None |
| Disable | Retain all settings/localizations | Current aggregate consistency | Stop serving/work through policy | None |
| Enable | Retain or atomically replace value | Active schema, dependency closure, required integrations, secrets, context | Hooks only after validation | None |
| Reset | Materialize current defaults through explicit command | Active schema | Per declared activation mode | Replaces selected setting values only |
| Package update | Retain exact value and digest | Real N/N+1 compatibility or maintenance migration | Guarded rollout/reconciliation | None |
| Rollback | Retain only if predecessor accepts it | Predecessor compatibility | Guarded recovery | None |
| Remove from build | Retain native/static owner data | N/A while code absent | No serving | None |
| Purge | Delete only exact authorized settings boundary | Recovery, holds, tombstone, owner continuity | No serving | Explicit privileged operation only |

### Required enable sequence

1. Load exact current lifecycle revision and tenant intent.
2. Resolve the active package and schema digest.
3. Normalize the retained or submitted value against that schema.
4. Resolve the minimal unconditional dependency closure.
5. Resolve tenant integration policies and contextual capability requirements without
   reading provider private tables.
6. Validate secret handles and other external prerequisites without exposing bytes.
7. Run the pre-enable hook with the validated normalized document.
8. Atomically commit value, schema digest, enablement, revision, receipt, policy
   transition, and outbox facts.
9. Run/reconcile post-enable effects under the lifecycle recovery contract.

If step 3 fails, store/report `migration_required` and keep the module non-serving.

## Package update compatibility

| Schema change | Automatic eligibility |
| --- | --- |
| Add optional key with deterministic default | Eligible if the materialized N document is accepted unchanged or through an explicitly compatible normalization |
| Widen numeric/string/array bound | Usually eligible when both schemas accept current/intermediate writes |
| Add enum option | Eligible for existing values; mixed writers still need the real intersection guard |
| Change default only | Does not mutate already materialized tenants automatically |
| Remove/rename key | Maintenance migration required |
| Change type/unit/meaning | Maintenance migration required |
| Narrow enum/range | Maintenance when any current value is outside N+1 |
| Add required key without admissible default | Maintenance migration required |
| Change localized/sensitive/secret classification | Maintenance and data-accounting review required |
| Disjoint schemas | Fenced maintenance transition and protected recovery point |

The compatibility guard must persist both schema digests and rollback identity. Passing
the same active schema twice is not compatibility evidence.

## Cross-module settings

### Dependency and capability classification

A static hard dependency is valid only when every valid deployment and every core
operating mode of the consumer requires the provider. A compile-time import, one UI
contribution, one optional feature, or one subset of domain data is not sufficient.

| Class | When it applies | Canonical representation |
| --- | --- | --- |
| Unconditional module dependency | Consumer cannot enable or serve any valid core contract without provider | Static module dependency graph; lifecycle enforces the complete closure |
| Tenant integration policy | Tenant chooses whether/how a feature uses a provider | Consumer-owned setting plus effective capability binding |
| Contextual domain requirement | Only a specific entity, line item, workflow, or command needs the provider | Typed domain fact/snapshot evaluated at that operation boundary |
| Optional presentation contribution | Provider adds UI but consumer behavior remains valid without it | Host capability/UI composition, not a lifecycle dependency |

The static graph is the smallest unconditional kernel. It must not become the union of
every provider that any feature or data subtype might use.

Every static edge needs an owner-reviewed proof, not just synchronized manifest text:

| Required proof | Evidence |
| --- | --- |
| No provider-free core mode exists | Consumer operating-mode matrix, including provider absent |
| The provider is needed for the whole module rather than one feature/data subtype | Entity/operation capability matrix |
| Disable semantics are safe | Required-binding, in-flight-operation, retained-data, and historical-read cases |
| All declarations express the same reviewed edge | `modules.toml`, package manifest, runtime metadata, server feature closure, and validator evidence |

This review proves that `blog -> comments` and `commerce -> fulfillment` are
over-constrained. It does not silently certify every other current static edge as
ideal; unreviewed edges remain executable truth pending the same evidence.

### Tenant integrations and contextual capabilities

Use a consumer-owned integration policy when a tenant selects an optional provider
feature. Use a typed domain fact, not a generic setting, when the requirement varies by
entity or operation.

```text
consumer intent
    + consumer/provider module policy
    + provider registration/readiness
    + channel/locale/auth/maintenance context
    = effective integration decision
```

The stored policy or domain fact is intent only. The decision is a projection returned
through an owner port. Direct reads of another module's `tenant_modules` row are
forbidden.

Provider absence does not disable the consumer. Only the feature or operation that
requires the capability fails closed or follows its explicitly documented degraded
mode. Provider disable is rejected only while an active `required` binding, in-flight
operation, or retained owner invariant genuinely needs it; an enabled consumer module
alone is not evidence of such a requirement.

| Example | Correct ownership |
| --- | --- |
| Blog posts/categories without comments | Valid Blog core behavior; no Comments requirement |
| Blog comments visible/closed/hidden | Blog owns surface policy; Comments owns threads/comments/moderation; provider availability is resolved through the optional capability boundary |
| Blog reactions | Blog owns whether its post surface requests Reactions; Reactions owns catalogs/actor state/counts |
| Forum engagement provider | Forum owns the selection and transition/reconciliation; Reactions owns reaction state |
| Reactions enabled for unrelated consumers | Does not force Blog or Forum to use Reactions |
| Digital product/order line | Product/Order owns the non-shipping domain fact; no Fulfillment requirement |
| Physical product/order line | Product/Cart/Order carries a typed fulfillment requirement; checkout requires Fulfillment only for the affected delivery group |
| Mixed cart | Digital lines bypass delivery; physical lines form fulfillment groups without forcing a shipping identity onto digital lines |

Blog must remain enableable and able to serve publications when Comments is absent.
Likewise, Commerce must support a digital-only flow without Fulfillment. The current
static edges are executable truth to remove, not examples to preserve.

#### Blog comment capability matrix

These are semantic policy states; the owning Blog contract chooses the final field
name and transport shape.

| Blog comment policy | Comments capability required? | Blog publication serving | Comment behavior | Existing comment data |
| --- | --- | --- | --- | --- |
| Disabled/hidden | No | Available | No comment read/write surface | Retained by Comments; no implicit purge |
| Read-only/closed | Yes for reads | Available | Existing comments may be read; new writes rejected | Retained |
| Enabled/open | Yes | Available | Read/write/moderation through Comments owner ports | Retained |
| Required capability unavailable | Unresolved | Available | Comment surface fails closed with actionable state | Retained; stored Blog intent unchanged |

Turning a Blog feature off never deletes Comments-owned data or changes the Comments
module for other consumers. Re-enabling the feature re-runs capability and settings
compatibility checks before exposing the surface.

#### Blog reaction capability matrix

| Blog reaction policy | Reactions capability required? | Blog publication serving | Reaction behavior | Existing reaction data |
| --- | --- | --- | --- | --- |
| Disabled/hidden | No | Available | No reaction read/write surface | Retained by Reactions; no implicit purge |
| Enabled with hide-on-unavailable policy | Only while serving the feature | Available | Use Reactions while available; otherwise expose an explicit unavailable/hidden state, never a fabricated zero count | Retained; stored Blog intent unchanged |
| Enabled and required | Yes for reaction operations | Available | Read/write fails closed with actionable state when unavailable | Retained; stored Blog intent unchanged |
| Reactions enabled for another consumer | No | Available | Does not opt Blog into reactions | Unchanged |

Blog never enables or disables the Reactions module. It owns only its post-surface
policy; Reactions owns reaction kinds, actor state, counts, and retention.

#### Commerce fulfillment capability matrix

| Order/cart state | Fulfillment capability required? | Required behavior |
| --- | --- | --- |
| Digital-only | No | No shipping selection, delivery group, shipping charge, or fulfillment row |
| Physical-only | Yes at shipping/checkout/dispatch boundaries | Every physical line has exact typed delivery-group and fulfillment coverage |
| Mixed | Yes only for physical subset | Digital lines remain outside delivery groups and fulfillment items |
| Existing in-flight shipment | Yes until owner-safe terminal/reconciliation state | Provider disable is rejected or explicitly reconciled |
| Historical completed shipment | Not for new execution | Records remain readable/retained under Fulfillment ownership |

An enum is preferred over a boolean when `disabled`, `optional/degraded`, and
`required/fail-closed` have different behavior.

## Localization model

### 1. Settings editor UI copy

Presentation metadata uses stable message IDs for:

- field label;
- description/help;
- group/section title;
- option label;
- validation guidance;
- restart/reconciliation notices.

The host resolves the effective locale once. Module-owned Fluent bundles provide the
copy. Humanizing a JSON key and displaying an English manifest `description` are
fallback diagnostics during migration, not the target UI contract.

### 2. Localized setting values

Only explicitly registered string leaves may be localized. The static owner already
provides stable field IDs, sensitivity fences, exact-locale rows, source-locale
provenance, owner/target CAS, and Translation provider integration.

No current production `rustok-module.toml` declares `settings_localization`, so this
infrastructure is not evidence that a real module's setting values are localized.

### 3. Locale-scoped behavior

A value such as a locale-specific date format, domain routing policy, or per-locale
feature rule is configuration, not translated copy. Model it as a typed overlay with:

- explicit locale key;
- tenant locale-policy validation;
- documented fallback;
- locale in cache/projection identity;
- owner revision and migration semantics.

Never localize schema keys, enum tokens, URLs, secret handles, or policy identifiers.

## Secrets

| Operation | Contract |
| --- | --- |
| Set | Write-only value enters secret provider; settings persist returned handle/revision |
| Read | Return `configured`, safe provider/reference metadata, and revision only |
| Rotate | New secret-provider revision plus owner receipt; dependent runtime reconciliation is explicit |
| Clear | Explicit command; empty string is not a magic preserve/delete instruction |
| Export/event/log | Never include bytes; redact sensitive identifiers where necessary |

Historical `platform_settings.email` secret material must be scrubbed and affected
credentials rotated during cutover.

## API and Admin contract

A settings read should expose typed facts rather than only a raw JSON string:

| Field | Purpose |
| --- | --- |
| owner/scope identity | Prevent ambiguous writes |
| desired document or typed fields | Operator intent |
| schema digest | Exact validator identity |
| revision | CAS precondition |
| validation state/diagnostics | `ready` versus `migration_required` |
| enabled/effective/active states | Separate lifecycle and runtime truth |
| activation mode | Immediate/reconciled/restart/next-enable semantics |
| integration decisions | Provider availability without exposing provider storage |
| localized coverage | Exact rows only; fallback shown separately |

Every mutation requires authenticated context, expected revision, and idempotency key.
Raw JSON may remain an expert/debug editor only when it uses the same owner mutation and
schema; it is never a bypass.

## Events, cache, workers, and observability

The owning transaction persists a content-free settings change fact when downstream
work is required. The fact includes safe owner/scope, schema digest, revision, and
stable field IDs, never raw values or secrets.

Owners must document:

- activation mode: `immediate`, `reconciled`, `restart_required`, `next_request`, or
  `next_enable`;
- cache key dimensions and invalidation generation;
- worker/schedule reconciliation and retry identity;
- fixed-cardinality success/failure/conflict/migration-required metrics;
- sanitized audit facts and correlation identifiers;
- how to rebuild or reconcile derived state.

## Known gaps and required cutover

| Priority | Gap | Required outcome |
| --- | --- | --- |
| P0 | Re-enable does not validate retained static settings before hooks | Active-schema preflight and `migration_required` state |
| P0 | Static N/N+1 guard validates the same schema twice | Persist/load real predecessor and candidate schema digests/documents |
| P0 | Generic email settings can diverge from runtime and historically hold secrets | Email-owned typed settings, secret handles, data scrub/rotation |
| P0 | Platform settings event is published after save on a best-effort path | Owner transaction plus transactional outbox |
| P0 | Unknown manifest schema keywords are ignored; live SEO metadata uses unsupported nested-shape names | One canonical schema vocabulary, `deny_unknown_fields`, manifest migration, and negative tests |
| P1 | `platform_settings.schema_version` is decorative | Replace with exact owner schema identity/revision or delete the field |
| P1 | SEO reads `tenant_modules` directly | Move SEO to the canonical owner settings read/effective-integration boundary |
| P1 | Static settings still lack complete owner-port/effective-integration coverage | Finish owner-specific read/effective-capability ports and activation evidence for remaining modules |
| P1 | Disabled module settings cannot be repaired in Admin | Dormant edit and/or atomic enable-with-settings |
| P1 | Static rows do not persist exact schema digest/state | Add digest and `not_applicable/ready/migration_required` semantics |
| P1 | Module keys include camelCase internal names | Zero-legacy `snake_case` cutover with data transformation |
| P1 | Blog/Comments and Commerce/Fulfillment use unconditional edges for feature/data-specific capabilities | Remove the hard edges; add typed tenant bindings and operation-scoped capability requirements |
| P1 | Blog dependency views diverge and the publish-readiness validator does not recognize its canonical source layout | Fix validator discovery, then atomically synchronize the minimal unconditional graph across all three declarations |
| P2 | Settings UI labels/options are derived from keys/raw English descriptions | Fluent presentation metadata and bundle verification |
| P2 | No real module opts into localized settings values | Select a justified pilot; prove exact locale, sensitivity, CAS, and Translation flow |
| P2 | Activation/restart state is inconsistent across settings surfaces | Standard desired/effective/active projection and activation modes |

## Implementation sequence

1. Inventory all stored keys, schemas, readers, writers, runtime consumers, secrets,
   events, caches, workers, UI surfaces, and dependency edges. Classify decorative
   settings and over-constrained dependencies for removal.
2. Freeze canonical schema digest and presentation metadata contracts in
   `rustok-module.toml`; reject unknown schema keywords, migrate current nested schema
   declarations, and add `snake_case` guardrails.
3. Extend the static aggregate with schema digest and settings state; backfill each
   current row by its actual owner schema.
4. Add dormant edit and atomic enable-with-settings. Validate before hooks and publish
   owner policy/outbox facts atomically.
5. Replace the placeholder static rollout guard with durable real N/N+1 schema
   compatibility and maintenance migration.
6. Replace over-constrained static edges with tenant capability bindings or typed
   operation-scoped requirements; keep provider-free consumer paths executable.
7. Move `platform_settings` categories and direct module readers to semantic owners.
8. Cut secrets to handles, scrub persisted bytes, and rotate affected credentials.
9. Add Fluent UI metadata and one justified localized-value pilot.
10. Add source/runtime/database/browser guardrails and delete raw/direct/fallback paths.

## Verification matrix

| Boundary | Minimum evidence |
| --- | --- |
| Schema | Canonical digest determinism, unknown schema-keyword and value-key rejection, limits, `snake_case`, sensitive/localized classification |
| Persistence | Tenant uniqueness/FKs, value+digest+revision+receipt+outbox rollback atomicity |
| Lifecycle | disable retains; dormant edit; invalid re-enable denied before hook; enable-with-settings replay/CAS |
| Update | N/N+1 intersection, concurrent writes, one-sided maintenance, rollback, process loss |
| Dependencies | minimal unconditional graph; Blog with/without Comments; optional Reactions; digital-only, physical-only, and mixed Commerce/Fulfillment flows |
| Localization | Fluent coverage, exact-locale CAS, source provenance, fallback-not-coverage, sensitive-path denial |
| Secrets | no bytes in DB JSON, response, event, log, receipt, translation row, or export |
| Runtime | saved versus active/restart/reconciliation status and cache/worker convergence |
| Transports | GraphQL/native parity, typed errors, auth/tenant/permission context |
| Cleanup | Removed keys/readers/fallbacks absent repository-wide |

## Definition of done for one setting

A setting is complete only when all applicable cells are answered:

| Area | Required answer |
| --- | --- |
| Owner | Which component owns meaning, validation, persistence, and runtime use? |
| Scope | Global, tenant, channel, locale, user, installation, or another typed identity? |
| Schema | What exact digest/version admits the value and what are its limits? |
| Default | When is it materialized and can it change existing behavior? |
| Lifecycle | What happens on disable, enable, update, rollback, reset, remove, and purge? |
| Concurrency | What revision and idempotency identity protect writes/retries? |
| Dependencies | Is this unconditional dependency, tenant integration intent, contextual capability requirement, or effective decision? |
| Localization | UI copy, localized value, locale overlay, or not localized? |
| Secrets | Does any value require a handle and sensitivity fence? |
| Activation | When does saved state become active and how is failure reconciled? |
| Derived state | Which caches, indexes, workers, schedules, or projections change? |
| Security | Which permission and tenant/context checks fail closed? |
| Verification | Which database, owner, transport, runtime, and UI evidence proves it? |

## Related documents

- [Platform and module settings ADR](../../DECISIONS/2026-09-22-platform-module-settings-architecture.md)
- [Module architecture](./modules.md)
- [Database architecture](./database.md)
- [i18n architecture](./i18n.md)
- [`rustok-module.toml` contract](../modules/manifest.md)
- [`rustok-modules` owner documentation](../../crates/modules/rustok-modules/docs/README.md)
- [Module release and rollback plan](../modules/module-release-rollback-plan.md)
