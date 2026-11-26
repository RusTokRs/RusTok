# Forum and Taxonomy storage sovereignty and Forum port decoupling

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md), [2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Following the decoupling of `rustok-product` via `ProductTaxonomyPort` and `rustok-blog` via `BlogTaxonomyPort`, an architectural review of `rustok-forum` revealed that Forum remains tightly coupled to `rustok-taxonomy`:

1. **Direct Service, Reader & Transaction Coupling**:
   - `CategoryService`, `CategoryTaxonomyReadService`, `CategoryTaxonomyTreeReadService`, `CategoryCommandService`, `CategoryLifecycleService`, `CategoryAudienceService`, `CategoryVisibilityService`, and `CategorySearchScopeService` directly invoke `TaxonomyOwnerCategoryReader`, `TaxonomyService`, `sync_module_category_with_owned_aliases_in_tx`, `lock_category_hierarchy_writer_in_tx`, `move_module_category_in_tx`, `reorder_module_category_siblings_in_tx`, and `shift_module_category_siblings_for_insert_in_tx`.
   - `ForumCategoryRouteService` invokes `TaxonomyService::resolve_term_route_for_module` and `TaxonomyOwnerCategoryReader::load_scoped_categories` directly to resolve category routes and canonical slugs.
   - `TopicService` calls `TaxonomyService::resolve_term_names` and `TaxonomyService::ensure_terms_for_module_in_tx` directly for topic tags.
   - `ForumSearchProjectionService` and `ForumReadModelOwnerService` call `TaxonomyOwnerCategoryReader` directly to assemble category read models and search documents.
   - `ForumError` contains `impl From<rustok_taxonomy::TaxonomyError> for ForumError`.
   - `crates/modules/rustok-forum/Cargo.toml` declares a mandatory dependency on `rustok-taxonomy`.

2. **Migration Coupling**:
   - Migrations `m20260823_000030_backfill_forum_categories_to_taxonomy.rs` and `m20260824_000031_retire_forum_category_legacy_storage.rs` import living `rustok_taxonomy` entities and normalization helpers directly.

3. **Impact on Fluid Backend Architecture (FBA) & Microservice Sovereignty**:
   - A standalone forum service or isolated test harness cannot start or execute read/write operations against an isolated database schema containing only forum tables (`forum_topics`, `forum_replies`, `forum_categories`, `forum_topic_tags`, etc.).
   - Any query or mutation triggers missing-table or missing-service errors when `taxonomy_terms` is absent.
   - This violates `AGENTS.md` §5 (module sovereignty, zero cross-module private table reads, no shadow/coupled dependencies).

## Decision

Apply the Ports & Adapters (Hexagonal / FBA) architecture pattern to decouple `rustok-forum` from `rustok-taxonomy`:

### 1. Consumer-Owned Port (`ForumTaxonomyPort`)
- Define `ForumTaxonomyPort` in `rustok-forum::ports::taxonomy`.
- The port specifies all taxonomy operations required during the forum category and topic tag lifecycle:
  - **Category Hierarchy & Locks**:
    - `lock_category_hierarchy_in_tx`: lock category hierarchy writer inside transaction.
    - `load_category_parents`: load parent mapping for categories.
    - `load_category_siblings_in_tx`: load sibling category IDs under a parent in transaction.
    - `shift_category_siblings_for_insert_in_tx`: shift siblings for insert in transaction.
    - `move_category_in_tx`: reposition category in hierarchy.
    - `reorder_category_siblings_in_tx`: reorder sibling category placements.
  - **Category Localized Copy & Projections**:
    - `load_scoped_categories`: load localized category metadata snapshots for category lists, trees, and search projections.
    - `load_category_locale_copy_in_tx`: load localized category name/slug/description for existing category in transaction.
    - `sync_category_copy_in_tx`: synchronize created/updated category translations and hierarchy into taxonomy in transaction.
  - **Route Resolution**:
    - `resolve_category_route`: resolve category term route and alias by slug and locale.
  - **Tag Dictionary & Attachment**:
    - `resolve_term_names`: resolve term names for topic tags by tenant and locale.
    - `ensure_tags_in_tx`: ensure topic tag terms exist in taxonomy in transaction.
- Introduce `ForumTaxonomyConnection<'a>` supporting both `&DatabaseConnection` and `&DatabaseTransaction`.
- `CategoryService`, `ForumCategoryRouteService`, and `TopicService` accept an optional `Arc<dyn ForumTaxonomyPort>`.

### 2. Standalone Mode & Graceful Degradation
- Provide `NoopForumTaxonomyPort` for standalone microservice execution and unit/integration testing:
  - Category operations operate purely on local `forum_categories` rows.
  - Tag queries return fallback values without failing SQL queries.
  - No foreign table errors when `taxonomy_terms` is absent.

### 3. Acyclic Architecture & Host Wiring
- `ForumTaxonomyPort` and `NoopForumTaxonomyPort` are defined within `rustok-forum::ports::taxonomy`.
- Because `rustok-profiles` depends on `rustok-taxonomy`, and `rustok-forum` depends on `rustok-profiles`, `rustok-taxonomy` MUST NOT declare a compile-time Cargo dependency on `rustok-forum` to avoid cyclic path dependencies (`forum -> profiles -> taxonomy -> forum`).
- The host runtime context (`ForumHttpRuntime` and `ForumGraphqlRuntimeData`) dynamically injects `Arc<dyn ForumTaxonomyPort>` via host capabilities, defaulting gracefully to `NoopForumTaxonomyPort` when absent.
- Remove `rustok-taxonomy` from `crates/modules/rustok-forum/Cargo.toml` `[dependencies]`.

### 4. Migration Independence
- Inline migration-specific entities and local helper functions in `m20260823_...` and `m20260824_...` so migration files do not import living `rustok_taxonomy` entities.

## Sources of truth and ownership

- `rustok-taxonomy` owns taxonomy terms, localized term copies, route aliases and canonical category hierarchy.
- `rustok-forum` owns topic/reply data, forum-specific category policy, audience rules and the consumer-owned `ForumTaxonomyPort` contract.
- The port is defined by Forum; a provider adapter belongs to the composition/provider boundary and must not create a `forum -> taxonomy -> forum` compile-time cycle.
- The current Forum manifest keeps `rustok-taxonomy` as a dev-dependency for integration fixtures, not a production dependency. The server now registers an in-process `ForumTaxonomyPort` adapter only when both modules are selected in its distribution registry; behavioral verification is pending.

## Invariants

- Forum production code does not query Taxonomy tables directly; Taxonomy access goes through `ForumTaxonomyPort`.
- `rustok-forum/Cargo.toml` has no production dependency on `rustok-taxonomy`; a dev-dependency may remain for test fixtures.
- The production Cargo dependency graph remains acyclic.
- Every provider-backed mutation that updates both Forum and Taxonomy uses the caller's database transaction.
- A no-op port is a degraded standalone behavior, not evidence that taxonomy-backed routes, labels or hierarchy are available.

## Non-goals

- Merging taxonomy storage or translations into forum entities.
- Removing taxonomy features or capabilities from the platform monolith.
- Treating a no-op provider as a replacement for a Taxonomy adapter in deployments that enable both modules.

## Data, transaction, and concurrency boundary

- `ForumTaxonomyConnection` represents a borrowed connection, a borrowed transaction or no supplied connection. The provider may use its owned database connection only for the explicit `None` case.
- All hierarchy mutations, category-copy synchronization and tag creation are expressed as `*_in_tx` operations and receive the Forum write transaction; provider failures propagate so the operation can roll back atomically.
- Category hierarchy writes use the port's writer lock before reading or changing placements. Provider implementations must serialize those operations consistently with Taxonomy's hierarchy invariants.
- Category and tag reads are batch-capable; the port carries tenant scope and does not expose Taxonomy entities or ORM models.

## Context dimensions

- Tenant: every port operation receives `tenant_id`; provider reads and writes must restrict to that tenant and the Forum module's Taxonomy scope.
- Locale: category snapshots, route resolution and tag-name reads carry requested locale and optional fallback locale. The provider returns matched/effective locale explicitly where the contract needs it.
- Actor and authorization: Forum enforces its security context, RBAC and category policy before invoking the provider; the taxonomy port itself does not grant Forum permissions.
- Channel: channel visibility and publication decisions remain Forum-owned and are not delegated to the taxonomy port.

## Events and projections

- This decision adds no event schema. Forum topic/category events remain Forum-owned; taxonomy term events remain Taxonomy-owned.
- Forum category summaries/tree/search projections and canonical category-route resolution obtain taxonomy metadata through the port.
- Topic tag names are resolved through the port; the Forum search projection must not query `taxonomy_terms` directly.
- The no-op implementation returns synthetic category snapshots, no route match, and UUID text for unresolved tag names. Those values are not canonical taxonomy projections.

## Failure semantics

- A configured provider's error is mapped to `ForumError` and fails the owning command/read rather than being silently converted into a successful taxonomy mutation.
- The no-op provider deliberately accepts hierarchy operations, returns synthetic category metadata, returns no canonical route, and returns deterministic IDs/names for tags without persisting them in Taxonomy.
- The joint Forum+Taxonomy server distribution installs a concrete owner-API adapter; a Forum-only registry retains the standalone fallback even when both Cargo features are compiled. Taxonomy-backed production behavior remains unverified; keep implementation status `In progress` until compilation and observable integration evidence exist.

## Migration and cutover

- The legacy forum-category-to-taxonomy migrations now carry their own migration-specific schema/helpers rather than importing live Taxonomy entities; migrations remain historical, executable transition code.
- The production dependency on Taxonomy was removed from Forum's manifest; the test-only dependency remains for fixtures that install Taxonomy alongside Forum.
- The domain source paths named in the decision were converted to the Forum-owned port. The host now supplies the real in-process adapter for joint distributions. Remaining cutover evidence is compilation and execution of both provider-enabled and standalone profiles.

## Alternatives considered

- Retaining direct production calls into `TaxonomyService` was rejected because it couples Forum behavior to a foreign service and bypasses the owner contract.
- Adding a compile-time adapter dependency from Taxonomy to Forum was rejected where it would form a cycle through Profiles; provider selection belongs to host composition or another acyclic seam.
- Duplicating canonical taxonomy terms and routes in Forum was rejected because it creates competing sources of truth.
- Treating the no-op provider as a faithful monolith implementation was rejected; its synthetic fallback semantics are only suitable for explicitly standalone operation.

## Consumer capability propagation follow-up (2026-11-26)

A configured `Arc<dyn ForumTaxonomyPort>` must reach both route resolution and
category audience metadata/hierarchy reads. GraphQL, REST, native storefront,
public discovery and Search must forward the same provider; no adapter may
replace a configured provider or its failure with the no-op fallback. The category
visibility floor and richer inherited audience layers must consult that provider's
hierarchy, not reinterpret every category as a root. Whole-tenant hierarchy
projections must match Forum's tenant-local membership exactly: no missing/extra
keys, foreign parent references, cycles or excessive depth. Validate the entire
graph before policy overrides or filtering can short-circuit. Category-tree
metadata must also reject duplicate/foreign identities instead of trusting row
count alone. Invalid provider projections fail the read; they are not route absence.

Storefront selected-category reads retain their existing `forum_categories:list`
admission through the metadata owner; ordinary owner-selected reads retain
`forum_categories:read`. Preserve this distinction with an internal typed access
mode, not by augmenting the caller's permission snapshot or weakening all reads.

SEO target providers need request-time access to host capabilities. Carry a cloned
`ModuleRuntimeExtensions` snapshot in `SeoTargetRuntimeContext`, populated by the
SEO service's runtime assembly. This is a generic provider-composition seam, not
a dependency from SEO onto Forum or Taxonomy. Direct standalone contexts use an
explicit empty snapshot. No new global registry or second taxonomy owner is added.

The consumer propagation follow-up alone did not prove taxonomy-backed routing.
The subsequent host implementation is `apps/server/src/services/forum_taxonomy.rs`,
registered before Search/SEO/GraphQL/native extension snapshots only when the
runtime distribution contains both Forum and Taxonomy. It depends on both modules
at the composition root and accesses only Taxonomy's public owner APIs, never its
entities. No dependency from Taxonomy to Forum is introduced.

Reads require the caller's connection or transaction. `None` fails with
`FORUM_TAXONOMY_CONNECTION_REQUIRED` rather than escaping to the adapter's pool.
Taxonomy now exposes transaction-aware route+alias resolution and a strict
module/tenant/kind membership parent reader, which rejects missing placements
instead of inventing roots. Sync uses Taxonomy's owned append-only alias history;
hierarchy and tag operations retain the same transaction. Provider failures remain
typed failures, not route absence. Module Category routes reject global fallback
matches that do not belong to the Forum binding.

The downstream continuation carries the selected provider through topic base/richer
reads, topic-policy management and dependent topic/reply, discovery, read-state,
engagement and report compositions, plus creation admission/policy management and
split/fork/reply-range comparisons, richer/moderation category management and
public moderation admission. Native creation and GraphQL/HTTP moderation factories
also forward the provider. Transaction-scoped policy reads retain the actual transaction; provider errors do not trigger standalone fallback. This is
source evidence only, with unexecuted fake-provider Rust regressions. Standalone
loaders remain in move/merge comparisons and route-tombstone snapshots. Completing and verifying
these owner/transaction paths is mandatory before production inheritance
acceptance, as tracked in Forum's implementation plan and README limitations.

Real-owner adapter tests are authored, not executed. No-op standalone semantics
and route absence remain unchanged when Taxonomy is not selected. A remote adapter
and distributed transaction semantics are not provided by this in-process adapter.

## Verification

- Verify the Forum production manifest has no `rustok-taxonomy` dependency and that production module dependencies remain acyclic.
- Exercise hierarchy reads/writes, localized category copy, route aliases, tag resolution, provider error rollback and isolated Forum-only execution.
- Exercise the no-op profile separately and ensure it is not represented as taxonomy-backed output.
- Rust compilation/tests for these boundaries remain required; they were not run in the current environment because the Rust toolchain is unavailable.

## Consequences

- Forum can own a stable taxonomy contract without importing Taxonomy's private persistence model in production code.
- Taxonomy can evolve behind that contract while host composition selects the provider.
- Standalone no-op behavior remains materially degraded. The joint server distribution now wires a real owner-API provider in source, but provider-backed compilation/runtime acceptance remains open, so this ADR stays `In progress`.
