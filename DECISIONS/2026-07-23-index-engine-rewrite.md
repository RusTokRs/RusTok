# ADR: Rewrite `rustok-index` as a generic cross-module Index Engine

- Date: 2026-07-23
- Status: Accepted
- Owners: RusToK platform / Index module
- Scope: `crates/rustok-index`, source-module index contracts, consumers, and runtime wiring

## Context

The current `rustok-index` implementation grew as a CQRS/read-model package with
source-specific Content, Product, and Flex indexers, direct reads of source
module tables, hard-coded projection models, incomplete query services, and a
minimal read/list port.

That shape does not satisfy the intended product requirement. The module must
solve the same class of problem as the Medusa Index Module: materialize data and
links from isolated modules into an optimized relational index so consumers can
filter, project, sort, count, and paginate across module boundaries without
runtime fan-out.

The project is still in early development. Preserving the rejected internal API
or migration history would add complexity without protecting production users.

## Decision

`rustok-index` will be rewritten as a generic, platform-owned relational Index
Engine.

The engine will own:

- schema and link registries;
- generic index records and mutations;
- incremental ingestion and deduplication;
- PostgreSQL index storage;
- link-aware query validation, planning, and SQL compilation;
- filtering, projection, sorting, count, and pagination;
- rebuild, checkpoints, reconciliation, and drift detection;
- distributed coordination and operator diagnostics.

Source modules will own schema declarations and adapters that convert their
domain state/events into generic Index records and mutations. Index will not
query source-module tables directly.

`rustok-search` remains a separate module responsible for relevance, ranking,
typo tolerance, autocomplete, synonyms, search UX, and external search-engine
connectors.

## Destructive rewrite policy

Backward compatibility with the current internal Index implementation is not a
constraint during this rewrite.

Existing code, migrations, tests, ports, adapters, fixtures, and documentation
may be deleted or replaced when they conflict with the target architecture.
Compatibility layers are prohibited unless they are required for a short,
documented consumer cutover and have an explicit removal milestone.

## Technical direction

- Use SeaORM and its SeaQuery integration for PostgreSQL access and dynamic SQL.
- Use a strongly typed query AST rather than unvalidated JSON filters.
- Use a graph library for schema/link planning rather than a custom graph.
- Use canonical locale types rather than free-form locale strings in keys.
- Use durable inbox/job/checkpoint state for event replay and rebuild recovery.
- Use bounded streaming pipelines; never collect all rebuild IDs first.
- Select the physical PostgreSQL layout through benchmark evidence.
- Keep search-engine libraries outside Index core.

## Status impact

The existing `index.read_model.v1` / `index.rebuild.v1` boundary is considered
legacy during the rewrite. FBA readiness is reset from `boundary_ready` to
`in_progress` until the new query/rebuild contracts have compiled and live
provider-consumer evidence.

## Consequences

Positive:

- the engine can scale across modules and tenants without source-query fan-out;
- new modules can participate through registration rather than Index-core edits;
- rebuild and recovery semantics become first-class;
- query behavior can be benchmarked and optimized independently of source
  schemas;
- the Index/Search boundary becomes explicit.

Costs:

- current Index consumers and contract evidence will require migration;
- legacy migrations and tests may be removed;
- the rewrite temporarily reduces boundary readiness;
- PostgreSQL schema and query planning require benchmark-driven design work.

## Implementation tracking

The authoritative task list and completion marks live in
`crates/rustok-index/docs/implementation-plan.md`. Every implementation PR must
update that plan and the progress log.
