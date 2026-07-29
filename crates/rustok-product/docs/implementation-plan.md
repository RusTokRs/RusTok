# Implementation Plan for `rustok-product`

## Current state

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
and performs no persistence. These harnesses have not been executed by the
implementation agent.

Adapter and production-wiring source are complete. Consumer-behavior,
authentication, provider-host, and schema-preflight source are complete, but the
service-host unit, PostgreSQL schema preflight, loopback conformance,
authenticated transport, and remote consumer harnesses have not been run by the
implementation agent, so Product remains `boundary_ready` rather than
`transport_verified`. Configured remote-profile execution evidence remain open.
Provider-host execution evidence remains open. Schema-preflight execution evidence
remains open.

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
search-option discovery, detached-value marker contract, and no-compile schema
guardrail are source-locked. The complete storefront/admin catalog-controls
contract carries snake_case `search`, `category_id`, `sort_by`,
`sort_direction`, and `attribute_filters` through typed UI state, native and
GraphQL adapters, Product-owned request models, and shared server-side
execution. Storefront and admin accept at most eight semicolon-separated
`code=value` attribute predicates. Product resolves each code against an active,
product-scoped, filterable definition and executes exact typed EAV equality for
localized/plain text, integer, decimal, boolean, date, datetime, select, and
multiselect storage while excluding detached values. JSON attributes are
explicitly rejected because this contract does not claim unindexed JSON
comparison semantics. Recheck on 2026-07-29.

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
  provider host, startup schema preflight, and Commerce/AI remote behavior
  harnesses are source-complete. Provider-host, schema-preflight, and authenticated
  end-to-end execution evidence remain open.
- Structural shape: `core_transport_ui`
- Evidence: `crates/rustok-product/contracts/product-fba-registry.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-contract-smoke.json`,
  `crates/rustok-product/contracts/evidence/product-runtime-fallback-smoke.json`,
  `crates/rustok-product-transport/tests/port_conformance.rs`,
  `crates/rustok-product-catalog-service/src/main.rs`,
  `crates/rustok-commerce/tests/product_remote_consumer_behavior.rs`,
  `crates/rustok-ai/src/direct_product_attributes.rs`,
  `scripts/verify/verify-product-runtime-fallback-smoke.mjs`,
  `scripts/verify/verify-product-catalog-read-runtime-composition.mjs`,
  `scripts/verify/verify-product-native-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-http-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs`,
  `scripts/verify/verify-product-catalog-grpc-transport.mjs`,
  `scripts/verify/verify-product-catalog-grpc-deployment.mjs`,
  `scripts/verify/verify-product-catalog-grpc-authentication.mjs`,
  `scripts/verify/verify-product-catalog-grpc-service-host.mjs`,
  `scripts/verify/verify-product-remote-consumer-behavior.mjs`,
  `scripts/verify/verify-product-admin-boundary.mjs`,
  `scripts/verify/verify-product-admin-category-sort.mjs`,
  `scripts/verify/verify-product-storefront-boundary.mjs`,
  `scripts/verify/verify-product-storefront-category-sort.mjs`,
  `scripts/verify/verify-product-catalog-attribute-filters.mjs`,
  `scripts/verify/verify-product-catalog-controls-plan-sync.mjs`, and
  `scripts/verify/verify-ai-product-fba.mjs` for the AI consumer contract.

## Open results

1. Execute and retain the external runtime evidence:
   - `cargo test -p rustok-product-catalog-service`;
   - `cargo test -p rustok-product-transport --lib`;
   - `cargo test -p rustok-product-transport --test port_conformance`;
   - `cargo test -p rustok-commerce --test product_remote_consumer_behavior`;
   - `cargo test -p rustok-ai --features server --lib remote_product_`.
   Retain the generated `Cargo.lock` package entry with the first successful Cargo
   execution. Run `cargo run -p rustok-product-catalog-service` against the
   migrated PostgreSQL schema and retain the successful `products`,
   `product_variants`, and `sys_events` preflight evidence. Then start the server
   with the matching authenticated gRPC deployment variables and retain end-to-end
   Commerce and AI evidence through the selected runtime. Promote above
   `boundary_ready` only with those retained results.
2. Keep Product richtext adoption explicitly deferred until the owner approves
   a typed storage/API/index migration. `product_translations.description` and
   catalog attributes currently named `richtext` are scalar text, so replacing
   their textarea alone would create a false contract. When approved, use the
   shared [Richtext plan](../../../docs/modules/rich-text-implementation-plan.md),
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
- [ ] Execute the Product catalog service-host, schema preflight, authentication, and loopback conformance harnesses.
- [ ] Execute the Commerce and AI remote consumer behavior harnesses.
- [ ] Retain end-to-end Commerce and AI behavior through a separately configured Product service.
- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.
- [x] Connect storefront title search through typed UI state, native/GraphQL transports, and Product-owned server-side filtering.
- [x] Connect storefront category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect admin search/status/category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.
- [x] Connect typed attribute_filters through storefront/admin UI state, native/GraphQL transports, filterable-definition validation, and Product-owned typed EAV execution.
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
