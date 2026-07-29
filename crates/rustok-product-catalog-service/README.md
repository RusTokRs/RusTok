# RusTok Product catalog service

`rustok-product-catalog-service` is the standalone provider-side deployment unit for `ProductCatalogReadPort`.

The binary composes the canonical Product owner service and the replaceable tonic adapter:

```text
PostgreSQL
   │
   ▼
CatalogService
   │ ProductCatalogReadPort
   ▼
ProductCatalogGrpcService
   │ ProductCatalogGrpcBearerInterceptor
   ▼
authenticated tonic gRPC
```

It does not own Product DTOs, catalog policy, persistence queries, locale/channel rules, consumer fallback behavior, migrations, or an outbox relay. The exposed RPC surface is read-only. `CatalogService` receives an `OutboxTransport`-backed `TransactionalEventBus` so the owner constructor remains production-valid without introducing a no-op event transport.

## Required configuration

| Variable | Purpose |
| --- | --- |
| `RUSTOK_PRODUCT_CATALOG_DATABASE_URL` | PostgreSQL URL. Falls back to `RUSTOK_DATABASE_URL`, then `DATABASE_URL`. |
| `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN` | Shared service credential also configured on the consumer server. Never place it in source control or endpoint URLs. |
| `RUSTOK_PRODUCT_CATALOG_TRUSTED_SERVICE_ACTOR` | Server-owned identity assigned to authenticated calls, for example `rustok-server`. Caller-provided `PortContext.actor`, claims, and roles remain untrusted. |

The database schema must already be migrated by the platform migration workflow. This service does not run migrations at startup.

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

The implementation agent does not claim this command was executed. Product remains `boundary_ready` until the host and consumers are executed together and retained evidence is committed.
