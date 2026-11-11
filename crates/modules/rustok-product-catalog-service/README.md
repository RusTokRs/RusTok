# RusTok Product catalog service

> This is the current Product standalone deployment package. The [FBA conversion target](../../../docs/backend/module-backend-implementation.md#remote-fba-module-layout) places its binary at `src/bin/service.rs` in the Product owner package.

For full public interface and operational contract definitions, see [CRATE_API.md](CRATE_API.md).

`rustok-product-catalog-service` exposes Product catalog read and SKU identity ports and can
optionally expose `ProductCatalogCommandPort`.

The binary composes the canonical Product owner service and the replaceable tonic adapter:

```text
PostgreSQL
   │ schema preflight
   ▼
CatalogService
   │ ProductCatalogReadPort
   ▼
ProductCatalogGrpcService
   │ ProductCatalogGrpcBearerInterceptor
   ▼
authenticated tonic gRPC
```

Product owns the DTOs, catalog policy and persistence. The standalone host composes
`CatalogService` with an `OutboxTransport`-backed `TransactionalEventBus`. Platform migrations
and outbox delivery are separate deployment responsibilities.

## Service Capabilities

This host exposes:
- `ProductCatalogReadService`: `ReadProductProjection`, `ReadVariantProductProjection`, `ListPublishedProducts`, `ReadVariantIdentities`, `ListProductVariantIdentities`, `ReadStorefrontVariantProductProjection`, `ListFilteredPublishedProducts`, `LoadStorefrontCatalogFacets`, and `ReadStorefrontProductProjection`.
- `ProductCatalogCommandService` (opt-in): 15 catalog operations (`CreateProduct`, `UpdateProduct`, `DeleteProduct`, `PublishProduct`, `UnpublishProduct`, `ArchiveProduct`, `CreateVariant`, `UpdateVariant`, `DeleteVariant`, `SetVariantAxes`, `PreviewVariantAxes`, `AddProductImage`, `UpdateProductImage`, `DeleteProductImage`, `ReorderProductImages`).

The standalone provider passed a local PostgreSQL smoke check for the six original read RPCs,
including the storefront SKU projection for a published product and a draft.
The local Commerce HTTP checkout passed with embedded and separate gRPC Product.
The mounted AI attributes task passed locally with a separate provider and separate databases.
See the [Product plan](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
The configured owner database and any local command database must have an explicit
authority/data-placement contract; this binary provides no replication mechanism.

## Required configuration

| Variable | Purpose |
| --- | --- |
| `RUSTOK_PRODUCT_CATALOG_DATABASE_URL` | PostgreSQL URL. Falls back to `RUSTOK_DATABASE_URL`, then `DATABASE_URL`. |
| `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN` | Credential for catalog and SKU identity reads. Never place it in source control or endpoint URLs. |
| `RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR` | Provider-owned identity assigned to authenticated reads, for example `rustok-server`. |
| `RUSTOK_PRODUCT_CATALOG_COMMAND_GRPC_BEARER_TOKEN` | Optional, distinct credential for commands. Share it only with a host authorized to write Product data. |
| `RUSTOK_PRODUCT_CATALOG_COMMAND_ACTOR_ID` | Non-nil service actor UUID used for commands. Required together with the command credential. |

The command service is registered only when both command variables are set. The consumer
must authorize the user request before delegating a command. The provider trusts the
authenticated host's tenant metadata and attributes the write to the configured service UUID.
Caller-provided actor, claims and roles are replaced by provider-owned authority.

The database schema must already be migrated by the platform migration workflow. This service does not run migrations at startup.

## Known Limitations / Pending Implementation

The standalone host does not wire the embedded Pricing, Shipping Profile or Media providers.
For example, authoring variant prices requires the Pricing binding. Inventory lifecycle and
reservation paths still depend on Product identity in the same database transaction. Before
moving writes, define the owner database, co-location rules and outbox relay, and prove
persisted write parity, retry behavior and recovery against PostgreSQL. The retained combined
read capture is a separate gate.

## Schema preflight

After the PostgreSQL pool connects and before tonic starts listening, the host performs read probes through the canonical owner entities for:

- `products`;
- `product_variants`;
- `sys_events`.

These bounded probes verify the listed Product and outbox entity reads for the configured database principal; they do not exercise every catalog join, dependency or RPC business path. A missing table, incompatible schema, or insufficient read permission aborts startup with a sanitized migration-precondition error. The host does not create, alter, or repair schema and does not silently continue with partial readiness.

## Listener and TLS

The default bind is `127.0.0.1:7443` and is configured with `RUSTOK_PRODUCT_CATALOG_SERVICE_BIND`.

Production or non-loopback deployment requires both:

- `RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_CERT_PATH`
- `RUSTOK_PRODUCT_CATALOG_SERVICE_TLS_KEY_PATH`

Plaintext is accepted only when all of the following are true:

- `RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK=true`;
- no TLS certificate/key is configured;
- the bind address is loopback.

The consumer must separately opt into loopback HTTP through `RUSTOK_PRODUCT_CATALOG_GRPC_ALLOW_INSECURE_LOOPBACK=true`. Neither side silently weakens transport security.

## Database pool

- `RUSTOK_PRODUCT_CATALOG_DATABASE_CONNECT_TIMEOUT_MS` defaults to `5000` and is bounded to `1..=30000`.
- `RUSTOK_PRODUCT_CATALOG_DATABASE_MAX_CONNECTIONS` defaults to `20` and is bounded to `1..=200`.
- SQL statement logging is disabled by the service host.
- Logs expose only a sanitized database target, never credentials or the full URL.

## Telemetry and shutdown

The host uses `rustok-telemetry` with service name `rustok-product-catalog-service`.

- `RUSTOK_LOG_FORMAT=json` selects JSON logs; otherwise pretty logs are used.
- `OTEL_ENABLED=true` enables the platform OpenTelemetry configuration.
- `RUSTOK_METRICS=true` initializes the platform metrics registry.
- Ctrl-C and Unix `SIGTERM` trigger tonic graceful shutdown and OpenTelemetry shutdown.

## Local source-level invocation

```bash
RUSTOK_PRODUCT_CATALOG_DATABASE_URL='postgres://user:password@127.0.0.1/rustok' \
RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN='replace-through-secret-management' \
RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR='rustok-server' \
RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK=true \
cargo run -p rustok-product-catalog-service
```

For the source-level invocation above, the implementation agent does not claim this command was executed.

## Retained separate-process evidence

The locked local evidence profile is defined by `crates/modules/rustok-product/contracts/evidence/product-catalog-separate-process-runtime-contract.json`. Its runner starts this provider, waits for the schema-preflight and listener markers, invokes all six catalog and SKU identity RPCs through the authenticated `rustok-product-transport` probe, then starts `rustok-server` with `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` and waits for remote-provider initialization.

The profile is intentionally restricted to an explicitly enabled loopback HTTP endpoint. The runner rejects TLS overrides in this local packet rather than silently weakening a non-loopback deployment. It requires already migrated provider and consumer PostgreSQL databases plus operator-supplied tenant/product/variant fixtures. Those values and the bearer credential are never persisted; retained evidence contains output hashes and byte counts rather than raw process logs.

The runner enables the provider and consumer readiness log targets at `info` so
an inherited `RUST_LOG=warn` cannot hide the required markers.

```bash
node scripts/verify/verify-product-catalog-separate-process-runtime-contract.mjs
node scripts/verify/verify-product-catalog-separate-process-runtime-contract.test.mjs
node scripts/evidence/capture-product-catalog-separate-process-runtime.mjs
```

A successful capture proves provider schema preflight, authenticated owner RPC execution, and consumer remote-profile startup as separate processes. It does not prove Commerce checkout or AI generation requests through the consumer HTTP/GraphQL surface, so those business end-to-end gates remain open.

The implementation agent does not claim the source-level invocation or runtime capture command was executed. Product remains `boundary_ready` until the capture result and separate-process Commerce/AI business evidence are retained.
