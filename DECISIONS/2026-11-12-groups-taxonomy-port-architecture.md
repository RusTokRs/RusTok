# Groups and Taxonomy Storage Sovereignty and Group Taxonomy Port

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-groups, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md), [2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md), [2026-11-12-forum-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-12-forum-taxonomy-sovereignty-and-port-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Social groups in `rustok-groups` organize communities by interests, topics, and categories.
The database schema (`m20260721_000001_create_groups.rs`) already reserves `category_id: Option<Uuid>` on the `groups` table, and `CreateGroupInput` accepts `category_id`.
However:
1. `category_id` was unvalidated: passing a non-existent or foreign category ID did not return a validation error.
2. `GroupSummary` and `GroupSummaryGql` did not project `category_id` or resolved category metadata (name, slug, icon, color).
3. `ListGroupsRequest` did not allow filtering groups by `category_id`.
4. Integrating taxonomy directly with foreign table queries or concrete database entities would violate Fluid Backend Architecture (FBA) and module storage sovereignty (`AGENTS.md` §5).

## Decision

Apply the Ports & Adapters (Hexagonal / FBA) architecture pattern to integrate `rustok-groups` with taxonomy:

### 1. Consumer-Owned Port (`GroupTaxonomyPort`)
- Define `GroupTaxonomyPort` in `crates/modules/rustok-groups/src/ports/taxonomy.rs`.
- The port specifies all taxonomy operations required during the group lifecycle:
  - `validate_category_exists`: Verifies category existence within the tenant and module scope ("groups") within an active transaction or connection.
  - `load_categories`: Loads localized category snapshots (`GroupTaxonomyCategorySnapshot`) by ID batch for group summaries, listings, and details.
  - `resolve_term_names`: Resolves tag term names for group tags.
  - `ensure_group_tags_in_tx`: Contract and provider operation for future group-tag writes; current `rustok-groups/src` has no call site for this method.
- Define `GroupTaxonomyConnection<'a>` supporting `&DatabaseConnection`, `&DatabaseTransaction`, or `None`.

### 2. Standalone Mode & Graceful Degradation (`NoopGroupTaxonomyPort`)
- Provide `NoopGroupTaxonomyPort` in `rustok-groups::ports::taxonomy`:
  - `validate_category_exists` returns `Ok(true)` in standalone mode.
  - `load_categories` returns synthesized snapshots based on UUIDs.
  - `resolve_term_names` maps `term_id -> term_id.to_string()`.
  - `ensure_group_tags_in_tx` generates deterministic UUIDv5 identifiers.
- `GroupsService::new(db)` defaults to `NoopGroupTaxonomyPort`, allowing `rustok-groups` to run in isolated microservice environments and unit tests without taxonomy tables.
- `GroupsService::with_taxonomy_port` allows host/runtime injection of concrete taxonomy adapters.

### 3. Concrete Provider Adapter in `rustok-taxonomy`
- Implement `TaxonomyGroupTaxonomyPortAdapter` in `crates/modules/rustok-taxonomy/src/groups_taxonomy_port.rs`.
- `rustok-taxonomy` depends on `rustok-groups` (strictly acyclic: `taxonomy -> groups`).
- `rustok-groups` does NOT depend on `rustok-taxonomy` in production dependencies.

### 4. DTO & Query Capabilities
- Add `category_id: Option<Uuid>` to `GroupSummary` and `GroupSummaryGql`.
- Add `category: Option<GroupCategorySummary>` to `GroupSummary` when category details are resolved.
- Add `category_id: Option<Uuid>` to `ListGroupsRequest` and GraphQL query `groups(categoryId: ...)`.

## Sources of truth and ownership

- `rustok-groups` owns group records, group translations, memberships, and the stored `category_id` reference on a group.
- `rustok-taxonomy` owns category terms, localized category metadata, hierarchy, tag terms, and term-name resolution.
- `rustok-groups` owns `GroupTaxonomyPort` and its DTO snapshots; `rustok-taxonomy` owns `TaxonomyGroupTaxonomyPortAdapter`.
- The host/composition root chooses whether to inject the provider. The adapter exists in `rustok-taxonomy`, but no `GroupTaxonomyPort` wiring was found in `apps/server` at this recheck.

## Invariants

- `rustok-groups` contains zero direct SQL queries against `taxonomy_*` tables and zero foreign keys to taxonomy tables.
- `rustok-groups/Cargo.toml` contains no production dependency on `rustok-taxonomy`.
- The production Cargo dependency graph is strictly acyclic: `taxonomy -> groups`.
- Group creation validates a non-nil category through the port in the group transaction; taxonomy tag creation also receives that transaction.
- A missing provider must remain an explicit standalone profile, not be mistaken for a successful taxonomy-backed category or tag write.

## Non-goals

- Moving group-owned data or group membership policy into `rustok-taxonomy`.
- Making `rustok-groups` depend on taxonomy entities or tables.
- Changing group visibility, access, or membership authorization through this port.

## Data, transaction, and concurrency boundary

- `GroupTaxonomyConnection` distinguishes a borrowed connection, a borrowed transaction, and `None`; reads that are part of a group write can reuse its transaction.
- `validate_category_exists` is called with the active group-creation transaction; its provider error aborts the enclosing operation rather than committing a partial group write.
- `ensure_group_tags_in_tx` receives a transaction by contract, but no current Groups code calls it; tag writes are not part of the verified Groups lifecycle yet.
- Category and term-name reads are batch-shaped in the port. `load_categories` is used for projections; `resolve_term_names` currently has no call site in `rustok-groups/src`.
- This interface defines no hierarchy mutation or locking operation; category placement concurrency remains owned by Taxonomy.

## Context dimensions

- Tenant: every taxonomy port method receives `tenant_id`; the provider resolves categories and tags within the `groups` module scope.
- Locale: localized category reads and term-name resolution receive the requested locale and optional fallback. The category snapshot reports requested and effective locale.
- Actor and authorization: group command authorization remains in Groups and the typed `PortContext`; the taxonomy adapter does not grant group permissions.
- Channel: no channel dimension is carried by this port.

## Events and projections

- This decision adds no event contract. Group lifecycle events remain owned by Groups; taxonomy term events remain owned by Taxonomy.
- `GroupSummary` and `GroupSummaryGql` project `category_id` plus optional resolved category metadata; `ListGroupsRequest` and GraphQL `groups(categoryId: ...)` filter by the stored reference.
- Category snapshots are batch-loaded for summaries. If the no-op port is active, it synthesizes display metadata rather than reading Taxonomy; this is degraded standalone output, not proof that a canonical category exists.

## Failure semantics

- With the Taxonomy adapter attached, category validation errors propagate through `GroupsError`; a failure there aborts group creation before it commits. Category metadata loads used only for projections are best-effort at current call sites and may be omitted when the provider fails.
- `NoopGroupTaxonomyPort` accepts every non-nil category id and synthesizes category snapshots. Its tag methods return UUIDs/names without persisting or resolving taxonomy terms, but those methods currently have no Groups call sites. These fallbacks preserve an isolated Groups schema but do not establish taxonomy existence or persistence.
- The no-op default is therefore not equivalent to a wired monolith. Runtime composition and fail-closed behavior for taxonomy-dependent writes remain open; the in-progress status must not be upgraded until the host boundary is wired and tested.

## Migration and cutover

- The group schema already had `category_id`; the decision adds the read/write DTO and query support without adding a foreign key to Taxonomy.
- Group code depends only on its consumer-owned port. The provider adapter resides under `rustok-taxonomy` and depends on the Groups contract, preserving the acyclic dependency direction.
- Remaining cutover work is host composition: inject the adapter for deployments where Taxonomy is enabled and test the enabled and standalone profiles.

## Alternatives considered

- Direct queries from Groups to `taxonomy_*` tables were rejected because they break storage ownership and isolated deployment.
- A production dependency from Groups to `rustok-taxonomy` was rejected because the consumer must own the required contract and the dependency direction would be reversed.
- Duplicating category labels and tag dictionaries inside Groups was rejected because Taxonomy owns those canonical terms and translations.

## Verification

- Verify the Groups production manifest has no `rustok-taxonomy` dependency and the provider dependency graph remains acyclic.
- Exercise category validation, category snapshot projection, category filtering, transaction rollback on provider errors, and tag creation through the adapter.
- Exercise the no-op profile separately and assert it is reported as degraded rather than taxonomy-backed.

## Consequences

- Groups can compile against a consumer-owned taxonomy boundary and can be tested without taxonomy tables.
- The provider can batch localized category snapshots and term names without leaking Taxonomy entities into the Groups API.
- Until the host injects `TaxonomyGroupTaxonomyPortAdapter`, the default no-op behavior accepts unverified category ids and does not persist group tags in Taxonomy; the architecture is implemented in part, not complete.
