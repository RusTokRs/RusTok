# M4 query planner actualization

Date: 2026-07-30

This note actualizes the live `rustok-index` implementation plan after rechecking the
M3 storage boundary and the M4 query slices merged into `main`.

## Recheck result

M3 source implementation remains complete through retained bundle review, archive
manifest generation, saved-manifest verification, and recursive filesystem drift
detection. The repository owner still needs to execute and admit one fresh real
PostgreSQL partition packet. That owner gate blocks production partition lifecycle
design but does not block M4 query source work.

The previous server-composition blocker is removed at the capability level. Source
modules publish generic schema contracts into an Index-owned catalog, the selected
distribution materializes one non-empty immutable registry after all module registrations,
and the server binds that registry to its database through the Index-owned PostgreSQL
runtime materializer. Social Graph is the first source publication. Notification block/mute
policy is the first authorized consumer source, protected by a default-off activation gate
until retained projection readiness, lag, and result-parity evidence exists.

## Actualized status

- M3 real retained PostgreSQL packet execution: `open_owner_action`.
- M3 production partition lifecycle: `blocked_by_retained_packet`.
- M4 deterministic executable query planning: `source_complete_execution_pending`.
- M4 stable relation aliases and typed referenced fields: `source_complete_execution_pending`.
- M4 root/one-link query compilation and result decoding: `source_complete_execution_pending`.
- M4 query-scoped cursors and lookahead pagination: `source_complete_execution_pending`.
- M4 many-link `EXISTS` filtering: `source_complete_execution_pending`.
- M4 nested many-link projection aggregation: `source_complete_execution_pending`.
- M4 PostgreSQL query port and strict row adapter: `source_complete_execution_pending`.
- M4 retained plan/SQL snapshots: `source_complete_owner_execution_pending`.
- M4 PostgreSQL/reference fixture source: `source_complete_owner_execution_pending`.
- M4 retained PostgreSQL/reference capture source: `source_complete_owner_execution_pending`.
- M4 PostgreSQL/reference admission review source: `source_complete_owner_execution_pending`.
- M4 source-owned immutable schema registry: `source_complete_execution_pending`.
- M4 server-owned shared query runtime composition: `source_complete_execution_pending`.
- M4 first authorized consumer cutover source: `source_complete_execution_pending`.
- M4 first consumer activation: `open_owner_action`.
- M4 live PostgreSQL/reference execution evidence: `open_owner_action`.

## Executable plan v4

`SchemaRegistry::plan_query`:

1. validates the registry-backed query before planning;
2. collects and sorts every referenced link prefix;
3. assigns deterministic `t0`, `t1`, ... aliases;
4. resolves joins against registered schema contracts;
5. propagates `traverses_many` from the first many-cardinality link;
6. captures every referenced field with path, alias, type, cardinality, and nullability;
7. preserves public projection order;
8. groups projected many-traversing fields by terminal relation path into
   `PlannedManyProjection` contracts;
9. records every identity prefix required to reconstruct a complete nested relation
   chain;
10. retains filters, ordering, pagination, and exact-count intent.

The deterministic fingerprint domain is `rustok-index-query-plan-v4` because grouped
many-projection metadata is part of executable plan identity and compiler safety.

## Controlled PostgreSQL boundary

Root and explicit one-link projection, all validated filters, typed ordering, exact
count, keyset continuation, and bounded offset remain controlled SQL with ordered bind
DTOs. Many-link filters compile through independent correlated `EXISTS` chains.

Each `PlannedManyProjection` compiles as one correlated JSONB aggregate outside the
outer root rowset. Aggregate items preserve the complete linked entity identity chain
and aligned tagged field values. Stored link ordinal, target entity identity, and locale
produce deterministic item ordering. Missing reachable rows yield an empty array.

Because many projection does not enter the outer rowset, root pagination, one-row
lookahead, and exact count remain duplicate free.

## Result and execution handoff

`SchemaRegistry::compile_postgres_page_query` changes only the validated page-limit bind
from `N` to `N + 1`. `decode_postgres_query_page` re-plans and verifies:

- the v4 plan fingerprint;
- unique scalar and many-relation output aliases;
- exact scalar column and `CompiledManyRelationColumn` metadata;
- requested page size and maximum `N + 1` rows;
- tagged field type/cardinality/nullability;
- nested identity/value arity;
- non-nil and non-duplicate complete nested identity chains;
- optional exact count.

The lookahead row is removed. Cursor pages derive the next scoped cursor from the last
retained root/order tuple; offset pages report `has_more` without creating a cursor.

`PostgresIndexQueryPort` is the Index-owned execution boundary. It verifies exact active
persisted schema contracts for the query tenant, converts every `PostgresBindValue`
variant, executes page and optional count in one read-only repeatable-read PostgreSQL
transaction, maps only compiler-declared aliases, and delegates semantic validation and
cursor creation to the strict decoder.

## Retained snapshots

`query_snapshot_tests::retained_v4_plan_and_sql_snapshots_are_stable` compares a fixed
canonical query against three retained files:

- readable executable-plan metadata;
- the complete exact PostgreSQL SQL string;
- ordered bind, scalar-column, and many-relation metadata.

The fixture uses fixed identifiers and forbids automatic snapshot rewriting. SQL keeps
all contract values in `$N` binds.

## PostgreSQL/reference fixture

`postgres_query_port_matches_reference_fixture` creates one isolated PostgreSQL schema,
applies the canonical Index migrations, persists schemas and records through production
stores, executes through `PostgresIndexQueryPort`, and compares the complete
`IndexQueryPage` with an independent in-memory materialization from the same records.

The source scenarios cover scoped cursor continuation, exact count, bounded offset,
one-link filtering/projection, many-link `Gte`/`Contains`/`Ne`/`IsNull`, and nested
identity/value alignment. The fixture is env-gated and has not been run by the
implementation agent.

## Retained equivalence capture and admission

`index-query-equivalence-capture` requires explicit opt-in, an exact clean checkout
commit, stable run key, and PostgreSQL 16. It runs only the merged fixture, rejects
skipped-test success, rechecks source and database identity, and publishes a fresh
three-file descriptor-last bundle containing exact stdout/stderr plus hashes and
provenance. The PostgreSQL URL and credentials are not serialized.

`index-query-equivalence-admission` performs no Cargo or database execution. It reads the
immutable bundle, requires independent expected source identity, rejects unknown
descriptor fields, aliases, symlinks, extras, hash drift, command/scenario drift,
skipped output, and mid-review byte changes, then creates one no-clobber receipt outside
the bundle. The receipt records `production_lifecycle_authorized: false`.

Both tools are source complete but have not been run. A capture exit alone does not admit
the bundle; an admission receipt alone does not authorize deployment or partition work.

## Source-owned schema registry and runtime

`IndexModule` seeds `IndexSchemaSourceCatalog` in `ModuleRuntimeExtensions`. Source
modules publish exact generic contracts with `register_index_schema_source`; duplicate
ownership for one `SchemaRef` and owner drift across versions of one schema identity fail
closed.

After all modules and selected bridges register, `rustok-distribution` materializes the
complete catalog through one `SchemaRegistry::register_batch`. This permits cross-source
links, preserves deterministic `BTreeMap` order, and publishes one
`SharedIndexSchemaRegistry` wrapping an immutable `Arc<SchemaRegistry>`. Missing or empty
catalogs do not publish a false registry or query runtime.

With its `index` feature enabled, `SocialGraphModule` is the first source owner. It
publishes the existing relation schema under owner slug `social_graph`; distribution and
server code do not import the schema builder or Social Graph DTOs.

`SharedIndexQueryRuntime` is a neutral cloneable `IndexQueryPort` capability. The
Index-owned `materialize_postgres_index_query_runtime` is the only production constructor:
it combines the final shared registry with the host database, rejects duplicate runtime
publication, performs no SQL, and inserts the capability into module extensions. The
server facade invokes it after all existing host-provider composition, and those
extensions transfer unchanged into `HostRuntimeContext`.

Runtime presence does not establish persisted tenant schema readiness. Every execution
still performs the exact active fingerprint and semantic JSON preflight inside
`PostgresIndexQueryPort`.

## First authorized consumer source

`IndexSocialGraphPrivacyReadPort` is the first owner-defined adapter consuming
`SharedIndexQueryRuntime`. It preserves the existing `SocialGraphPrivacyReadPort`
authorization and request bounds while translating block, mute, and follow checks into
typed filters over the owner-published relation schema.

The final server facade uses the default-off activation gate
`RUSTOK_SOCIAL_GRAPH_INDEX_PRIVACY_READS_ENABLED`. While disabled, the authoritative owner
read path remains selected. When enabled, the facade requires `SharedIndexQueryRuntime` and
recomposes notification block/mute policy. Block remains symmetric, mute remains
recipient-to-actor directional, and custom notification relation providers retain
priority. Missing tenant schema or storage readiness is retryable fail-closed and cannot
become an implicit allow; no owner-table fallback exists after activation.

Revision-bearing follow reads, profile privacy, GraphQL, storefront, admin, and
presentation authorization remain outside this cutover. The activation flag must remain
off until the owner retains projection readiness, lag, and result-parity evidence.

## Remaining bounded M4 work

The canonical checklist remains open until the owner runs the fixture through capture,
admits the retained bundle, and preserves both bundle and receipt. Additional boundaries
remain:

- retain Social Graph projection readiness, lag, and result-parity evidence and decide
  whether to activate the first consumer;
- aggregate ordering semantics for paths traversing `many`;
- publish schemas from additional source owners as consumers are selected;
- additional server/storefront/admin/search authorization and consumer cutovers.

The real retained PostgreSQL partition packet remains an independent owner gate for
production partition lifecycle work.

## Owner validation

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-index planner_tests -- --nocapture
cargo test -p rustok-index source_schema_registry -- --nocapture
cargo test -p rustok-index query_runtime -- --nocapture
cargo test -p rustok-index postgres_compiler_tests -- --nocapture
cargo test -p rustok-index postgres_many_projection_tests -- --nocapture
cargo test -p rustok-index postgres_query_result_tests -- --nocapture
cargo test -p rustok-index query_snapshot_tests -- --nocapture
cargo test -p rustok-social-graph --features index index_privacy -- --nocapture
cargo test -p rustok-social-graph --features index module_publishes_its_index_schema_through_runtime_extensions -- --nocapture
cargo test -p rustok-distribution source_schema_catalog_materializes_after_all_modules_register -- --nocapture
cargo test -p rustok-server host_materializes_index_query_runtime_after_source_registry -- --nocapture
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-index postgres_query_port_matches_reference_fixture -- --nocapture
INDEX_QUERY_EQUIVALENCE_ALLOW_CAPTURE=1 \
RUSTOK_INDEX_TEST_DATABASE_URL=postgres://... \
INDEX_QUERY_EQUIVALENCE_COMMIT=<40-char-head-commit> \
INDEX_QUERY_EQUIVALENCE_RUN_KEY=<stable-run-key> \
  cargo run -p rustok-benchmarks --bin index-query-equivalence-capture
INDEX_QUERY_EQUIVALENCE_ALLOW_ADMISSION=1 \
INDEX_QUERY_EQUIVALENCE_BUNDLE=<fresh-capture-root> \
INDEX_QUERY_EQUIVALENCE_EXPECTED_COMMIT=<40-char-head-commit> \
INDEX_QUERY_EQUIVALENCE_EXPECTED_RUN_KEY=<stable-run-key> \
INDEX_QUERY_EQUIVALENCE_ADMISSION_OUTPUT=<existing-parent>/equivalence-admission.json \
  cargo run -p rustok-benchmarks --bin index-query-equivalence-admission
cargo check -p rustok-index --all-targets
cargo check -p rustok-social-graph --features index --all-targets
cargo check -p rustok-distribution --all-targets
cargo check -p rustok-server --all-targets
cargo check -p rustok-benchmarks --bin index-query-equivalence-capture
cargo check -p rustok-benchmarks --bin index-query-equivalence-admission
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-index-query-planner.mjs
node scripts/verify/verify-index-postgres-query-compiler.mjs
node scripts/verify/verify-index-query-result-decoder.mjs
node scripts/verify/verify-index-many-link-filtering.mjs
node scripts/verify/verify-index-query-snapshots.mjs
node scripts/verify/verify-index-postgres-reference-equivalence.mjs
node scripts/verify/verify-index-query-equivalence-capture.mjs
node scripts/verify/verify-index-query-equivalence-admission.mjs
node scripts/verify/verify-index-source-schema-registry.mjs
node scripts/verify/verify-index-query-runtime-composition.mjs
node scripts/verify/verify-index-social-graph-privacy-consumer.mjs
cargo xtask module validate index
```
