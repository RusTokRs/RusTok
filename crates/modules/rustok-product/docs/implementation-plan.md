# Implementation Plan for `rustok-product`

> Canonical large-reference hardening is tracked in
> [reference-hardening-current.md](./reference-hardening-current.md). That cursor is authoritative
> for cross-module parity with the source-certified Blog v1 reference; this long-form plan remains
> authoritative for Product-specific historical/runtime work.
>
## Current state

### Accepted variant-axis target

[Unified variant axis architecture](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md)
is the accepted target for Product variant identity. The ADR registry currently
marks the cutover as `Not started`: the legacy option vocabulary may still
exist in current code, but it must not be extended as the target model. The
implementation slice must follow the ADR's atomic zero-legacy cutover and
verification contract.


`rustok-product` owns the catalog, variants, category-bound attribute schemas,
typed attribute values, and product admin/storefront packages. Product UI uses
owner-owned core, transport, and Leptos adapter layers. Native server functions
use `HostRuntimeContext` and a typed event bus; GraphQL remains the parallel
selected path. The product packages contain no package-local framework or
framework-specific outbox adapter dependency.

`ProductCatalogReadPort` / `product.catalog_read.v1` is implemented by
`CatalogService`. Its in-process profile has live PostgreSQL execution evidence.
The Product-owned `ProductCatalogReadRuntime` gives host composition one typed
profile selector for `embedded_native` or `external` execution. The server
preserves a runtime already installed in `HostRuntimeContext` or
`ServerRuntimeContext`, otherwise composes the embedded provider once. AI,
Marketplace Listing, Order storefront native checkout, Commerce HTTP checkout,
and mounted Commerce GraphQL checkout consume the host-selected port rather than
constructing parallel `CatalogService` instances. GraphQL schema data carries the
Product runtime into a resolver-scoped task-local; directly embedded schemas
retain an explicit in-process compatibility fallback. The checkout consumer
source cutover is complete.

`rustok-product-transport` supplies a concrete tonic gRPC client/server adapter
for all three catalog-read operations. Protobuf owns RPC identity and framing
while JSON preserves Product-owned request/response DTOs and `PortContext`. The
client maps context deadlines to tonic timeouts and restores structured
`PortError` details. The server requires interceptor-provided
`TrustedProductCatalogAuthority`, verifies tenant/operation authority, and
replaces untrusted actor/claims/roles before invoking the owner port. A loopback
conformance harness covers product projection, variant-first projection,
published list pagination, typed not-found, deadline-required semantics, and
trusted actor replacement.

The transport now has a concrete service-to-service bearer authentication
boundary. `ProductCatalogGrpcBearerToken` validates visible non-whitespace ASCII,
redacts its `Debug` representation, and compares the complete `Authorization`
metadata value in constant-time. Authenticated clients attach both
`authorization` and `x-rustok-tenant-id`; the server interceptor validates the
tenant UUID, installs a server-configured trusted service actor, and authorizes
only Product catalog read operations. Authentication failures use one generic
message and never echo the credential. TLS protects the channel while bearer
authentication establishes caller identity; neither substitutes for the other.

The production host now owns explicit Product catalog deployment selection.
`RUSTOK_PRODUCT_CATALOG_PROVIDER` defaults to `embedded`; `grpc` requires both
`RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT` and
`RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`.
`GrpcProductCatalogReadConnectionConfig` requires HTTPS without
credentials/path/query/fragment, permits plaintext only for an explicitly
enabled loopback host, bounds connect timeout, and applies WebPKI TLS roots. The
server validates authentication before connecting, connects before
`bootstrap_app_runtime`, inserts `ProductCatalogReadRuntime::external(...)` into
`ServerRuntimeContext`, and lets all existing composition surfaces reuse that
same runtime. Invalid remote configuration or connection failure aborts startup
and never silently falls back to embedded execution. Invalid authentication
configuration also aborts startup before the network connection is opened.
Remote variables, including the bearer credential, are rejected in embedded
mode.

The standalone Product catalog service host is source-complete in
`rustok-product-catalog-service`. It composes PostgreSQL, the canonical
`CatalogService`, an `OutboxTransport`-backed `TransactionalEventBus`,
`ProductCatalogGrpcService`, and `ProductCatalogGrpcBearerInterceptor` without
reimplementing Product policy or persistence. The RPC surface is read-only and
does not run migrations or an outbox relay. The host is TLS-by-default; plaintext
requires both an explicit opt-in and a loopback bind. Database URLs and bearer
credentials are debug-redacted, the trusted caller actor is configured
server-side, SQL logging is disabled, and tonic shutdown is coordinated with
Ctrl-C / `SIGTERM` plus platform telemetry shutdown. Provider schema migration
remains an external deployment precondition.

The provider schema preflight is source-complete. After the PostgreSQL pool
connects and before `CatalogService`, outbox composition, or tonic listener
creation, the host performs canonical SeaORM read probes for `products`,
`product_variants`, and `sys_events`. A missing table, incompatible schema, or
insufficient read permission aborts startup with a sanitized migration-precondition
error. The host never creates or repairs schema and never enters a partially ready
serving state.

Remote consumer behavior is now source-complete through executable loopback
harnesses. Commerce builds checkout plans with a real external gRPC runtime and
asserts that remote `Unavailable` and `Timeout` errors remain typed, retryable
`read_checkout_product_projection` boundary failures; it never substitutes the
cart line snapshot for current Product authority. AI uses the same real gRPC
adapter and asserts that both failures skip catalog enrichment while preserving
typed degraded metadata. The production handler still requires operator review
and performs no persistence.

On 2026-07-29 the repository maintainer-attested that the nine requested Product
gRPC verifier, service-host, authentication, loopback conformance, Commerce, and
AI commands passed. The exact maintainer-local Git revision was not independently
observed by the implementation agent and raw logs were not retained. The bounded
attestation is stored in
`crates/modules/rustok-product/contracts/evidence/product-catalog-grpc-maintainer-test-attestation.json`.
It closes the requested unit, loopback, and remote-consumer harness commands only;
it does not prove standalone PostgreSQL provider startup or authenticated
separate-process Commerce and AI execution.

The separate-process runtime capture contract is source-complete. The locked
local-loopback runner starts `rustok-product-catalog-service`, waits for schema
preflight and listener readiness, invokes all three owner read operations through
the authenticated transport example, starts `rustok-server` with the external
Product profile, and waits for remote-provider and HTTP host readiness before
bounded shutdown. Commands and readiness markers are allowlisted; arbitrary shell
commands and non-loopback plaintext are rejected. Retained output contains source
hashes, toolchain versions, marker sets, output hashes, and byte counts only.
The capture contract guarantees that raw database URLs, bearer credentials, tenant IDs, product IDs, variant IDs, and process logs are not retained.
A successful capture would prove provider startup, authenticated owner RPCs, and consumer remote-profile initialization, but Commerce and AI business requests through the separate consumer process remain open.

Adapter and production-wiring source are complete. Consumer-behavior,
authentication, provider-host, and schema-preflight source are complete, and the
requested unit, authentication, loopback, Commerce, and AI harnesses are
maintainer-attested as passed. They were not independently run by the
implementation agent and raw logs were not retained.
Product remains `boundary_ready` rather than `transport_verified`; configured remote-profile execution evidence remain open; provider-host execution evidence remains open; schema-preflight execution evidence remains open; the standalone PostgreSQL schema-preflight runtime evidence remains open; authenticated separate-process Commerce and AI end-to-end evidence remains open.
Schema-preflight execution evidence remains open.
The separate-process capture source is complete, but its sanitized runtime packet has not been generated or retained.

The composed `rustok-ai` consumer has existing unavailable/deadline degraded-path
evidence. Commerce checkout treats Product as a hard dependency and must not
claim a cart-snapshot fallback that does not exist. The port resolves
variant-first consumer input to the owning product projection, so consumers do
not query product or variant entities. The compiled commerce checkout
channel-inventory regression executes the in-process product projection provider
before inventory preflight; it is bounded consumer evidence only and does not
close the external transport gate.

Product runtime contract, commerce transport, and module metadata remain synchronized.
The category-bound admin transport keeps native server functions as the
internal path and parallel GraphQL operations for the public/headless path.
The DB-level tenant consistency audit, `VARCHAR(32)` locale storage, catalog
search-option discovery, detached-value marker contract, and
no-compile schema guardrail are source-locked. The complete storefront/admin catalog-controls
contract carries snake_case `search`, `category_id`, `sort_by`,
`sort_direction`, and `attribute_filters` through typed UI state, native and
GraphQL adapters, Product-owned request models, and shared server-side
execution. Storefront and admin accept at most eight semicolon-separated
`code=value` attribute predicates. Product resolves each code against an active,
product-scoped, filterable definition and executes exact typed EAV equality for
localized/plain text, integer, decimal, boolean, date, datetime, select, and
multiselect storage while excluding detached values. JSON attributes are
explicitly rejected because this contract does not claim unindexed JSON
comparison semantics. The storefront facet panel renders the owner-counted buckets of the requested attribute codes:
each bucket links to the same catalog page with one `code=value` selection flipped, so selections
live in the URL, and facets whose domain is unbounded keep the free-form filter input. The facet
contract itself is shared (`rustok-grid`), while counting stays Product-owned.
Recheck on 2026-07-29.

Product write GraphQL derives tenant and actor exclusively from authenticated
contexts. Product-owned `map_product_public_error` is shared by GraphQL and
native admin/storefront transports; it keeps internal errors in structured logs
and exposes only a safe message, stable code, retryability, and correlation id.
Entity writes that publish product domain events use
`ProductWriteTransaction` to keep the outbox write and database commit in one transaction.
Admin and storefront product roots reject an explicit tenant that differs from the
host-provided `TenantContext` before accessing storage.

Product migrations enforce PostgreSQL-only execution, tenant-scoped
translation/SKU/tag identity, canonical primary categories, typed EAV option
relations, bounded JSON inputs, normalized/indexed channel visibility, and a
target schema without unused compatibility columns. The owner-local migration
fixture also verifies non-null Media-owned image identifiers and exact decimal
variant weights through `up/down/up`.
The isolated `product_postgres_migrations_support_up_down_up` fixture verifies
the complete Product migration lifecycle against PostgreSQL with owner
prerequisites and schema/constraint/index assertions.
The isolated `product_postgres_constraints_reject_invalid_and_racing_writes`
fixture proves concurrent tenant-scoped handle and SKU uniqueness as well as
database rejection of cross-tenant tags, corrupt typed EAV rows, legacy
primary-category assignments, duplicate root slugs, parent cycles, and closure
drift. Deferred database triggers validate the exact tree/closure projection at
transaction commit.
The pre-integrity `product_tenant_integrity_migration_rejects_dirty_data_and_maps_inventory`
fixture proves dirty handle, SKU, and root-slug data blocks migration and verifies
the legacy inventory-field backfill and physical column removal after cleanup.
The owner-local tenant-storage fixture rejects mixed-tenant product
translations, category parents, schema/category/attribute relations, EAV
values, and product-category joins, and verifies owner-derived translation
isolation for category, attribute, and schema copy.

`product_catalog_read_port_executes_against_postgres` exercises product,
variant-first, and published-list operations with live price/inventory
enrichment, tenant isolation, locale fallback, channel filtering, count, and
pagination. The gRPC loopback harness mirrors the same public owner operations
without taking ownership of Product persistence.
`storefront_queries_use_indexes_at_representative_scales` seeds ten tenants at
10k, 100k, and 1M total products and captures live
`EXPLAIN (ANALYZE, BUFFERS)` plans for storefront page and count SQL. The
specialized published/global-visibility index is used at all three page scales;
the count path uses it at 100k and 1M.

`CatalogService` is separated by responsibility across
`services/catalog/commands.rs`, `admin_queries.rs`, `attribute_filters.rs`,
`queries.rs`, `projection.rs`, and `tags.rs` while the public service contract
remains unchanged. Inventory state uses the owner-owned native
`rustok_inventory::BootstrapService` inside product's transaction for variant
initialization, cleanup, and available-quantity reads; this is a
documented bootstrap exception because no GraphQL/REST bootstrap contract exists
yet. Public inventory availability/reservation contracts remain inventory-owned;
the exception must be replaced if a public bootstrap transport is introduced.
Initial price creation, projection reads, and cleanup use the transaction-aware
`rustok_pricing_persistence::BootstrapService`, keeping pricing ORM ownership
outside Product without creating a `rustok-pricing -> rustok-product ->
rustok-pricing` dependency cycle.
`ProductCatalogSchemaService` is separated across `attributes.rs`,
`schemas.rs`, `categories.rs`, `values.rs`, `effective_forms.rs`, and
`virtual_categories.rs`; the parent retains shared records and validation.

## FFA/FBA status

- FFA status: `in_progress` — both owner UI surfaces exist and must preserve
  the core/transport/UI split and native/GraphQL parity.
- FBA status: `boundary_ready` — the owner port, in-process profile, host runtime,
  declared consumer source cutovers, external gRPC adapter, validated connection
  policy, production client wiring, service-to-service authentication, standalone
  provider host, startup schema preflight, Commerce/AI remote behavior harnesses,
  and the separate-process runtime capture contract are source-complete. The
  requested unit, authentication, loopback, Commerce, and AI harnesses are
  maintainer-attested as passed without retained raw logs. Runtime capture
  execution and authenticated Commerce/AI business-request evidence remain open.
- Structural shape: `core_transport_ui`
- Evidence: `crates/modules/rustok-product/contracts/product-fba-registry.json`,
  `crates/modules/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/modules/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `crates/modules/rustok-product/contracts/evidence/product-catalog-grpc-maintainer-test-attestation.json`,
  `crates/modules/rustok-product/contracts/evidence/product-catalog-separate-process-runtime-contract.json`,
  `crates/modules/rustok-product-transport/tests/port_conformance.rs`,
  `crates/modules/rustok-product-transport/examples/product_catalog_runtime_probe.rs`,
  `crates/modules/rustok-product-catalog-service/src/main.rs`,
  `crates/modules/rustok-commerce/tests/product_remote_consumer_behavior.rs`,
  `crates/modules/rustok-ai/src/direct_product_attributes.rs`,
  `scripts/evidence/capture-product-catalog-separate-process-runtime.mjs`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-catalog-read-runtime-composition.mjs`,
  `scripts/verify/verify-product-native-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-http-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-catalog-grpc-transport.mjs`,
  `scripts/verify/verify-product-catalog-grpc-deployment.mjs`,
  `scripts/verify/verify-product-catalog-grpc-authentication.mjs`,
  `scripts/verify/verify-product-catalog-grpc-service-host.mjs`,
  `scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.mjs`,
  `scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs`,
  `scripts/verify/verify-product-remote-consumer-behavior.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-admin-category-sort.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`,
  `scripts/verify/verify-product-storefront-category-sort.mjs`,
  `scripts/verify/verify-product-catalog-attribute-filters.mjs`,
  `scripts/verify/verify-product-catalog-controls-plan-sync.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Run the source guard before execution:
   - `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs`;
   - `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.test.mjs`.
   Then execute `node scripts/evidence/capture-product-catalog-separate-process-runtime.mjs`
   from a clean commit with migrated provider and consumer PostgreSQL databases,
   matching loopback bind/endpoint values, one bearer credential, and existing
   tenant/product/variant fixtures. Commit the generated sanitized evidence packet.
2. After a successful capture, retain authenticated Commerce checkout and AI
   product-generation business requests through the separately running consumer
   server. Promote above `boundary_ready` only after those results and their
   sanitized logs or CI artifacts are retained.
3. Keep Product richtext adoption explicitly deferred until the owner approves
   a typed storage/API/index migration. `product_translations.description` and
   catalog attributes currently named `richtext` are scalar text, so replacing
   their textarea alone would create a false contract. When approved, use the
   shared [Richtext plan](../../../../docs/modules/rich-text-implementation-plan.md),
   assign an owner profile, migrate both transports, and keep short/meta
   descriptions plain text.

## Verification

- [x] Compose one host-selected `ProductCatalogReadRuntime` and reuse it for AI and Marketplace Listing.
- [x] Cut Order storefront native checkout over to the composed Product runtime.
- [x] Cut Commerce HTTP checkout over to the composed Product runtime.
- [x] Cut mounted Commerce GraphQL checkout over to the composed Product runtime.
- [x] Implement the concrete Product catalog gRPC adapter and loopback conformance harness.
- [x] Wire a fail-closed production external Product runtime profile.
- [x] Add service-to-service bearer authentication and trusted tenant metadata.
- [x] Add executable Commerce hard-dependency and AI degraded-behavior gRPC harnesses.
- [x] Implement a standalone Product catalog service host.
- [x] Fail closed on missing Product/outbox schema before owner composition or listener startup.
- [x] Execute the Product catalog service-host unit, authentication, and loopback conformance test suites.
- [x] Execute the Commerce and AI remote consumer behavior harnesses.
- [x] Lock a reproducible separate-process Product runtime evidence capture contract.
- [ ] Execute the separate-process Product runtime capture and retain its sanitized evidence packet.
- [ ] Execute the standalone PostgreSQL schema preflight and retain runtime logs.
- [ ] Retain authenticated separate-process Commerce and AI end-to-end evidence.
- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.
- [x] Connect storefront title search through typed UI state, native/GraphQL transports, and Product-owned server-side filtering.
- [x] Connect storefront category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect admin search/status/category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect typed attribute_filters through storefront/admin UI state, native/GraphQL transports, filterable-definition validation, and Product-owned typed EAV execution.
- [x] Render storefront facet buckets from the Product owner contract: the shared `rustok-grid` facet contract, owner-counted `code=value` buckets, and URL-toggled selections in the storefront facet panel.
- `node scripts/verify/verify-product-catalog-read-runtime-composition.mjs`
- `node scripts/verify/verify-product-catalog-read-runtime-composition.test.mjs`
- `node scripts/verify/verify-product-native-checkout-catalog-runtime.mjs`
- `node scripts/verify/verify-product-native-checkout-catalog-runtime.test.mjs`
- `node scripts/verify/verify-product-http-checkout-catalog-runtime.mjs`
- `node scripts/verify/verify-product-http-checkout-catalog-runtime.test.mjs`
- `node scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs`
- `node scripts/verify/verify-product-graphql-checkout-catalog-runtime.test.mjs`
- `node scripts/verify/verify-product-catalog-grpc-transport.mjs`
- `node scripts/verify/verify-product-catalog-grpc-transport.test.mjs`
- `node scripts/verify/verify-product-catalog-grpc-deployment.mjs`
- `node scripts/verify/verify-product-catalog-grpc-deployment.test.mjs`
- `node scripts/verify/verify-product-catalog-grpc-authentication.mjs`
- `node scripts/verify/verify-product-catalog-grpc-authentication.test.mjs`
- `node scripts/verify/verify-product-catalog-grpc-service-host.mjs`
- `node scripts/verify/verify-product-catalog-grpc-service-host.test.mjs`
- `node scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.mjs`
- `node scripts/verify/verify-product-catalog-grpc-maintainer-test-attestation.test.mjs`
- `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs`
- `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.test.mjs`
- `node scripts/evidence/capture-product-catalog-separate-process-runtime.mjs`
- `node scripts/verify/verify-product-remote-consumer-behavior.mjs`
- `node scripts/verify/verify-product-remote-consumer-behavior.test.mjs`
- `cargo test -p rustok-product-catalog-service`
- `cargo test -p rustok-product-transport --lib`
- `cargo test -p rustok-product-transport --test port_conformance`
- `cargo test -p rustok-commerce --test product_remote_consumer_behavior`
- `cargo test -p rustok-ai --features server --lib remote_product_`
- `node scripts/verify/verify-product-catalog-attribute-filters.mjs`
- `node scripts/verify/verify-product-catalog-attribute-filters.test.mjs`
- `node scripts/verify/verify-product-admin-category-sort.mjs`
- `node scripts/verify/verify-product-admin-category-sort.test.mjs`
- `node scripts/verify/verify-product-storefront-category-sort.mjs`
- `node scripts/verify/verify-product-storefront-category-sort.test.mjs`
- `node scripts/verify/verify-product-catalog-controls-plan-sync.mjs`
- `node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs`
- `npm run verify:product:runtime-fallback-smoke`
- `npm run verify:product:admin-boundary`
- `npm run verify:product:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `cargo test -p rustok-ai --features server --lib direct_product_attributes_`

## Boundaries

- Product owns catalog data, `ProductCatalogReadPort`, and
  `ProductCatalogReadRuntime` profile selection.
- `rustok-product-transport` owns tonic/protobuf framing, validated client
  connection policy, deadline/status mapping, bearer metadata, constant-time
  credential comparison, and trusted-authority adaptation. It must not own
  Product policy, persistence, DTOs, locale/channel rules, fallback decisions,
  or deployment secret storage.
- The consumer server host owns deployment variables, endpoint/TLS selection,
  bearer credential injection, startup connection, and insertion of the selected
  runtime. It must fail closed for invalid authentication, invalid configuration,
  or an unavailable configured remote provider and must not silently select
  embedded execution.
- The standalone provider host owns only deployment composition: PostgreSQL pool,
  required-schema read probes, canonical `CatalogService`, `OutboxTransport`,
  tonic TLS/listener lifecycle, bearer interceptor configuration, telemetry, and
  graceful shutdown. It does not run migrations, issue DDL, expose write RPCs,
  relay outbox rows, or duplicate Product policy/persistence. It configures the
  trusted service actor server-side, verifies `products`, `product_variants`, and
  `sys_events` before serving, and never trusts actor/claims/roles supplied in
  `PortContext`.
- The retained evidence runner owns orchestration only: fixed process commands,
  local-loopback validation, bounded readiness/shutdown, source/output hashing,
  secret-leak rejection, and atomic JSON output. It does not seed data, run
  migrations, persist raw logs, alter Product policy, or claim Commerce/AI
  business-request execution.
- Commerce owns hard-dependency checkout behavior: a Product timeout or
  unavailable result blocks planning and cannot be replaced by the cart snapshot.
- AI owns advisory degradation: Product transport failures skip enrichment,
  preserve typed degraded metadata, require operator review, and never persist
  generated suggestions automatically.
- The host selects and shares one Product read runtime; consumers receive the
  public port and must not construct parallel owner services.
- Order native checkout, Commerce HTTP checkout, mounted Commerce GraphQL
  checkout, Marketplace Listing, and AI consume Product's public read contract
  through host composition. Directly embedded GraphQL schemas retain an explicit
  in-process compatibility fallback. Pricing uses Product's public embedded
  service contract and does not claim a read-port fallback profile. None regain
  Product DTO, entity, or storage ownership.
- Hosts compose product UI packages and pass the effective locale and runtime
  context without adding a package-local locale or transport fallback.

## Source Layout: Port Responsibility Modularization

Per `docs/backend/module-backend-implementation.md`, `src/ports/` is organized into canonical responsibility submodules bounded below 32 KiB:
- `ports/mod.rs`: Module facade, documentation, and public re-exports.
- `ports/types.rs`: Request and projection DTOs (`ProductProjectionRequest`, `PublishedProductsRequest`, etc.) and pagination bounds validation.
- `ports/diagnostics.rs`: Structured telemetry, bounded context/error facts, tenant parsing, and stable error mapping without consumer type coupling.
- `ports/catalog_read.rs`: `ProductCatalogReadPort` trait definition and `CatalogService` adapter implementation.

## Workflow remediation (2026-10-17)

Base: `bc1046c10f51fab070f7ad7c0417dca648a13e16`; implementation branch:
`arena/8161797f-rustok`. Status: **incomplete; not runtime certified**.

Implemented in this slice:

- Validate Product updates against the persisted candidate category/translations within the same
  transaction, including edits that remain Active; failed requirements roll back the document.
- Lock Product before reading category/form/definitions/options for typed product-value patches;
  validate Active completeness after writing patches and before committing.
- Make repeated dedicated lifecycle commands no-ops without moving revision or timestamps.
- Preserve all supplied translations, SEO, category, shipping profile, barcode, compare-at price,
  inventory policy, and explicit zero prices in the Next create adapter.
- Fence stale category/locale form responses and clear obsolete forms on selection/error.

Verification:

- Four executable Next create-adapter tests in `product-create.test.mjs` pass, using the actual
  adapter and its injected executor (not a reimplementation of the payload mapping).
- The existing lifecycle-event source guard and its seven mutation tests pass. These prove source
  contract wiring, not runtime transaction, event-count, or PostgreSQL concurrency behavior.
- `verify-product-next-data-table-host.mjs` passes. The separate
  `verify-commerce-product-command-port.mjs` fails two pre-existing exact-text assertions in the
  unchanged Commerce HTTP controller sources (create header documentation and a multiline
  `headers.get` lookup). This guard is **not green**; no controller or guard was altered to mask it.
- The remediation gate with `--skip-cargo` passed diff invariants but failed its architecture
  dependency step because that step still requires `cargo metadata`. The gate did not pass.
  Frontend formatting was run with the application's formatting options but without the unavailable
  Tailwind plugin. Full Next typechecking, mounted UI tests, Rust formatting/compilation and
  PostgreSQL tests remain unverified.
- Added `repeated_lifecycle_commands_preserve_revision_and_timestamps` and
  `category_change_validates_candidate_before_publishing_or_remaining_active` to the established
  Commerce catalog service fixture. They have **not** run: this sandbox has no Cargo/Rust compiler.
  Maintainer gate: `cargo test -p rustok-commerce --test catalog_service_test`, followed by the
  scoped Product checks and PostgreSQL concurrency coverage. No full-workspace suite was run.

Remaining before closing the original workflow audit:

1. Execute the source-implemented atomic catalog document/category/typed-value/publication command
   against the database and mounted clients (see the atomic editorial follow-up below).
2. Retain immutable Next logical-attempt keys through ambiguous outcomes; verify replay and revision
   refresh. The compensating-delete editor workflow has been removed, not treated as atomicity.
3. Review nullable Variant patch semantics across all writers/readers before adopting `Patch<T>`.
4. Implement Media durable retention only after resolving the Proposed Product-image ADR through
   its owner contract; include stable relation identity, receipt/retry behavior, release and reconciliation.
5. Remove unresolved catalog-price fallback from Next storefront in favor of Pricing-owned context
   resolution, with mounted unavailable/no-price behavior tests.
6. Exercise concurrent category/value/schema edits on PostgreSQL; the Product lock does not by
   itself serialize independent schema administration or provide a repeatable-read schema snapshot.

Audit correction: the optional owner-internal document revision is explicitly documented in
`dto/product.rs`; external document transport checks, not a blanket prohibition of internal
unconditional writes, own its admission. No change to that accepted boundary was made here.

### Deeper workflow follow-up

Additional source changes in the same unintegrated slice:

- Product detached-value clearing now resolves detachment after the Product write lock, not
  before it. A category change can no longer invalidate the preflight between validation/deletion.
- Variant patch and detached clear now lock Product then Variant before resolving the form,
  definitions, options or detachment. The helper rechecks observed Variant ownership and uses
  backend-aware locking so SQLite does not execute unsupported `FOR UPDATE`.
- Exact-locale Product/Variant attribute-value Translation apply paths previously locked the
  value first and touched its owner afterward, which conflicts with catalog/value commands.
  They now discover owner identity without using that read for validation, acquire Product
  (then Variant where applicable), and finally lock the value with owner-identity predicates.
  Snapshot/CAS checks still run after those locks, and receipt completion remains in the same
  transaction. PostgreSQL Translation-target execution remains required.
- The Next status action no longer passes a partial document to `saveProductAction` (which
  converted omitted vendor/seller/shipping fields to null). A dedicated callback submits only
  `{status, revision}` through the existing Product update API, with status acknowledged on
  success only. Unsaved document fields and dirty attribute patches are untouched by status actions.
- Same-status lifecycle-only `update_product` is a no-op after predecessor validation, matching
  dedicated publish/unpublish/archive commands.

Verification: `npm test --prefix apps/next-admin/packages/rustok-product` passes 11 tests (four
create adapter and seven lifecycle action/handler tests). These execute extracted production
functions with injected dependencies; they are not mounted React or real GraphQL execution.
The attribute-validation verifier passes, with 17 fixture/mutation tests including seven new
lock/read-order regressions. The lifecycle verifier and its seven fixture tests also pass.
Two additional Rust service regressions cover same-status predecessor validation and rejection
of clearing reattached Product/Variant values; all four new Rust tests remain unexecuted because
Cargo is absent. PostgreSQL concurrent interleavings and schema-administration races remain open.
`verify-product-catalog-schema.mjs` currently fails an exact-text assertion for
`publish_in_tx_with_envelope_id(&self.transaction` in unchanged `write_transaction.rs`; this is
not evidence of a passing storage gate and was not bypassed.

Further confirmed blockers:

- **Next money transport mismatch:** Commerce GraphQL `PriceInput.amount` and
  `compare_at_amount` are decimal `String` fields, and `GqlPrice` serializes Decimal with
  `to_string()`. Next create/variant inputs currently send numbers; the variant editor uses
  fixed two-decimal integer presentation (`* 100` / `/ 100`) while catalog responses are returned
  without an explicit boundary conversion. A displayed 12.50 becomes the number 1250 in a write
  payload instead of the required decimal string `"12.50"`; real GraphQL scalar validation must
  reject the numeric String input. Read-side strings can also fail numeric rendering. The four
  create mock tests certify field preservation only, not schema admission or money correctness.
  Fix this through one canonical decimal money contract and audit both directions, currency
  precision, compare-at values, pricing previews and zero/invalid values; do not add another
  implicit `/100` at a random caller.
- **Price replacement scope:** catalog `update_variant(prices: Some(...))` deletes all price
  rows for the Variant and bootstraps rows without price-list/region/quantity context. The Next
  editor preserves only currency/amount/compare-at in its mutation payload. Before allowing that
  path to edit an already context-priced Variant, define Pricing-owned replacement/upsert semantics
  so saving SKU with a price payload cannot erase channel/list-specific rows.
- **Lifecycle receipts:** the existing [write boundary](./write-boundary.md) explicitly records
  caller-key validation without durable lifecycle replay. Responses are projected after commit.
  Atomic editor save must resolve this pre-commit response/receipt gap and the intentionally
  separate EAV receipt boundary; wrapping the existing two services does not provide atomicity.
- **Both editors:** Leptos `admin/src/ui/leptos.rs` also saves the Product first and typed values
  afterward. Atomic-save remediation must migrate that path as well as Next, not create a Next-only
  owner contract. Current `reference-hardening-current.md` was synchronized with implemented
  Product patches/revisions without marking aggregate concurrency complete.

### Decimal money and catalog base-price scope follow-up

This source slice supersedes the money-mismatch/destructive-replacement findings above; backend
execution is still required before they can be considered runtime-certified.

- Next Admin create, Variant create/update, reads, drafts and host actions use decimal strings in
  major units. The Pricing adapter no longer coerces returned amounts through `parseFloat` or
  `*100`. Compare-at zero and null are distinct. `decimal.js-light` is a **direct** module dependency
  with its host lockfile record updated; the editor uses exact decimal arithmetic for draft
  discounts, validates supported Decimal range, and rejects invalid input rather than writing zero.
  It contains no currency exponent table or currency rounding policy.
- Variant editing starts with no price rows if the owner returned none, permits clearing the last
  base row, keeps the dialog/draft on owner errors, and validates integer inventory independently.
  The discarded price-list selector is removed rather than claiming unsupported scoped writes.
- Pricing persistence now owns a shared base-row predicate for catalog reads/replacement: all
  list/channel/channel-slug/region/quantity context columns must be null. Catalog replacement uses
  this predicate inside the existing owner transaction; full deletion remains for actual entity
  deletion. Scoped input on Variant update is a validation error. Catalog snapshots return only
  round-trippable base prices, not anonymous mixtures of contextual rows.
- Next Product and Pricing storefront amount **and discount-percent** DTOs agree with GraphQL
  strings. Product detail displays exact strings and trusts owner `onSale`; it no longer calls
  `.toFixed` on strings or compares decimal strings lexically. Nullable SKUs match GraphQL and are
  never overlay join keys. Pricing contributes only prices/effective-price by ID, never inventory.
  Missing/empty Pricing cannot fall back to catalog prices, and unavailable prices disable add.
  This is not proof of complete currency/region/quantity context selection or Cart money hardening.

Evidence for this follow-up:

- `npm test --prefix apps/next-admin/packages/rustok-product`: **22/22** (six create, seven lifecycle,
  three exact-money and six Variant dialog/Pricing adapter tests). Adapter/dialog tests execute
  production functions with injected dependencies; they are not mounted React tests.
- `node --test apps/next-frontend/packages/rustok-product/src/catalog/pricing.test.mjs`: **4/4**;
  runtime overlay tests plus a source rendering assertion, not mounted unavailable-state evidence.
- Targeted TypeScript checks pass for the money helper/Admin DTOs and storefront pricing merge/API
  DTO dependency closure. The latter uses a temporary standalone TS 5.9.3/Apollo 4.0.9 toolchain,
  not the application's locked full build. Full app/React typechecking remains open.
- Added two Rust persistence regressions for base replacement/clear, each of six independent
  contexts, another Variant, and refusal before deletion; added actual `PriceInput::parse` GraphQL
  scalar-admission regression. **None has run**: Cargo/rustc are absent. They do not certify
  PostgreSQL concurrency or physical schema/migration compatibility.

Required owner commands once a Rust toolchain is available:
`cargo test -p rustok-pricing-persistence`,
`cargo test -p rustok-commerce --lib price_input_requires_major_unit_decimal_strings_at_graphql_admission`,
and `cargo test -p rustok-commerce --test catalog_service_test`, followed by PostgreSQL/context
integration coverage. Pre-commit public projections, durable lifecycle replay and atomic editor
save across both frontends remain the next major implementation boundary, not completed work.

### Catalog pre-commit projection and receipt follow-up

This follow-up supersedes the earlier source-level lifecycle-receipt-gap findings, without claiming
runtime readiness or atomic editor save.

- Every concrete catalog mutation now records its actual public result before commit: Product,
  Variant, Image, axes, unit deletion/reorder results and all four same-status branches. Public
  Product/Variant projections accept the write connection; no response query runs after commit.
- Tag relations and localized Taxonomy names now use that connection too, so newly created terms
  are visible before commit without reading Taxonomy persistence from Product. The existing
  Taxonomy read method delegates to the same owner implementation on its ordinary connection.
  Image update projects localized alt text and translations from its transaction, including
  position-only edits, instead of presenting omitted input as the actual stored alt text.
- All fourteen `ProductCatalogCommandPort` operations admit tenant-scoped receipts under owner
  `product`. Request hashes bind actor/locale/IDs/input; successful and deterministic-error retries
  replay the stored result. Shared transaction completion/failure finalization is reused rather
  than implementing a second ledger or persisting success after commit. Product/Variant root
  create IDs use their operation UUID. Existing transport key derivation is unchanged.
- Media retention remains Proposed. The existing live Media validation decorator may reject an
  image attempt before it reaches owner replay; this is explicitly not receipt-aware Media
  workflow/retention certification. Its ordering must be settled with the accepted Media contract.
- Next logical-attempt key retention, preflight transport reads on replay, nullable Variant patches
  and database concurrency evidence remain. Atomic editorial source was subsequently cut over below.

Evidence: `verify:product:catalog-receipts` passes, with eleven source mutation tests. These check
wiring and ordering only. Added four Commerce Rust regressions for original-snapshot replay,
changed-request refusal, delete/error replay, image-localization snapshot and projection-failure rollback; added a Taxonomy
transaction/tenant/rollback regression. They have **not run**, because Cargo/rustc are absent.
Tree-sitter parses the changed Rust files without syntax errors; that is not compilation, type
checking, rustfmt or database execution. Maintainer commands:

- `cargo test -p rustok-commerce --test catalog_service_test catalog_`
- `cargo test -p rustok-taxonomy --test owner_read`
- `cargo test -p rustok-product --lib services::write_transaction`
- the existing Pricing/money regressions and scoped PostgreSQL tests.


## Atomic editorial follow-up (2026-10-17)

Canonical create/update DTOs now carry typed locale/patch batches. Catalog owner transactions apply
all batches after persisting the candidate document/category and before final Active validation,
receipt encoding and commit. Canonical duplicate locales, duplicate attributes in a batch, and
cross-locale edits of the same nonlocalized attribute are refused; there is no silent batch-order
last-write-wins. EAV-only catalog edits count as document edits and require a predecessor revision
at the command port. Standalone Product EAV save/clear and exact-locale Translation writes advance
that revision; the embedded writer does not create another receipt, transaction or revision.

Next sends one catalog command and never compensates a receipted create with deletion. Its create
selection now uses actual `GqlProduct` fields (`id`, `revision`), not list-only `title`/`handle` fields.
The mounted Leptos `ui/product_editor.rs` now binds typed fields to the document draft for both new
and existing products; its shared section no longer has an independent value-save button. The
non-mounted reference composition was migrated too. Both Rust lifecycle input mapping and the
standalone schema adapter share GraphQL enum normalization, including textual editor kinds.
The unreachable unkeyed lifecycle implementation in `graphql_adapter.rs` was deleted. Value
refreshes preserve dirty drafts until catalog acknowledgement. Redundant sequential status save
was removed; a rejected document save cannot proceed to Variant side effects.

This scope is **document/category/Product EAV/publication**, not every operation visible on the
screen. At that intermediate slice the mounted Leptos editor still saved its existing default
Variant in a second command and reported partial success. The explicit-action cutover below
supersedes that flow without claiming whole-screen atomicity. Mounted save/status/delete logical-attempt identity and typed Server Action outcomes were
subsequently implemented below; browser-journal recovery is implemented in the next follow-up,
while child mutation identity remains open. Media retention is Proposed.

Added five Commerce regressions for required-locale create/publish/replay, aggregate update with
one revision, late-batch rollback, invalid-create cleanup, and stale document refusal after an
independent value edit; two Admin Rust tests cover GraphQL enum normalization and dirty-state
refresh/acknowledgement. All seven are **unrun**: Cargo/rustc are absent. Source/mutation guards and
Node adapter tests do not replace them. Maintainer gates include:

- `cargo test -p rustok-commerce --test catalog_service_test atomic_editor`
- `cargo test -p rustok-commerce --test catalog_service_test standalone_attribute_write`
- `cargo test -p rustok-product-admin --lib`
- the existing scoped Product Translation PostgreSQL tests and failure/commit-ambiguity injection.

`verify:product:atomic-save` and its mutation tests pin the shared writer, final publication order,
revision participation and both mounted editor cutovers; they are source checks only. Targeted TS
checks cover actual Product API files and the create adapter extracted from its UI barrel with
real host/Apollo declarations; this is not a full application or mounted React build.


## Next editorial retry follow-up (2026-10-17)

Mounted Next save/status/delete now share a synchronous attempt lock. A retained request is cloned
and frozen with its caller UUID, tenant/actor scope, predecessor, translations, initial Variant and
EAV patches. Repeating an unknown request uses that same snapshot, never current control state.
An in-flight double click does no IO; an unresolved command cannot be replaced by another operation.
An acknowledged create/delete cannot execute again during delayed navigation. Native fields and
portal mutation callbacks are guarded while the editorial attempt is unresolved.

Canonical create/update/delete adapters forward the supplied key unchanged. Server Actions require
a valid caller UUID, compare captured scope with one live authenticated session, and return explicit
confirmed/rejected/unknown values. Unknown/redacted/network errors preserve the key. Failed cache
invalidation after owner acknowledgement does not undo success. Initial price/compare-at/inventory
scalar admission happens before the request is retained; dirty fields are not silently rebased to a
new predecessor merely because server props refresh.

Apollo 4 error handling now preserves `code`, `retryable` and `outcome` through its actual
`CombinedGraphQLErrors.errors` shape. The Commerce Product error mapper marks only bounded,
nonretryable, receipt-finalized domain codes as `outcome: rejected`. Context/admission failures can
map to the same public `PRODUCT_VALIDATION` code, so `retryable: false` alone never releases a key.
Other errors, auth refusals, missing metadata and receipt conflicts remain unknown. No legacy
`graphQLErrors` fallback or arbitrary exception-message classification remains in that executor.

Evidence: 56 Product Admin Node tests pass, including actual extracted save/status handlers,
canonical adapters and Server Action control flow with injected IO; 57 source/mutation guard tests
and four storefront tests pass. Targeted strict TS covers actual Product API/attempt/outcome files,
the create adapter extracted from its UI barrel, and real host/Apollo declarations. The actual
cached Apollo 4.0.9 executor was also checked with mocked HTTP for owner refusal metadata and
successful data. This is not a full host build, mounted React execution, actual manifest-version
runtime attestation, live backend commit-ambiguity test, or Rust compilation. A Commerce Rust unit
test for domain-vs-admission outcome classification was added but is **unrun** without Cargo/rustc.

The mounted-only limit of this intermediate slice is superseded by the browser-journal cutover
below. Child callback identity, preflight replay liveness, Leptos secondary Variant saves and the
Proposed Media ADR are not certified by this evidence.

## Next browser recovery follow-up (2026-10-17)

The canonical `write-journal.ts` persists a strictly validated Product request before owner IO,
scoped by tenant/actor/resource and the URL `editor_intent` UUID. `write-attempt.ts` restores that
snapshot, serializes duplicated URLs with Web Locks and settles them with a body-free tombstone.
There is no production in-memory fallback or automatic record expiry. Explicit recovery bypasses
current-field validation and uses the original operation, caller UUID, scope and predecessor.
Lost/corrupt live identity and storage/lock failures fail closed. Clearing/evicting browser data,
receipt expiry and cross-device recovery are outside this browser projection's guarantee.

The editor starts disabled until initialization, fences portal callbacks synchronously, displays
pending/settled/error recovery controls and deliberately starts a new intent only after a terminal
decision. A confirmed existing editor opens a fresh document rather than reusing stale fields.
A rejected draft can explicitly begin a new intent preserving its old predecessor and fields.
Missing Product reads with a valid intent mount a recovery-only screen so an uncertain delete
can still replay; no replacement draft or authority is inferred from the URL. Route mount keys
include scope/resource/intent, and native query changes cannot send an old mounted intent.

Evidence: 86 Product Admin Node tests cover canonical journal validation, persistence-before-send,
reload recovery for all three commands, duplicate-tab lock simulation, tombstone reuse refusal,
terminal-storage failure, scope/resource/intent corruption, invalid JSON coercion and browser
factory/URL admission (including the asynchronously acquired lock).
Extracted production editor handlers/initialization execute with injected IO; they are not mounted
React tests. The actual host route JSX also passed six new/existing/recovery-only/invalid-intent
cases in an injected-read smoke (not browser or backend execution). Scoped strict TypeScript
7.0.2 (the host manifest version) passes for the actual editor and its transitive UI dependencies,
plus the API/controller/outcome/host executor scope; cached Next is the declared 16.4.0 and Apollo
resolves within the manifest range to 4.3.4. This is not a whole-host build or live Apollo/backend
runtime attestation. Source/mutation guards and storefront pricing tests are separate evidence.

Remaining: real browser reload/history/duplicate-tab/disabled-storage execution against owner
receipts, backend failure injection and Rust/PostgreSQL verification; retained identities and
acknowledgements for Variant/Image/axes/Relation/Bundle/Pricing callbacks; preflight receipt-replay
liveness; Leptos pending-request recovery and nullable Variant patches. The secondary-Variant
Save behavior is superseded by the explicit-action cutover below. Media
retention ADR remains **Proposed**; neither module readiness nor the sequential audit cursor advances.


## Leptos explicit Variant action follow-up (2026-10-18)

Existing Product document Save no longer invokes `update_product_variant` or reports a partially
applied multi-command save. Price/SKU/barcode/stock controls have an independent operator action,
show the exact Variant ID hydrated into the controls and keep that target even when a Product
acknowledgement returns a reordered Variant list. Missing targets refuse sending rather than
silently editing a different first row. Ordinary edit Save uses selected status instead of
implicitly publishing every edit; creation Publish remains explicit.

Variant controls are captured before bootstrap/IO, with a synchronous shared busy guard and a
disabled editor fieldset. Each successful action updates its own confirmed snapshot rather than
triggering a full-editor reload over unrelated dirty document/EAV or Variant fields. Invalid
money/inventory scalars are refused locally; blank amount leaves prices untouched instead of
creating a free USD row. Unsupported nullable SKU/barcode clears are refused explicitly, not
reported as successful no-ops. Full nullable patch transport is still open. SKU/stock controls are visible in the same Variant
card for every product kind, and mounted initial creation now forwards its selected inventory policy
instead of hardcoding Deny.

Evidence: the atomic-save gate now has 22 source/mutation tests, including ten new regressions
for hidden Variant commands, dirty-field refetch, post-IO signal reads, retargeting, missing local
admission/action, lost document acknowledgement, forced publication and stale invalid inventory. The scoped combined
source/mutation suite has 67 passing tests. Product Admin Node (86) and storefront pricing (4)
remain separate evidence. Tree-sitter parses the four changed Rust sources, but Cargo/rustc/rustfmt
are absent: five new Rust tests, compilation, formatting and mounted/database behavior are
**unrun**, not replaced by source checks. Maintainer: `cargo test -p rustok-product-admin --lib` plus the scoped SSR/hydrate checks and mounted dirty-draft/action scenarios.

Replay trace: direct delete reaches owner admission without a live Product lookup. Confirmed
remaining barriers are Commerce GraphQL Product/Variant Shipping Profile preflight, REST Product
Shipping Profile preflight, and the Media validation decorator. Simply deleting checks or querying
owner receipt tables from foreign adapters is not a fix. Composed receipt-aware policy admission,
Leptos immutable retry snapshots and child-command browser recovery remain open. Media retention
ADR stays **Proposed**; module readiness and audit integration status do not advance.

## 2026-10-18: accepted Product image / Media retention implementation

This entry supersedes older `Media remains Proposed` / read-only-decorator statements in
historical slices above. The explicit implementation request accepts the Product-image ADR;
its implementation status remains **In progress**, not Completed or runtime certified.

Source changes:

- Removed the read-only outer command decorator and opaque image-write fallback. The
  Product-owned consumer port is installed inside `CatalogService` after owner admission;
  completed receipts replay without Media. Stable receipt operation UUIDs allocate image IDs
  before retain; direct trusted internal execution still generates its own ID.
- Host adapts Media's public read/write controls, using an atomic `MediaAssetPorts` pair
  from one owner instance. A remote read cannot be mixed with an embedded writer. Supported
  gRPC control-client composition has an independent `media-grpc` host feature, without
  requiring Profiles. No new remote Product command deployment is declared verified.
- Retain + current-hold confirmation precede the local Product transaction; parent existence
  is rechecked under the Product lock. Image, provisioning coverage, event and receipt commit
  together. Failure/unknown commit never compensates a hold.
- Image/Product deletion atomically stores exact committed release work with removal/event/
  receipt. A role-gated bounded worker uses stable keys, bounded fresh delivery deadlines,
  backoff, current-absence acknowledgement, restart-safe queue rows and StopHandle cancellation.
  Postcommit release failure never changes the committed Product response into an error.
- Additive transactional migration 000037 creates the release queue, FK-cascading per-image
  provisioning coverage and sealed per-tenant readiness. Legacy backfill preserves IDs;
  incomplete readiness is retryable/unavailable, not a terminal caller validation error.
  Steady-state admission uses a readiness point lookup. Rollback refuses undrained obligations.
- Privileged bounded owner maintenance ports and Commerce HTTP/OpenAPI expose diagnostic-only
  audit, explicit backfill and queue status. Status does not require live Media capability.
  Native operator permissions use effective Product permissions; HTTP rejects mismatched
  tenant/actor request context. REST now propagates authenticated claims and request correlation.
- Product remains independent of Media crates/persistence. Its new consumer registry declares
  both Media contracts, no fallback profiles, and `source_implemented_runtime_pending`.
  Media and Ecommerce FBA metadata source checks pass; no runtime attestation was fabricated.
- Existing lifecycle/source guards and fixtures no longer require the removed decorator. The
  historical category migration test now pins its intended migration by name, not moving head.

Executed in this workspace:

- Seven workflow guard/mutation suites: **134 PASS** (including **19** new retention cases).
- All seven corresponding standalone source guards plus six Admin source checks: **PASS**.
- Media, SEO and Ecommerce FBA metadata/source checks: **PASS**.
- Product Admin Node suite: **86 PASS**; storefront pricing suite: **4 PASS**.
- Two scoped TypeScript **7.0.2** configurations: **PASS** (Admin product transport/domain and
  reachable editor dependencies). These do not certify the whole host.
- **28** touched Rust files parsed with tree-sitter: **PASS**; source formatting through
  `@scalar/rust-fmt` **0.3.0** (WASM rustfmt) is idempotent. Neither is Rust typechecking or a
  repository-toolchain `cargo fmt --check` run.
- `git diff --check`: **PASS**.

Added **20 Rust tests**, all **unrun**: seven Catalog owner transaction/replay cases; four
owner context/permission/deadline/error cases; two real SQLite/PostgreSQL local protocol fixtures
(with a consumer-port fake, not Media GC proof); seven host/key/projection/composition cases
(including a feature-gated gRPC client pair construction check, not live RPC).

Open gates: Rust **1.96** compilation and supported feature matrices; execution of SQLite and
PostgreSQL fixtures; real Product→native/remote Media GC fences; ambiguous commit/lease reclaim;
process crash/restart and multi-replica queue delivery; authenticated mounted operator routes
and both admin image-command transports. `cargo`, `rustc`, PostgreSQL and mounted backend execution
are not available here. Do not promote the ADR/module readiness, advance the sequential audit
cursor, or treat source/mutation tests as runtime evidence.

The [workflow/cutover runbook](./media-reference-retention.md) records the mandatory legacy-writer
and Media-delete/cleanup maintenance window, bounded backfill/retry procedure, queue inspection,
rollback constraints, and maintainer execution commands. This slice does not fix the independent
Shipping Profile preflight, nullable Variant patch semantics, or child-editor durable retries.


### 2026-10-18 continuation: release replica race and real-owner integration sources

The release worker now conditionally updates failure metadata by image ID and original
attempt count. A stale failure cannot resurrect acknowledged work, overwrite a newer
retry/backoff, or abort delivery of the remainder of the batch. Two deterministic
SQLite queue regressions cover both interleavings; the isolated PostgreSQL protocol
fixture invokes both scenarios as well. Their Media port remains a controlled fake.

New server integration sources exercise actual MediaService, migrations and local PNG
storage against separate Product/Media databases, natively and via live loopback gRPC
with fixed trusted-tenant authority and an explicit operation allowlist. Scenarios cover
cross-tenant refusal, GC/delete fences, stale retain and release receipts after repair,
live-image release refusal, and eventual queue draining/final deletion. Product image
rows and deletion intents are fixture-seeded: this does **not** certify mounted Product
commands/receipt transactions, PostgreSQL Media, generic remote deployment, or crashes.
The fixture pins migration 000037 by name, rather than a moving migration head.

Freshly executed evidence for this continuation:

- Seven workflow source/mutation suites: **137 PASS**, including **22** retention cases;
  all seven corresponding standalone source guards pass.
- Additional gRPC authentication source/mutation suite: **8 PASS**.
- Catalog-schema verifier was found to pin pre-refactor category-only preflight,
  out-of-transaction projection and old transaction helper signatures. It now locks
  transaction-aware validation/projection/event/commit markers instead; the fixture
  suite passes, including four new removal mutations for these replacements.
- Four changed Rust sources formatted with WASM rustfmt, with an idempotence check;
  this is not typechecking. Changed JSON/TOML parse and `git diff --check` pass.

There are now **24 added Rust tests, all unrun** (previous 20 plus two queue race tests,
actual native Media owner and live loopback gRPC owner tests). The earlier 86 Admin,
four storefront, scoped TypeScript and 28-file parser results remain historical, not
reruns in this continuation. No cargo/rustc is available. ADR stays **Accepted / In progress**;
all runtime/feature-matrix/unknown-commit/restart/mounted-client gates above remain open.


### 2026-10-18 implementation continuation (maintainer executes tests later)

Source review identified and fixed further production wiring gaps:

- Delivery now calls `ProductCatalogCommandRuntime::process_media_releases`, using the
  exact embedded CatalogService snapshot for its database and Media capability. The
  worker no longer combines a host database and independently resolved shared port.
  External Product profiles fail closed rather than draining a local queue.
- Release delivery/lifecycle compiles with `mod-product` alone. It requires a composed
  neutral Media port at runtime, not an embedded `mod-media` feature. Missing capability
  skips delivery and preserves obligations; supplying a provider later is observed on
  subsequent ticks. This does not provision or certify a new remote deployment.
- Worker lifecycle is atomically reserved before spawn, replacing check-then-insert.
  No automatic owner handover, queue migration or fleet fence is claimed.
- Native maintenance permission denial now returns Forbidden before storage IO; HTTP
  preserves it as 403. OpenAPI advertises validation/authentication/authorization failures.
- Audit resolves each bounded Media hold page with one tenant-scoped Product query,
  removing its N+1 local database roundtrips without inventing a distributed snapshot.
- Restored the missing `PortError` import in the command runtime during source review.

Existing native/loopback scenario sources now exercise runtime-owned delivery and native
maintenance permission classification. Six new source-mutation cases lock the wiring
changes. No new Rust test functions were added: the 24 previously added tests remain
unrun. Per maintainer request, no test suite was executed in this continuation. Only
source guards, syntax/formatting and whitespace checks were run; the earlier 146 Node
passes are historical and do not certify these edits. Rust compilation, runtime/feature
matrix, database, restart, replica and mounted-client gates remain with the maintainer.


### 2026-10-18 Shipping Profile receipt-admission continuation

The confirmed transport-preflight gap now has a source implementation. Product consumes
its own read-only ProductShippingProfilePort; Commerce supplies an owner adapter. Four
CatalogService create/update Product/Variant methods validate explicit assignments before
transaction start, including all initial Variant overrides on Product create. Command
receipt admission/replay precedes service execution, so completed success/failure replay
never reads mutable profile state. Trusted internal service writes validate as well.

Normalization/deduplication, bounded selection and whole-read deadlines prevent unbounded
cross-owner IO. Missing provider, outage, authorization and uncertain outcomes remain
retryable; only nonretryable authoritative NotFound is finalized as Product validation.
Absent/clear/inherited fields require no provider lookup. No distributed active-at-commit
or permanent-reference guarantee is claimed. Existing nullable Variant patch semantics
are unchanged. See the updated write-boundary contract.

Composition is installed inside the selected embedded owner, preserves explicit providers
and external Product owners, and supports neutral provider injection without mod-commerce.
Commerce's embedded HTTP/GraphQL fallback runtimes also compose their native provider.
REST/GraphQL no longer perform mutable profile preflights; obsolete Product-specific
transport validators/mappers were removed. Permissions/module/tenant/actor checks stay.

Two added owner receipt test sources cover normalized initial Variant overrides,
completed replay during outage, fail-closed missing capability and finalized rejection
replay after provider recovery. Existing round-trip fixture now explicitly supplies its
policy fake. Total added tests in the broader slice: 26, all unrun. No test suites were
run in this continuation, as requested; no cargo/rustc is present.

Executed source checks: shipping admission, catalog receipts, catalog schema, Media
retention, atomic save, attribute validation and GraphQL lifecycle cutover guards pass.
Fifteen changed Rust sources pass WASM formatter/idempotence checks; this is not Rust
typechecking. Changed JSON/relative documentation links and git diff --check pass.
Historical diagnostic source guards for
GraphQL order helpers, HTTP route error-context and HTTP diagnostic safety still fail
on older owner/error/route assumptions outside this shipping cutover; they are not
reported as passing. Their shipping-preflight expectations were updated, not their
unrelated historical contracts. Old shipping-specific mapper verifier CLI entrypoints
now delegate to the replacement receipt-aware shipping guard; historical mutation
fixtures for those retired preflight contracts require maintainer migration.

Still required: compilation/feature matrices; all four real command/provider paths;
SQLite/PostgreSQL transaction/receipt execution; authenticated mounted transports;
provider outage/lease reclaim, unknown commit, restart and concurrent profile lifecycle
changes. No runtime certification or module-readiness advancement is asserted.


### 2026-10-18 diagnostic continuation superseding the three guard failures

The three historical HTTP diagnostic/route and GraphQL helper source guards reported
above now pass against current owner ports. This supersedes their failure status only,
not the unrun build/runtime gates. Review fixed real Shipping-option message/log detail
leaks, preserved Order Forbidden, and added authenticated Product/Order scope checks
before IO plus trusted request correlation propagation. See the Commerce plan and
current Product diagnostic contract for scope. Four new Rust unit scenario sources
remain unrun; no test suite was executed. Source checks/formatting are not runtime proof.


## Nullable Variant patches (2026-10-18)

The [nullable Variant update contract](./variant-nullable-patches.md) supersedes earlier notes
that Variant nullable updates still conflate missing and null. Product now owns Keep/Clear/Set
for all five nullable fields, with Set-only Shipping admission, GraphQL exact-decimal mapping,
Next command-shaped callback/action forwarding, separate Leptos create/update drafts and
complete-draft retry intent. The unused foundation update DTO is removed. Source verification
and WASM formatting are not compilation or runtime proof. Ten new Rust regression scenario
sources are unrun, existing editor validation sources are migrated, and no test suite was run.
Mounted transports, database/concurrency/rollback and actual UI gates remain open; this does not
close durable child-command retry or whole-screen Variant atomicity gaps or advance readiness.


## Flex/error/receipt boundary (2026-10-18)

Flex conversion is exhaustive and payload-safe; fixed Product entity registration rejection is
internal rather than client validation. Storage class is preserved. Catalog command and receipt
replay-decode diagnostics no longer render raw owner/serde errors. The shared catalog/schema/
Translation failure finalizer persists only non-retryable Validation/NotFound/Conflict; an invariant
or permission failure is not terminal merely because `retryable=false`. See the
[current diagnostic contract and open runtime gates](./product-public-error-diagnostic-safety.md#flex-and-receipt-failure-continuation-2026-10-18).
Six new Rust scenario sources are unrun; no test suite was executed. Broader Core/Taxonomy
classification, mounted runtime and lease-reclaim execution remain open. Readiness is unchanged.


## Taxonomy and Core error continuation (2026-10-18)

The [current error boundary](./product-public-error-diagnostic-safety.md#taxonomy-and-nested-core-continuation-2026-10-18)
now distinguishes Taxonomy write refusals from read-projection corruption and centrally classifies
nested Core failures across Product read/write/Translation and shared public/HTTP adapters.
Generic conflicts have an owner Conflict variant rather than Validation text. GraphQL definitive
outcomes require a known domain code and actual domain kind; the shared receipt allowlist and
existing stored receipts remain unchanged. The three category source guards now track current
same-ID/all-backend owner composition rather than retired donor/binding/closure compatibility.
Seven new Rust scenarios and strengthened existing hierarchy scenarios are unrun. Sixteen standalone
source guards and WASM formatting/idempotence of 27 Rust files pass; these do not prove compilation,
mounted behavior, rollback/replay/reclaim or privacy at runtime. No test suites or CI were run.
