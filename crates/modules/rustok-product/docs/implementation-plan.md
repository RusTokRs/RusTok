# Implementation Plan for `rustok-product`

> Canonical large-reference hardening is tracked in
> [reference-hardening-current.md](./reference-hardening-current.md). That cursor is authoritative
> for cross-module parity with the source-certified Blog v1 reference; this long-form plan remains
> authoritative for Product-specific historical/runtime work.

## Current state

### Accepted variant-axis target

[Unified variant axis architecture](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md)
is the accepted target for Product variant identity. Implementation is **In progress**. Current
writers already use the unified model; the ADR's historical legacy inventory is not evidence
that similarly named current dictionaries, journals or installed rows may be deleted.

### Downstream owner continuation (2026-10-30; step 8 in progress)

A bounded source slice now shares checked UUID-based checkout stock demand and exact
Product/SKU/fulfillment/context correlation between direct and staged Commerce. Channel
completes saved selectors without page/default fallback; Inventory scopes only new identity
allocations while preserving exact receipt replay/release. No SKU or stored owner link is
retired and no historical migration is changed. See the
[Commerce plan](../../rustok-commerce/docs/implementation-plan.md#product-downstream-continuation-2026-10-30)
and [ADR contract](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10e-downstream-sku-contracts-2026-10-30).

Regression sources are unrun, and source checks are not Rust/database/mounted evidence.
Selected Product identity is now carried into Pricing in source: optional lean capability,
mandatory tenant-correlated responses before price IO, mandatory full-runtime factories and
actual Commerce/native/staged repricing roots. External capability failure refuses rather than
rebuild local Product; external SKU authoring is explicitly unsupported until its write protocol
exists. See the [Pricing plan](../../rustok-pricing/docs/implementation-plan.md). Real remote
execution, Cart quantity/tier mutations and the complete Order/Fulfillment/stock/outbox/Index/
selected-runtime chain remain open; step 8 is not complete.
The 2026-10-31 [Inventory adoption source handoff](../../rustok-commerce/docs/implementation-plan.md#inventory-owned-reservation-adoption-2026-10-31)
now keeps reservation binding mutation with Inventory and preserves UUID-based downstream lineage.
This removes one concrete direct-write leak; it does not close shipment/return/outbox/Index or runtime
verification, and adds no schema/history rewrite or new receipt.
The bounded [return-restock identity continuation](../../rustok-commerce/docs/implementation-plan.md#return-restock-identity-source-evidence)
now correlates saved Order SKU/quantity through the selected read port and scopes Inventory's
allocation/receipt replay. It adds no receipt/schema/history rewrite; 33 Rust/23 MJS definitions
remain unrun. The archived receiving-location correction preserves read-only replay while
refusing new stock at closed locations under the existing storage guard; no automatic
reactivation, relocation or key rotation occurs. Shipment/cancel/reopen tenant/allocation/history, lifecycle overlap and complete
outbox/Index/runtime convergence still remain open; this is not full return/step-8 completion.
Native physical stock now has I-owned canonical root outbox invalidations in the local
mutation transaction; [owner plan/evidence](../../rustok-inventory/docs/implementation-plan.md#native-stock-outbox-continuation-2026-11-01-source-only).
Preparation/exact replay/no movement do not publish; equal-availability shipment does.
The v1 quantities remain descriptive global availability, not stock movement deltas. Added
16 Rust/10 Node definitions are all unrun. Existing Search refetch wiring is traced, not
runtime delivery or generic Index/normal reservation convergence. No new schema/capability,
unpaid-return policy or inspection top-up is inferred; full step 8 remains open.

Step 9 remains consumer- and persisted-state-led, not a name-based deletion pass.

### Variant-axis continuation (2026-10-29)

This is ordinary feature work against base `16cb22206ca19de8218f47a1056d946105c7b09f`,
not execution or advancement of the reference/audit cursor.

**Saved source changes (not runtime-certified):**

- Schema/category binding inputs accept optional `variant_axis_policy` and
  `default_variant_axis` patches. Omission retains an existing binding; a new binding defaults
  to `forbidden`/`false`. The merged pair is validated, not only the supplied fields. Allowed
  and required axes require an unarchived `select` definition with `variant`/`both` scope.
- Canonical effective forms retain inheritance, reusable-schema, local overlay and independent
  clone-snapshot precedence. Unknown policy, invalid default hints and missing/malformed clone
  snapshots fail closed rather than masquerading as empty/forbidden forms. Product EAV inputs
  do not admit variant-only definitions.
- Policy-affecting binds, category mode changes and clone creation share a PostgreSQL tenant
  advisory transaction fence with Product/category/axis/EAV writes. Schema writers take the
  exclusive fence before before/after reads; Product writers take the shared fence before
  aggregate locks. SQLite reserves its writer before reads; unsupported engines fail closed.
  Changed effective categories (including inherited descendants) scan Products in pages of
  128 and refuse invalidation with at most eight UUID-only examples plus `has_more`.
  Refusal does not modify SKU identities, assignments, stock, prices or outbox state.
- Initial axes are installed inside the existing Product create transaction before selected
  SKU tuples are inserted. There is no intermediate default SKU for an axis-bearing Product
  and no Cartesian generation. Existing canonical command receipts remain the write boundary.
- Next and the routed Leptos editor expose eligible axes, allowed values and explicitly selected
  sparse tuples. Required/default suggestions seed the first untouched category draft; later
  category/value/axis changes retain rows and block invalid saves instead of silently pruning
  or rebasing them. Draft bounds are 32 axes, 256 values per axis and 128 initial SKUs; the SKU
  bound is a client draft bound, not a new owner-wide import limit.
- Both admin clients send assignments for standalone SKU creation. The shared Leptos SKU
  section is now mounted in `ui/product_editor.rs`; its projection reload merges only confirmed
  variants/axes and does not overwrite dirty document fields. Existing-axis saves are guarded
  during an unsaved category change. The newly mounted section does not expose deletion before
  a stock-safe owner retirement contract exists.
- The Next browser journal stores exact sparse create snapshots under the existing scoped
  recovery identity. Persisted pre-axis scalar create attempts remain readable and replay their
  original shape; they are durable pending requests, not a second canonical write model.

- Existing-axis reconfiguration now uses the selected command provider for preview and the
  existing receipt-bound command for atomic CAS application. Both admin clients expose full
  target tuple correction/confirmation and immutable, scope/locale-bound recovery. Configuration
  row IDs, Variant UUIDs and removed-axis EAV are retained. Native axes use JSON input; no
  transport/provider fallback follows an unknown response. Detailed semantics, bounds and
  outstanding gates live in the [owner contract](./variant-axis-reconfiguration.md).

- Bounded local combination browsing/picking is implemented in both admin clients, reusing
  existing atomic Product create and standalone SKU create. Configured-axis subsets are the
  candidate source; filters never change them. The picker checks complete saved membership
  (up to 256), refuses spaces above 4096 and generates only a 12-row page. Explicit picks add
  one initial draft or replace only the saved-SKU form's tuple; no price/stock is inferred.
  Saved assignments are visible in the SKU lists. Blank initial price is absent, explicit zero
  remains a price and orphan compare-at refuses. See the [workflow and source-only gates](./variant-combinations.md).
- Error-review corrections preserve a picked Leptos manual tuple through control mounting,
  clear and metadata refresh. Retained unavailable choices stay visible; filter resets no longer
  write through a reverse control effect. New-Product save ignores unused scalar inventory when
  explicit rows supply the actual values. Next validates assignments only for creation and binds
  the intended writer, refusing a missing callback instead of falling through to create or closing
  without a write. The price-save VM harness now supplies busy/disabled/localization bindings.
  Added regressions are source only and were not executed.

- Standalone SKU creation now has immutable full-request recovery in **both** clients through
  the existing owner command/receipt. Leptos shares the axes journal/browser mechanism and
  selects native JSON or metadata-preserving GraphQL before IO. Next extends its Product
  journal/action/recovery-only route with a distinct Variant acknowledgement, not a fabricated
  Product revision. Exact body/key/original locale survive reload, unavailable Product GET,
  unknown outcomes and explicit terminal correction. Missing storage, late acknowledgement
  CAS and terminal-only rotation fail closed. Stable root fences cover axes and SKU creation;
  stale auth/bootstrap/owner responses cannot retarget Leptos recovery. Both reference entry
  paths reuse the keyed routed editor for selected Products, not their old GET-dependent form.
  The obsolete memory-only standalone creator is removed, not retained as a second path. Details and bounds are in the
  [same owner recovery contract](./variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery).
  Other Leptos CRUD outside the axes/SKU/definition journals remains memory-only/GraphQL-backed
  and is not certified durable.

- Attribute/option lifecycle now uses the selected schema ports and existing owner receipt
  ledger: presentation-only updates, reference-checked archive and same-ID non-cascading restore.
  Both admin clients persist full dictionary command/key/scope/original locale before IO, recover
  without dictionary GET and require explicit terminal Fresh. Shared `attribute_id`/`option_id`
  keys and retained missing targets prevent an implicit parent/first-option replacement. Defaults,
  immutable identity, EAV/SKU assignments and cross-owner links are not rewritten. Deferred stable
  code hints remain metadata, not applied assignments. Source tests include strict preparation,
  receipts/recovery, missing projections, SQL bounds and separate-connection reference creation
  versus archive; they were not run. The [bounded contract and unexecuted gates](../../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10c-attribute-and-option-definition-lifecycle)
  do not certify Rust, database or mounted execution. Storefront/downstream work remains open.

**Source/data inventory and retention decision:**

| Surface                                                                              | Current role / disposition                                                                                            |
| ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| `product_attributes`, `product_attribute_options` and option translations            | Canonical dictionary and localization; retain. An option is not an obsolete independent Product option model.         |
| Product/Variant typed EAV values and option joins                                    | Current value/assignment storage; retain.                                                                             |
| `product_variant_axes`, allowed-value configuration and `combination_identity`       | Current SKU tuple identity/configuration; retain stable Variant UUIDs.                                                |
| Catalog variant prices/inventory fields                                              | Active compatibility/read snapshots; Pricing and Inventory remain authoritative. Migrate consumers before retirement. |
| Product graph/relation snapshots, tombstones, translation/change journals            | Active rebuild/replay and translation consumers; retain until owner/runtime evidence proves otherwise.                |
| Shipping-profile bindings, channel graph relations and inventory reservation records | Active cross-owner links; no cleanup inferred from names.                                                             |
| Browser pending Product attempts and server receipts                                 | Durable retry identity; do not rewrite an unknown request or invalidate its receipt framing.                          |
| Historical migrations and installed tables/rows/FKs                                  | Deployment inventory unavailable; no historical migrations changed and no destructive cleanup authorized.             |

Source tracing is not deployment inventory. No database rows, installed migration history,
external callers, production FKs or deployment configuration were inspected in this environment.
No canonical entity/table was retired in this change.

**Verification evidence:** tests were deliberately **not executed**, at the user's request.
Focused source scenarios were added for policy omission/eligibility, effective precedence,
bounded impact/rollback, the second Product page, sparse tuples, assignment errors, exact browser
replay and old pending scalar attempts. An ignored PostgreSQL test in
`apps/server/tests/product_variant_axis_policy_postgres.rs` uses a disposable database, real
migrations/outbox and separate connections for create versus policy withdrawal. The test is
source only, not proof that its assertions passed. Its reconfiguration continuation also covers
axis/value position swaps, complete tuple swaps, stable EAV/Inventory links, receipt replay and
explicit reassignment after an axis-removal collision. Owner/core/Next and shared GraphQL source
scenarios cover bounded planning, metadata/tuple preservation, closed refusals and exact replay.
Additional unrun combination scenarios cover mixed-radix pages, the exact 4096 bound, local
filters, full membership, stale/click-time refusals, single-row append and absent-versus-zero
prices. No test modules or runtime fixtures were executed. The latest expanded host check still
has 57 diagnostics and zero direct Product/detail/action diagnostics; that is not a green build.
The review's repository i18n parity gate accepts the Product catalogs but fails on existing
Messenger/Workflow plural messages; the i18n contract gate reports a missing Blog migration.
The remediation gate passes diff invariants but cannot complete architecture metadata checks
because Cargo is absent, even with its clippy step skipped. These gates are not reported as green.

Strict TypeScript checking passed for `api/types.ts`, `money.ts`, `schema-policy.ts`,
`variant-draft.ts`, `axis-reconfiguration.ts`, `combinations.ts`, `write-attempt.ts`,
`write-journal.ts`, `variant-create.ts` and transitive `attribute-lifecycle.ts`.
Source-only native TypeScript module linking passed; no test modules were imported. Rust
formatter/parser, JavaScript syntax, Fluent generation/syntax and diff checks are scoped source
checks, not execution evidence. Expanded UI/host typing includes the actual Product detail
route/actions and has no direct Product diagnostics, but is **not passing**: transient tooling
lacks form/richtext/UI dependencies and the graph also reports errors outside Product. Apollo
and the pinned Fluent package were restored to the transient tooling; no full app dependency
install or build was performed. Rust/Cargo and PostgreSQL CLIs remain unavailable. The
remediation gate's diff-invariant phase passed, but its architecture phase cannot invoke
`cargo metadata`; `--skip-cargo` does not bypass that dependency. `npm run verify:adrs` also
fails on pre-existing missing/unsorted registry entries, an unrelated Product image-status
mismatch and missing headings in other ADRs; it reports no unified-axis ADR mismatch.

The standalone-recovery continuation also adds unrun Next request/SDK/action/CAS scenarios,
Rust exact-price/retained-assignment/metadata/native-mapping/old-axes-envelope scenarios, and a
PostgreSQL stored-response scenario after later stock mutation. Current scoped checks parse and
format 27 affected Rust sources without compiling macros; strict core TS includes
`variant-create.ts`. The source reachability guard is synchronized with the selected adapters
and shared durable journal, not the removed in-memory retry chain. No tests were executed and
no database, migration, storefront or audit cursor was advanced.

The dictionary lifecycle continuation's final scoped static checks parsed/idempotently formatted
63 selected Rust sources, formatted 11 TS/TSX/MJS sources, passed strict core TypeScript and the
two read-only Product source-reachability guards, and checked JavaScript syntax without importing
test modules. Four Fluent catalogs parse with 44 paired lifecycle keys; generated types are current.
Expanded Product/UI/host typing still reports 57 diagnostics outside Product and is not green.
No tests, Rust compilation/macro expansion, database or mounted-client checks were executed.

**Remaining work / release blockers:**

1. Compile affected Rust owner/transport/admin crates, run the focused tests and PostgreSQL
   integration test, validate GraphQL/native parity, and exercise both mounted clients. The
   current source must not be represented as validated merely because formatting parsed it.
2. Verify Taxonomy-owned reparenting participates in the schema validity fence; direct Product
   operations alone do not prove all external hierarchy writers are coordinated. Measure full
   tenant effective-form snapshot cost and lock contention at realistic catalog sizes.
3. Execute the new existing-axis planner/recovery scenarios and ignored PostgreSQL
   reconfiguration contract (position/tuple swaps, read-only preview, stable IDs/links,
   replay/stale refusal and axis-removal collisions). Measure maximum-bound transaction/lock
   costs and native/GraphQL host body limits. Source implementation is not a runtime sign-off.
4. Execute the new attribute/option lifecycle owner/impact/native/Next and ignored PostgreSQL
   contracts, including receipt replay, missing IDs, exact copy, bounds and reference-write races.
   Its update/archive/restore source implementation is not runtime certification. Stock-safe SKU
   retirement remains unfinished. Bounded sparse browsing/picking also needs mounted execution,
   including retained filters, duplicate membership and unpriced create/retry. Existing standalone
   deletion is not newly certified as stock-safe; no such action is exposed by the newly mounted
   Leptos section.
5. Ship the structured storefront axis/actual-SKU contract and Next/Leptos selectors,
   missing-versus-unavailable behavior, authoritative Pricing, links, accessibility and
   localization. Storefront packages have not been cut over in this continuation.
6. Verify Pricing/Inventory/Channel/Index/Cart/Order/Media/Fulfillment effects with real receipts,
   outbox/projection replay and stable Variant links; no end-to-end cross-owner completion claim.
7. Verify localized axes/SKU recovery controls and browser storage/History/scope/late-response
   paths in both mounted clients, including missing Product GET and every shipped transport
   profile. Axes and standalone SKU now have durable browser recovery in source. Other Leptos
   lifecycle writers remain memory-only and need separate immutable snapshot cutovers; Product
   create and remaining CRUD still delegate to the lifecycle GraphQL gateway. Native standalone
   creation does not certify a general native CRUD cutover. Finish remaining initial/schema
   control localization. Execute the new Next/Rust regressions and the PostgreSQL stored-response
   scenario; never infer successful execution from syntax/source-marker checks.
8. Obtain deployment data/caller/migration inventory before any destructive legacy retirement;
   preserve potentially applied history and document deterministic data treatment.

Maintainer checks (not run here): scoped `cargo check`/`cargo test` for `rustok-product`,
`rustok-product-admin` and affected Commerce/Pricing contracts; the Product Next package test
script; and `cargo test -p rustok-server --test product_variant_axis_policy_postgres -- --ignored`
with `RUSTOK_MIGRATION_SMOKE_ADMIN_URL` pointing to a disposable PostgreSQL admin environment.
The PostgreSQL test creates and drops its own uniquely named database, not the configured
application database. These commands still require the repository's normal feature/dependency
setup; this list is not successful execution evidence.

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
retain an explicit in-process compatibility fallback. The checkout consumer source cutover is complete
for these entrypoints. Checkout pricing receives the selected runtime, and checkout
planning obtains the catalog read port from it.

`rustok-product-transport` supplies a concrete tonic gRPC client/server adapter
for catalog projections, the storefront variant projection used by Cart, and the two SKU identity operations.
Protobuf owns RPC identity and framing
while JSON preserves Product-owned request/response DTOs and `PortContext`. The
client maps context deadlines to tonic timeouts and restores structured
`PortError` details. The server requires interceptor-provided
`TrustedProductCatalogAuthority`, verifies tenant/operation authority, and
replaces untrusted actor/claims/roles before invoking the owner port. The loopback
conformance harness is intended to cover product projection, variant-first projection,
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
same runtime. Both projection and SKU identity reads share that authenticated client.
Invalid remote configuration or connection failure aborts startup
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
remains an external deployment precondition. The provider startup command is
`cargo run -p rustok-product-catalog-service`; see the
[service README](../../rustok-product-catalog-service/README.md) for prerequisites.

The provider schema preflight is source-complete. After the PostgreSQL pool
connects and before `CatalogService`, outbox composition, or tonic listener
creation, the host performs canonical SeaORM read probes for `products`,
`product_variants`, and `sys_events`. A missing probed table, incompatible probed column, or
insufficient probe read permission aborts startup with a sanitized migration-precondition
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
preflight and listener readiness, invokes all six catalog and SKU identity RPCs through
the authenticated transport example, starts `rustok-server` with the external
Product profile, and waits for remote-provider and HTTP host readiness before
bounded shutdown. Commands and readiness markers are allowlisted; arbitrary shell
commands and non-loopback plaintext are rejected. Retained output contains source
hashes, toolchain versions, marker sets, output hashes, and byte counts only.
The capture contract guarantees that raw database URLs, bearer credentials, tenant IDs, product IDs, variant IDs, and process logs are not retained.
A successful capture records provider startup, authenticated owner RPCs and consumer
remote-profile initialization. Mounted business requests have separate evidence.

Adapter and production-wiring source are complete. Consumer-behavior,
authentication, provider-host, and schema-preflight source are complete, and the
requested unit, authentication, loopback, Commerce, and AI harnesses are
maintainer-attested as passed. These commands were not independently run by the implementation agent, and raw
logs were not retained.
At the time of the maintainer attestation, Product remained at `boundary_ready`.
The attestation recorded these limits: configured remote-profile execution evidence remain open,
provider-host execution evidence remains open, Schema-preflight execution evidence remains open,
and standalone PostgreSQL schema-preflight runtime evidence remains open.
Product remains `boundary_ready` pending the combined deployment capture. The local
execution results below cover Commerce checkout and the AI attributes task. The
sanitized packet for the configured separate-process profile still needs to be retained.

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
  This label covers the catalog-read track, not whole-module remote interchangeability;
  current-revision compilation and the capability gaps below remain open.
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

## FBA deployment scope review (2026-11-06)

This inventory tracks the current `boundary_ready` profile. The SKU identity
extension reuses the existing Product owner, provider host and authenticated connection.
The July 29 attestation is retained for its original projection and consumer harnesses.

| Boundary | Source truth and limitation |
|---|---|
| Provider selection | `apps/server/src/services/product_catalog_deployment.rs` installs only the external catalog-read runtime, before ordinary bootstrap. Invalid configuration/connection aborts; no selected-read failover is implemented. |
| Wire operations | `ReadProductProjection`, `ReadVariantProductProjection` and `ListPublishedProducts` cover base catalog projections. `ReadStorefrontVariantProductProjection` applies Product visibility rules when Cart resolves a SKU. `ReadVariantIdentities` and `ListProductVariantIdentities` cover `ProductVariantIdentityReadPort`. |
| Additional reads | The gRPC client inherits unavailable defaults for filtered lists, facets, storefront detail by handle or Product ID and other optional read methods. External runtime schema/HTTP/tag handles remain absent unless explicitly supplied. |
| Mounted consumers | Pricing reads SKU identity from the selected Product provider. The local Commerce HTTP checkout passed against both selected profiles. Native storefront list/facets/detail require additional methods. Some native admin list/facets/schema paths construct local services. |
| Commands | `commerce_provider_runtime.rs` separately creates the embedded Product command runtime when no command runtime is installed. The catalog-read selector does not select remote writes. |
| Transactions | Product, Pricing and Inventory lifecycle/bootstrap operations share a local SQL transaction. Their transaction-participant interfaces cannot cross RPC. |
| Packaging | Public contracts and implementation are in the existing owner crate, with unconditional implementation dependencies. A contracts-only feature is unimplemented and is not a prerequisite for this bounded FBA profile. |

The current selection is mixed placement of local commands and remote reads. If the
provider uses the same Product database it is process extraction with coordinated schema,
not an autonomous replica. If different data stores are configured, their synchronization
and authority must be explicitly designed; selecting gRPC creates no replication stream.
Whole-module extraction requires a separate complete inventory of commands, reads,
outgoing capabilities, events/jobs, migrations/lifecycle and public UI/API entrypoints.
Resolve transaction/co-deployment decisions before changing write placement.

### Current revision and evidence limits

The conformance fixture now supplies the required `ProductResponse.brands` field.
On 2026-11-07, the Product SKU identity implementation in `308e8e7207` passed 29 local
tests after the transport and host API documentation sections were restored:

```bash
cargo test --locked --offline -p rustok-product-transport -p rustok-product-catalog-service
```

The loopback harness exercises both SKU identity operations through the selected runtime
with bearer authentication and typed errors. Unit tests cover distinct operation grants
and nonretryable errors for an unsupported RPC. These tests use a test owner for loopback
and cover host configuration separately. The older maintainer attestation does not cover
the identity RPCs.

On 2026-11-07, the standalone provider from the same runtime revision passed a
separate local smoke check with PostgreSQL 16. The test applied platform migrations
to an isolated database and seeded an active Product and one SKU. The provider
passed schema preflight and the updated authenticated probe verified all five
RPCs, including the exact tenant, Product and variant identifiers in both SKU
reads. A probe with a different bearer token was rejected.

The storefront SKU projection added in `89427e6055` passed the same 29-test
transport and provider suite. Against the migrated PostgreSQL 16 fixture, the
standalone provider passed all six RPCs. The updated probe resolved the
published SKU and confirmed that the same storefront operation hides a draft
SKU. A request with the wrong bearer token was rejected.

### Mounted Commerce HTTP checkout (2026-11-07)

The API host was built with `cargo build --locked --offline -p rustok-server
--bin rustok-server --no-default-features --features mod-commerce`. It ran with
`RUSTOK_RUNTIME_HOST_MODE=api`, a module manifest matching the build, and one
isolated PostgreSQL 16 database. In the gRPC profile the HTTP host and Product
provider were separate loopback processes sharing that database. The same HTTP
scenario ran with the embedded Product profile and the manual payment provider.

- `POST /store/carts` created a guest cart and issued its access token.
- `POST /store/carts/{id}/line-items` refused the draft SKU with HTTP 404
  and added two units of the published SKU for USD 50.
- `POST /store/carts/{id}/complete` applied a price change from USD 25 to
  USD 31 per unit. The order was paid and payment captured for USD 62. Reusing
  the idempotency key returned the same order, payment and amount in both profiles.
- With Product stopped in the gRPC profile, a new checkout returned HTTP 503.
  The PostgreSQL order count was unchanged.

The first HTTP run exposed a PostgreSQL parameter binding error when Cart loaded
the tenant locale. Commit `8077c68a71` selects the correct parameter syntax for
PostgreSQL, and the successful runs used that fix. This evidence covers the
Commerce HTTP path for a digital SKU on shared storage.

### Mounted AI product attributes (2026-11-10)

The `mod-commerce,mod-ai` API host and standalone Product service were built from
`8aedf54fda` and ran as separate loopback processes. They used separate migrated
databases in an isolated PostgreSQL 16 cluster. The published test Product existed
only in the provider database. Inference used a deterministic local model fixture.

- An authenticated `runAiTaskJob` request to `/api/graphql` returned suggestions
  with `product_context.source=owner_port`. The model request contained the
  remote Product projection and the title submitted by the operator.
- A missing Product produced `not_found` with `retryable=false`. Stopping the
  provider produced `unavailable` with `retryable=true`. Both requests generated
  suggestions from the submitted prompt with catalog enrichment skipped.
- All three responses required operator review and reported `persistence=none`.
  The owner Product, translation, SKU, and attribute value rows matched their
  initial state. The consumer database still contained zero Product rows.

This is local execution evidence for the AI business request. The combined
deployment capture remains open.

## Open results

1. Run the source guard before separate-process execution:
   - `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs`;
   - `node scripts/verify/verify-product-catalog-separate-process-runtime-contract.test.mjs`.
     Then execute `node scripts/evidence/capture-product-catalog-separate-process-runtime.mjs`
     from a clean commit with migrated provider and consumer PostgreSQL databases,
     matching loopback bind/endpoint values, one bearer credential, and existing
     tenant/product/variant fixtures. Set `RUSTOK_RUNTIME_HOST_MODE=api` and use
     `RUSTOK_MODULES_MANIFEST` with the modules compiled into that consumer profile.
     Use a fresh consumer database or one with the matching active composition.
     Commit the generated sanitized evidence packet.
2. Retain a mounted AI product-generation request and the Commerce checkout
   evidence for that captured deployment. Promote above `boundary_ready` after
   the required results and their sanitized logs or CI artifacts are retained.
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
- [x] Verify mounted Commerce HTTP checkout locally with embedded and separate gRPC Product on PostgreSQL.
- [x] Verify mounted AI product attributes locally with separate gRPC Product and PostgreSQL databases.
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

## Storefront/Pricing continuation (2026-10-29)

Pricing foundation source is extended in its owner before the axis-aware buyer UI:
Product exposes bounded identity-only batch/page reads; Pricing owns normalized
single/batch quotes, one Pricing snapshot/time, explicit unpriced items and source
provenance. Native and GraphQL effective-price paths use that port; the public Pricing
atlas no longer fabricates SKU/sale/currency coverage. Product contextual unpriced
previews do not fall back to raw rows, and missing Pricing is shown separately.

**Still open:** real sparse-tuple axis selection in Next and Leptos storefronts,
canonical SKU and quantity/context URL state, complete Product route/context refusal
rather than sanitizer defaulting, live quantity/context re-quotation, cart eligibility
and context handoff, accessibility/localization review and downstream owner verification.
Existing flat/first-variant selection and no-context base inspection do not complete
this block. No tests or Rust/database/mounted validation were executed for this work.
See the [Pricing owner plan](../../rustok-pricing/docs/implementation-plan.md).

## Buyer interaction continuation (2026-10-30)

Affected-contract inventory before implementation:

- Product's existing `ProductResponse.variant_axes`, `VariantResponse.axis_values`
  and `ProductCatalogReadPort.read_storefront_variant_product_projection` own
  identity/labels/public visibility. Next and native storefront transport models
  currently drop axes; their selected panels currently inspect the first SKU.
- Pricing already owns strict route quantity/context validation, bounded quotes,
  UUID joins and explicit unpriced results. Product must consume this contract,
  not calculate an effective price from compatibility rows.
- Next's Product route reads Pricing selectors but drops them at the buyer-panel
  boundary. Native Product sanitizers currently discard invalid explicit values.
- Cart owns persisted cart context and row-locked insertion. Next Cart query
  `adjustedAmount` and quantity mutation drift from the live Commerce schema;
  both actual adapters must use the real schema before wiring the buyer action.
- Commerce already owns cross-owner storefront/cart/checkout orchestration.
  Native/GraphQL buyer additions must share an owner workflow, selected Product
  port, Pricing and guarded Cart port; no UI-private product-table reader.
- Existing Cart guest-token claims/cookies and customer access remain authoritative.
  No historical migration, installed tuple, journal, stock or link is retired.

Work in progress: sparse selector, strict URL refusal, live quote binding and exact
cart-context handoff for Next/Leptos. Tests remain explicitly unexecuted at the
maintainer's request; Rust/database/mounted verification is not available here.

### Buyer source implementation and handoff (2026-10-30)

The inventory above describes the pre-change consumers, not the resulting implementation.
The step-7 buyer source is now implemented on Next and Leptos:

- Actual persisted sparse tuples, canonical non-nil UUIDs and a single no-axis default SKU;
  partial/missing tuples never select a different SKU. Projection bounds are 256 SKUs, 32 axes
  and 256 values per axis; malformed inventory booleans/i32 quantities and tuple mirrors refuse.
- URL-owned SKU/axes, quantity, Pricing context and Cart reference; duplicate/array-valued query
  selectors and oversized queries refuse. Links preserve unrelated scalar state and invalid
  quantity text while disabling purchase. Snapshot route/input disagreement and invalid tenant
  metadata refuse before mounting.
- SKU/quantity/input-cycle and auth/tenant/Cart-scope quote fencing, including A→B→A; selection
  survives auth changes, but stale money and added-state do not authorize another purchase.
  Full Product/SKU/context/RFC3339-time correlation precedes effective-price presentation.
- Commerce-owned exact SKU add, guarded Cart access and server-side repricing. The command never
  carries displayed money. Arbitrary list-scoped previews remain ineligible for ordinary Cart
  writes; immutable Cart currency/region/channel/exact locale must match, or the buyer must
  inspect/reset/use its context and quote again.
- Scoped Cart reference restoration and pending-before-IO warning hints on both clients. A known
  original-scope create ID is retained before checking the current actor; inactive/foreign/
  failed reads never silently clear a reference. Private Cart projections are hidden across
  actor changes. No unknown write is replayed automatically.
- Canonical auth/tenant metadata, bilingual notices and shared presentational controls. Native
  guest mounting binds a missing public tenant using the existing auth tenant key, never a
  foreign saved tenant or a fabricated session. Native declarations are shared with SSR-only
  handlers; GraphQL remains independently selected, never a native fallback.

**Evidence:** scoped strict Next typechecks, standalone source guards, Fluent generation and
syntax/format checks only. New/updated Rust and Node regression sources are intentionally unrun.
Cargo/rustc are unavailable. Lock entries for already-resolved local/workspace dependencies were
synchronized without changing registry versions/checksums; Cargo/npm installation is not certified.

**Release gates still open:** Rust default/SSR/hydrate and WASM builds; mounted Next/Leptos state,
URL history, keyboard/focus/screen-reader and locale behavior; actual auth refresh, guest HttpOnly
cookie/Set-Cookie forwarding and tenant middleware; backend/SQL concurrency and Pricing freshness;
remote selected capabilities, cancellation and full downstream-owner verification (step 8).
`apps/storefront/scripts/build-product-buyer-client.mjs` and the opt-in asset feature still need a
real build. Local pending hints are not receipts, locks, exactly-once execution or cancellation
proof. Opaque-token scope rotation does not infer principal continuity or merge another actor's
Cart. Pricing write CAS/receipts, list eligibility/invalidation/outbox work remain separate.

Maintainer commands (not run here):

```sh
cd apps/next-frontend && npm run test:storefront-buyer
cargo test -p rustok-product-storefront --lib
cargo test -p rustok-cart-storefront --lib
cargo test -p rustok-pricing-storefront --lib
cargo check -p rustok-product-storefront --features ssr
cargo check -p rustok-product-storefront --features hydrate
```

### Latest static handoff evidence

- Focused strict TypeScript check: zero diagnostics for Product pages (including the actual RSC
  detail route), buyer SDK/core/panel, Cart provider/core, Pricing and shared controls/auth wiring.
  Cached pinned tooling/one React declaration path was used; this is not a full app build or lint.
- Scalar Rust syntax/format idempotence: 62 scoped files; no Rust typechecking or compilation.
- Prettier idempotence: 43 scoped files; Node syntax-only parsing: 14 MJS files, including unrun
  regression sources. GraphQL syntax-only parsing: 14 actual SDK documents; no schema/runtime proof.
- Fluent syntax/duplicate-ID checks: six catalogs. Host type generation was refreshed earlier.
- Seven unique Product/Cart boundary/error-safety source guards pass. `git diff --check` passes.
- Selected Cargo/npm manifest↔lock dependency sets match structurally; existing registry versions
  and checksums were preserved, but no Cargo/npm resolver or application installation was run.

No test body, Cargo build/check/test, database, browser, mounted, WASM, CI or workflow verification
was executed. Temporary repository tooling links are removed before handoff. All release gates
above remain unpromoted.

### Step 8 — anonymous reservation prerequisite (2026-11-01; source only)

The reservation-notification continuation first corrects Inventory's unsafe quantity release:
proved anonymous pool only, original location, complete claim coverage, exact-one transactional
reductions and explicit refusal of Cart/Order/line selectors. Selected checkout continues using
durable identity. Numeric availability remains signed/global; a key is not an anonymous receipt.

See the [Inventory plan](../../rustok-inventory/docs/implementation-plan.md#anonymous-quantity-reservation-release-2026-11-01-source-only).
The prerequisite's 27 Rust/10 Node definitions remain uncompiled/unrun. The subsequent
reservation-fact continuation below wires the quantity-free Inventory family/current context and
explicit typed delivery/consumers in source. Existing `InventoryUpdated` v1 cannot represent
negative backorders; Product-owned revision/refresh-ledger facts cannot be counterfeit Inventory
publishers. Generic Index and end-to-end Search/relay/lag/restart evidence remain open. The
prerequisite itself added no schema/digest/migration/receipt or convergence claim.

### Step 8 — reservation fact/delivery continuation (2026-11-01; source only)

Inventory's quantity-free sealed fact/current-context transactional publisher, acknowledged
existing Core/Memory delivery, Iggy-first/local-ack fanout, artifact intake and explicit Search
refetch are source-wired. Identity and quantity responses use bounded checked signed arithmetic,
not saturation. Replay/zero/adoption remain publication-free; C/host/Product is not a publisher.
Search product writes/rebuilds/deletes serialize before fresh persisted-state reads, but this is
not generic Index's Inventory-owned source/revision/consumer or an owner-read/sellability API.

[Inventory plan](../../rustok-inventory/docs/implementation-plan.md#reservation-fact-delivery-continuation-2026-11-01-source-only)
and [source packet](../../rustok-inventory/contracts/evidence/reservation-events-source.json):
**34 additional Rust/10 Node definitions uncompiled/unrun**. Canonical Rust digest generation
(Cargo/rustc absent), actual SQL/relay/Iggy/cursor/restart/lag/concurrency, client/remote/deployment
and generic Index gates remain open. No Product revision/ledger refresh is fabricated.
**Step 8 is incomplete; source wiring is not runtime/readiness certification.**

### Step 8 — catalog deletion prerequisite (2026-11-01; source only)

Before an Inventory Index source can retain meaningful tombstones, canonical stock/history
must not be erased by Product deletion. Both real delete callers now use Inventory's bounded
concrete-transaction/current-tenant guard, with exact Product graph effects and existing
publication/default-SKU rules. Any reservation/shipment/return reference blocks, even zero,
deleted or terminal history; each stock counter must be zero. Missing item identity is not
absence. Root deletion refuses an empty corrupt graph or more than 256 children before cleanup.

The [owner collaboration contract](product-lifecycle-collaboration.md#catalog-deletion-retention-2026-11-01-source-only),
[Inventory plan](../../rustok-inventory/docs/implementation-plan.md#catalog-deletion-safety-2026-11-01-source-only)
and [source packet](../../rustok-inventory/contracts/evidence/catalog-deletion-source.json)
record the finite error policy, original locations/current locks, additive I026 indexes and
31 Rust/10 Node definitions, all uncompiled/unrun. HTTP/GraphQL presentation is not terminal
receipt authority: known nonretryable Validation/Conflict follows actual rollback/finalization,
whereas invariant/storage uncertainty remains unknown. Retry metadata alone cannot prove outcome.

Existing Next recovery requires real transport/browser evidence. Leptos remaining Product CRUD
continues through its generic GraphQL lifecycle gateway/redaction with non-durable retained
CRUD keys; actionable full-client recovery parity and a general retirement/disposal workflow
remain open. No destructive SKU UI was added. Inventory-owned revision, baseline, retained
tombstone, all-writer publication/source/bridge/worker and Product step 8 remain incomplete.

## Inventory quantity prerequisite (2026-11-01; source only)

The next Inventory Index trace exposed unsafe native set/adjust/bootstrap writes. Their shared
bounded quantity plan and the real Product caller are now source-wired before starting revisions.
See [collaboration](product-lifecycle-collaboration.md#quantity-writer-consolidation-2026-11-01-source-only)
and [source packet](../../rustok-inventory/contracts/evidence/quantity-writes-source.json).
Checked arithmetic/current identity-location proof/exact effects/canonical publication and typed
receipt/error preservation have 33 Rust/10 Node new focused definitions, **none executed**.
This is not a completed Inventory baseline/revision/tombstone/Index or step-8 gate; native stock
replay and full Next/Leptos actionable recovery remain open, with client code unchanged.

## Inventory source-history owner continuation (2026-11-01)

The fresh bootstrap now takes the concrete caller transaction; fresh creation, native/Product
quantity and history-preserving deletion share Inventory-owned source-history updates once an
explicit baseline is active. Missing source state is a nonterminal infrastructure/provenance
failure, never a fabricated deletion or terminal client validation. Building baseline fences
actual effects; replay/no-op/adoption/preparation do not advance. See the
[owner plan](../../rustok-inventory/docs/implementation-plan.md#inventory-source-history-continuation-2026-11-01-uncompiled).
18 Rust/8 Node definitions remain unrun. Canonical publication was pending at this handoff; the
continuation below adds that source slice. Selected Distribution bridge/current-authority generic
source, stale fence, persist-before-ACK and client recovery remain pending.

## Inventory canonical source delivery (2026-11-02; uncompiled)

Both fresh Bootstrap and both guarded deletion callers now forward current actor, not saved
metadata. Inventory changed/final-activation facts reuse retained change/baseline UUIDs on the
actual caller transaction; no Product revision proxy or second publisher. Required module
handling refuses unmatched broker-only acknowledgement while preserving Product remote-only
routes. [Owner plan](../../rustok-inventory/docs/implementation-plan.md#source-canonical-delivery-continuation-2026-11-02-uncompiled)
and [source evidence](../../rustok-inventory/contracts/evidence/source-delivery-source.json)
record 26 new Rust/10 Node definitions, all unrun. Selected authoritative Distribution source,
durable initial replay, exact min-version/stale fence/persist-before-ACK Index and full operator
client recovery remain pending. No live baseline before real handlers and operator recovery;
Cargo/digest/backend/runtime/Index/Product step-8 readiness stays unverified.

## Selected Inventory Index consumer (2026-11-02; source only)

Distribution now authors the actual native Inventory schema/source/sealed handler bridge and
Index owns durable min-version/stale-fenced persistence, original-ID initial enqueue and bounded
ModuleWork replay. Current authenticated service authority is independent of Product/event/
baseline actors; no Product revision or refresh proxy is introduced. Existing Product and remote
consumers remain selected separately. [Actual contract](../../rustok-distribution/docs/inventory-index.md)
and [source packet](../../rustok-inventory/contracts/evidence/source-index-consumer-source.json).

- [ ] Integrate failed-job retry and original-job operator recovery, then cross-client recovery.
- [ ] Compile/run focused definitions; validate current credentials/policy, SQL/concurrency/broker/
      restart/deployment and older-baseline cutover. No historical facts or checkpoint reset inferred.
- [ ] Admit full downstream Product step-8 evidence. Source/worker registration and initial pending
      enqueue are authored only; no test/build/runtime/readiness evidence is claimed here.

## Historical Audit Remediation Contracts

The following remediations from the product module audit are codified in code and verified by automated guards:

- `PROD-VALID-001`: Runtime enforcement of attribute-level base rules and schema/category validation overrides across product and variant EAV writes. Verified by `scripts/verify/verify-product-attribute-validation-rules.mjs`.
- `PROD-EVENT-001`: Product lifecycle events emit typed domain events on mutation, including `ProductUnpublished` and `ProductArchived`. Verified by `scripts/verify/verify-product-lifecycle-events.mjs`.
- `PROD-IDEM-001`: Product relations and bundles operations preserve transaction boundaries and enforce idempotency. Verified by `scripts/verify/verify-product-relations-bundles-idempotency.mjs`.
- `PROD-PORT-001`: Cross-module product relations expose typed public ports without leaking private persistence entities. Verified by `scripts/verify/verify-product-relations-bundles-idempotency.mjs`.

### Product Schema Write Receipt Coverage
- all eleven mounted Product schema writes
- exact completed projection
- empty-target detached clear
- source-complete durable owner replay


