# rustok-product-catalog-service / CRATE_API

## Executable interface

This is a binary package with no public Rust library API. Its supported operator
interface is the [environment/startup contract](README.md#required-configuration),
authenticated gRPC endpoint, telemetry and shutdown behavior.

The private implementation types `ServiceConfig`, `ServiceTransport` and
`RedactedSecret` are not consumer APIs. `RedactedSecret(String)` redacts Debug output;
it does not establish secure-memory storage or zeroization.

## Composition and exposed operations

The host composes PostgreSQL, Product's `CatalogService`, an Outbox-backed
`TransactionalEventBus` and `ProductCatalogGrpcService` with bearer authentication.
It exposes `ReadProductProjection`, `ReadVariantProductProjection` and
`ListPublishedProducts`. It does not expose commands, schema administration, every
read-trait capability or all Product UI routes.

Product owns catalog rules and queries. The transport package owns wire framing and
trusted authority adaptation. Outbox supplies the production event-bus constructor;
the exposed read-only operations do not perform domain/outbox writes.

## Startup and failure contract

- Required database, credential and trusted-service-actor configuration is documented
  in the README. Credentials and raw database URLs must not appear in diagnostics.
- The private preflight performs entity reads of `products`, `product_variants` and
  `sys_events` before the listener. It is a bounded prerequisite check, not complete
  schema/dependency/business-path verification.
- The host does not run migrations. Deployment must provide a migrated owner schema.
- TLS is required unless explicitly permitted plaintext is bound to loopback.
- Caller claims are admitted through the interceptor; the trusted actor is configured
  server-side. Initialization failure aborts startup.
- Platform telemetry uses service name `rustok-product-catalog-service`. Ctrl-C and
  Unix SIGTERM request graceful tonic and telemetry shutdown.

## Deployment limits and evidence

The catalog selector in the consumer does not select remote Product commands. Native
Product/Pricing/Inventory atomic participants remain a separate local contract. Sharing
owner storage and independent storage/replication are different deployment choices.

Current compilation, provider startup and mounted Commerce/AI business evidence remain
open as described by the [Product plan](../rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
Historical loopback attestation does not certify the current revision or a standalone
production deployment.
