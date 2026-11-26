# Executable native module scaffolding

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform tooling and native module owners
- Extends: [Canonical native module source layout](./2026-09-18-canonical-native-module-source-layout.md), [Native module reference contract](./2026-09-18-canonical-native-module-reference-contract.md)
- Supersedes: None
- Superseded by: None

## Context

The identity scaffold registers a crate, documentation and optional adapter
packages. Developers still have to reconstruct platform integration from Blog and
other owners. Stable working examples should carry that integration into generated
code and prove its behavior in a disposable workspace.

## Decision

Extend `cargo xtask scaffold` with executable owner capabilities selected by
explicit input. Keep the same source layout, workspace dependency catalog,
preview and recoverable registration pipeline. A persisted capability names
an owner entity and declares its storage contract. Its generator emits a real
operation, views of owner state, migration and tests. The first storage
contract is a tenant-scoped record with required localized titles.

The complete localized publication example includes owner writes, selected
read-side projections, and selectable API, UI and CLI adapters. Attach a
capability only when its implementation, wiring and behavior tests exist.

## Sources of truth and ownership

- `xtask/src/scaffold` owns the renderer and its build-time inputs.
- The rendered module owns its schema, records, locales, permissions and lifecycle.
- Existing foundation and capability crates retain their platform semantics.
- Blog provides reference operations, and Tax provides the compact stateless example.

## Invariants

- Each package uses the canonical file placement contract and declares its actual dependencies.
- Data is tenant-scoped and localized copy is separate from identity.
- Writes take an explicit normalized locale. Reads report requested, effective and available locales.
- Edit commands require a predecessor revision. An unknown tenant or record never returns another tenant's data.
- The owner declares authorization and runtime bindings before publishing a transport.
- The embedded localized owner requires a typed permission resource at composition and checks tenant-wide authority for privileged commands and reads.
- Test evidence identifies each generated and exercised capability.

## Non-goals

The native scaffold does not infer a database schema or permission grants from
a module slug. Independent FBA services remain a selectable deployment step.

## Data, transaction, and concurrency boundary

Generate a module-owned migration and private mappings for each declared
storage contract. An owner mutation commits its base row, locale rows and
any declared durable event together. External work runs after durable delivery.

## Context dimensions

Embedded owner calls consume tenant identity and an explicit locale. Published
adapters derive tenant, channel, principal and locale from the platform context
and apply the owner permission contract. Read fallback has no write authority.

## Events and projections

When selected, Search, Index and cache adapters use the existing capability
owners. The owner retains visibility policy. Integration tests demonstrate
publication, refresh, and removal or invalidation from the public read model.

The localized publication reference appends `ReindexRequested` with the owner
target `source:<module-slug>` on publish, published title edits and withdrawal.
The owner registers its bounded locale projection in the neutral Search source
registry. Search replaces the tenant/entity documents transactionally and
serializes that replacement with rebuilds in the same tenant/source scope.
The renderer registers the owner as gated through the host-owned [declared
audience contract](./2026-10-25-search-audience-declared-corpus.md). The same
tenant settings policy controls direct public reads, cached reads and current
Search candidate admission. Search also checks current publication in one
owner lookup for each candidate batch.

The embedded public reader can use its host's typed Cache service. It checks
current publication and revision against owner storage before cache lookup.
Tenant, record, owner revision, requested locale and fallback locale identify a
cached response. A mutation makes the previous revision unreachable. A failed
cache read falls back to the authoritative owner query.

For the relational Index, the localized record owner exposes tenant-scoped
keyset replay and exact-locale point loads. Distribution adapts the declared
schema, preserving source revision and locale identity and converting unpublished
rows to revisioned tombstones. This contract supplies internal replay independently
of Search and does not establish public Index serving or incremental delivery.

## Failure semantics

Unsupported or incomplete capability requests fail before apply. Generated
writes preserve the accepted revision, transaction and typed error contracts.
Select a fail-closed behavior for unavailable mandatory provider bindings.

## Migration and cutover

Keep existing identity-only operation and CLI attachment. Add implemented
capabilities through the same source transaction. Generated schema changes
follow native migration identity and deployment verification rules.

## Alternatives considered

- Empty files for every capability require callers to rebuild the integrations.
- Copying Blog directly transfers Blog-specific permissions, routes and schemas.
- A separate generator would duplicate registration and recovery semantics.

## Verification

Generate the module in a disposable workspace, apply the actual migrations, and
run its owner tests on supported databases. Exercise selected public entrypoints
and eventual projection changes before claiming readiness for those bindings.

## Consequences

New modules can reuse executable platform wiring and its regression cases.
Product-specific fields, invariants and permissions remain explicit owner input.
