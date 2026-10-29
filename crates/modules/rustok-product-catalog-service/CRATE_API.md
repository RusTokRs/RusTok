# rustok-product-catalog-service / CRATE_API

## Public surface

### Service Host Configuration
- `ServiceConfiguration`: parsed environment configuration for host address, database URL, credentials, and TLS parameters.
- `ServiceTransport`: transport security mode (`Tls`, `InsecureLoopback`).
- `RedactedSecret`: memory-safe secret container redacting sensitive bearer credentials in diagnostic output.

### Entry Point
- `main`: async service daemon entrypoint performing database pool setup, schema preflight verification, and tonic gRPC listener binding.

## Platform Capability Bindings
- **Standalone FBA Service Host**: Implements out-of-process provider execution for `ProductCatalogReadPort`.
- **gRPC Protocol Endpoint**: Exposes authenticated tonic gRPC read endpoints (`GetProduct`, `GetVariant`, `ListCatalogProducts`).
- **Telemetry Registration**: Initializes distributed tracing and metrics with service identity `rustok-product-catalog-service`.

## Domain Collaborations
- **`rustok-product`**: Core domain catalog queries and entity projections (`Product`, `ProductVariant`).
- **`rustok-product-transport`**: Tonic gRPC service implementation and bearer authentication interceptor (`ProductCatalogGrpcBearerInterceptor`).
- **`rustok-outbox`**: Read preflight verification of transactional outbox table schema (`sys_events`).

## Minimum Contract Set

### Input DTOs/Commands
- Service configuration derived from standard environment variables (`RUSTOK_PRODUCT_CATALOG_DATABASE_URL`, `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN`, `RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR`).

### Domain Invariants
- **Read-Only Surface**: The standalone host exposes strictly read-only catalog query endpoints; mutations and schema alterations are excluded.
- **Fail-Closed Schema Preflight**: Preflight checks query `products`, `product_variants`, and `sys_events` before binding the listener; missing schema aborts immediately.
- **TLS by Default**: Insecure plaintext HTTP is rejected unless explicitly enabled for loopback development via `RUSTOK_PRODUCT_CATALOG_SERVICE_ALLOW_INSECURE_LOOPBACK=true`.
- **Trusted Service Identity**: Callers cannot spoof actor identities; the host server enforces `RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR`.

### Events / Outbox Side Effects
- Emits telemetry spans and structured startup diagnostics.
- Outbox write operations remain inactive on this read-only deployment unit.

### Errors / Failure Codes
- Sanitized startup failure errors for database connection failures, missing schema tables, and invalid TLS credentials.
