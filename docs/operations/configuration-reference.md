# RusToK Platform Configuration & Environment Reference

This document is the canonical reference for runtime configuration, environment variables, and deployment profiles in RusToK.

---

## 1. Configuration Hierarchy and Precedence

Configuration values are resolved in strict priority order (highest to lowest):

1. **Explicit CLI flags** (e.g. `--port 5150`, `--config /path/to/custom.yaml`).
2. **Environment Variables** (e.g. `DATABASE_URL`, `REDIS_URL`, `RUSTOK_*`).
3. **Deployment YAML Configuration** (`apps/server/config/{environment}.yaml`).
4. **Compiled Domain Defaults** (built into crates and services).

---

## 2. Server & Network Configuration

| Key | Environment Variable | Default (Dev) | Description |
| :--- | :--- | :--- | :--- |
| `server.binding` | `RUSTOK_BINDING` | `127.0.0.1` | Network interface address to bind (`0.0.0.0` in Docker/production). |
| `server.port` | `PORT`, `RUSTOK_PORT` | `5150` | TCP port for HTTP and GraphQL API listener. |
| `server.host` | `RUSTOK_HOST`, `RUSTOK_BASE_URL` | `http://127.0.0.1` | Canonical public base URL emitted in redirects and OpenAPI/GraphQL schemas. |
| `RUSTOK_HTTPS` | `RUSTOK_HTTPS` | `false` | When `true`, enables HSTS and marks session cookies as `Secure`. Mandatory in production. |

---

## 3. Database & Connection Pooling

| Key | Environment Variable | Default (Dev) | Description |
| :--- | :--- | :--- | :--- |
| `database.uri` | `DATABASE_URL` | `postgres://rustok:rustok@localhost:5432/rustok_dev` | Connection string for PostgreSQL or SQLite (`sqlite://rustok.db?mode=rwc`). |
| `database.min_connections` | `DATABASE_MIN_CONNECTIONS` | `5` | Minimum idle connections retained in the pool. |
| `database.max_connections` | `DATABASE_MAX_CONNECTIONS` | `30` | Maximum concurrent connections allowed by SeaORM pool. |
| `database.connect_timeout` | `DATABASE_CONNECT_TIMEOUT` | `10000` | Connection acquisition timeout in milliseconds. |
| `database.idle_timeout` | `DATABASE_IDLE_TIMEOUT` | `300000` | Connection idle timeout in milliseconds before eviction. |
| `database.enable_logging` | `DATABASE_ENABLE_LOGGING` | `true` | When `true`, logs SQL queries at DEBUG/TRACE level. Must be `false` in production. |
| `database.auto_migrate` | `DATABASE_AUTO_MIGRATE` | `true` | Automatically run migrations on server startup. Strictly `false` in production deployments. |

---

## 4. Authentication, JWT & Security

| Key | Environment Variable | Default (Dev) | Description |
| :--- | :--- | :--- | :--- |
| `auth.jwt.secret` | `JWT_SECRET`, `RUSTOK_JWT_SECRET` | *dev placeholder* | HS256 signing secret. Production requires >= 64 high-entropy random bytes. |
| `auth.jwt.expiration` | `JWT_EXPIRATION` | `604800` (7 days) | Access token lifetime in seconds. Recommended `3600` (1 hour) in production. |
| `settings.auth.refresh_expiration` | `REFRESH_TOKEN_EXPIRATION` | `1209600` (14 days) | Refresh token lifetime in seconds before re-authentication is required. |
| `settings.auth.issuer` | `JWT_ISSUER` | `rustok` | Claim issuer (`iss`). Must match deployment API URL in production. |
| `settings.auth.audience` | `JWT_AUDIENCE` | `rustok-admin` | Intended token audience (`aud`). |
| `auth.session_cookie` | — | `rustok-session` | HttpOnly session cookie name issued to first-party browser clients. |

---

## 5. Multi-Tenancy & Runtime Profiles

RusToK enforces tenant isolation at runtime through explicit profiles defined in [tenant-runtime-profiles.md](./tenant-runtime-profiles.md):

| Key | Environment Variable | Supported Values | Description |
| :--- | :--- | :--- | :--- |
| `settings.rustok.tenant.profile` | `RUSTOK_TENANT_PROFILE` | `multi_tenant`, `single_tenant`, `development` | Core isolation behavior. `development` permits fallback; forbidden in production. |
| `settings.rustok.tenant.resolution` | `RUSTOK_TENANT_RESOLUTION` | `header`, `host`, `subdomain` | How the request tenant is extracted from incoming HTTP requests. |
| `settings.rustok.tenant.header_name` | `RUSTOK_TENANT_HEADER` | `X-Tenant-ID` | HTTP header name used when resolution mode is `header`. |
| `settings.rustok.tenant.default_id` | `RUSTOK_DEFAULT_TENANT_ID` | `00000000-0000-...-000000000001` | Default tenant UUID used in `single_tenant` or dev fallback. |
| `settings.rustok.tenant.base_domains` | `RUSTOK_BASE_DOMAINS` | `[]` | Allowed root domains when extracting tenants via `subdomain`. |

### 5.1 FBA provider selectors

These existing selectors are parsed from environment during server bootstrap, separately
from the tenant-profile settings above. Both default to `embedded`. They select bounded
capabilities, not complete module placement.

| Selector | Required remote configuration | Scope |
|---|---|---|
| `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` | `RUSTOK_PRODUCT_CATALOG_GRPC_ENDPOINT` and `RUSTOK_PRODUCT_CATALOG_GRPC_BEARER_TOKEN` | Three base catalog projections, the storefront variant projection used by Cart, and two SKU identity reads. Product commands remain embedded. |
| `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` | `RUSTOK_PROFILE_MEDIA_GRPC_ENDPOINT`; optional `RUSTOK_PROFILE_MEDIA_PUBLIC_ORIGIN` | Profiles public-image descriptors only; HTTP bytes and other Media operations retain their owner path. |

Both connection configurations support their documented TLS-domain, bounded connect-timeout
and explicit insecure-loopback options. Remote-only variables are rejected in embedded
mode; invalid remote configuration or connection aborts startup. There is no silent local
failover. The narrow Media selector does not configure provider authentication or client
certificates; these remain deployment gates.

Use the [Product connection contract](../../crates/modules/rustok-product/ports/README.md),
[Product provider host](../../crates/modules/rustok-product/README.md#standalone-catalog-profile)
and [Media connection contract](../../crates/modules/rustok-media-transport/docs/README.md#server-deployment-wiring)
for exact options and limits. There is no current whole-module `RUSTOK_MEDIA_PROVIDER`
selector. See [FBA scope](../backend/module-backend-architecture.md#fba-contracts).

---

## 6. Rate Limiting & Protection

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `settings.rustok.rate_limit.enabled` | `RATE_LIMIT_ENABLED` | `true` | Global toggle for ingress rate limiting. |
| `settings.rustok.rate_limit.backend` | `RATE_LIMIT_BACKEND` | `memory` | Rate limit storage backend: `memory` (in-process) or `redis`. |
| `settings.rustok.rate_limit.requests_per_minute` | — | `60` | General API request quota per tenant/IP window. |
| `settings.rustok.rate_limit.burst` | — | `10` | Allowable burst capacity for standard requests. |
| `settings.rustok.rate_limit.auth_requests_per_minute` | — | `20` | Strict quota for login/token issue endpoints. |
| `settings.rustok.rate_limit.oauth_requests_per_minute` | — | `30` | Quota for OAuth verification and authorization flows. |

---

## 7. Event Bus, Transactional Outbox & Messaging

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `settings.rustok.events.delivery_profile` | `EVENT_DELIVERY_PROFILE` | `outbox` | Delivery mechanism: `outbox` (local relay), `outbox_iggy` (distributed stream). |
| `settings.rustok.events.relay_interval_ms` | `OUTBOX_RELAY_INTERVAL_MS` | `1000` | Polling interval in ms for the transactional outbox relay worker. |
| `settings.rustok.events.channel_capacity` | `EVENT_CHANNEL_CAPACITY` | `256` | In-memory broadcast buffer capacity for domain event subscribers. |
| `iggy.tcp_address` | `IGGY_TCP_ADDRESS` | `127.0.0.1:8090` | TCP connection endpoint for bundled or external Iggy message stream. |
| `iggy.root_username` | `IGGY_ROOT_USERNAME` | `iggy` | Administrative credentials for stream creation and topic management. |
| `iggy.root_password` | `IGGY_ROOT_PASSWORD` | `iggy` | Secret credentials for Iggy cluster authentication. |

---

## 8. Search & Projection Engine

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `settings.rustok.search.enabled` | `SEARCH_ENABLED` | `true` | Toggle for platform search indexing and queries. |
| `settings.rustok.search.driver` | `SEARCH_DRIVER` | `postgres` | Search provider: `postgres` (native tsvector), `meilisearch`. |
| `settings.rustok.search.url` | `SEARCH_URL`, `MEILISEARCH_URL` | `""` | Search service endpoint URL when using remote search drivers. |
| `settings.rustok.search.api_key` | `SEARCH_API_KEY` | `null` | Authentication key for external search clusters. |
| `settings.rustok.search.index_prefix` | `SEARCH_INDEX_PREFIX` | `rustok_dev_` | Multi-tenant index prefix preventing namespace collisions. |

---

## 9. Caching & Redis

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `redis.url` | `REDIS_URL`, `RUSTOK_REDIS_URL` | *none* | Redis connection URI (e.g. `redis://localhost:6379/0`). |
| `cache.backend` | `CACHE_BACKEND` | `memory` | Caching engine: `memory` (Moka in-process), `redis`. |
| `cache.default_ttl_secs` | `CACHE_DEFAULT_TTL_SECS` | `300` (5m) | Base TTL for domain query results and permission cache snapshots. |

---

## 10. Logging & Observability

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `logger.enable` | `LOGGER_ENABLE` | `true` | Enables tracing/logging subscriber. |
| `logger.level` | `RUST_LOG`, `LOGGER_LEVEL` | `debug` | Minimum log severity: `trace`, `debug`, `info`, `warn`, `error`. |
| `logger.format` | `RUSTOK_LOG_FORMAT` | `compact` | Output format: `compact` (ANSI colored terminal), `json` (cloud ingestion). |
| `logger.pretty_backtrace` | `RUST_BACKTRACE` | `true` | Enables symbolicated stack traces on panics. Set `false` in production. |
| `otel.enabled` | `OTEL_ENABLED` | `false` | Enables OpenTelemetry distributed tracing export. |
| `otel.exporter_otlp_endpoint` | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | gRPC/HTTP endpoint for OpenTelemetry collector. |
| `otel.service_name` | `OTEL_SERVICE_NAME` | `rustok-server` | Service identifier reported to tracing backends. |
| `otel.sampling_rate` | `OTEL_SAMPLING_RATE` | `1.0` | Head-based trace sampling ratio (0.0 to 1.0). |

---

## 11. Email & Outbound Communication

| Key | Environment Variable | Default | Description |
| :--- | :--- | :--- | :--- |
| `settings.rustok.email.enabled` | `EMAIL_ENABLED` | `false` | Master toggle for outbound transactional email. |
| `settings.rustok.email.provider` | `EMAIL_PROVIDER` | `smtp` | Mail transport provider. |
| `settings.rustok.email.from` | `EMAIL_FROM` | `no-reply@example.com` | Default sender email address in RFC 5322 format. |
| `settings.rustok.email.reset_base_url`| `RESET_BASE_URL` | `http://localhost:3000/reset` | Base URL used to construct password reset verification links. |
| `settings.rustok.email.smtp.host` | `SMTP_HOST` | `localhost` | Outbound SMTP relay hostname. |
| `settings.rustok.email.smtp.port` | `SMTP_PORT` | `587` | Outbound SMTP relay port (typically 587 for STARTTLS, 465 for TLS). |
| `settings.rustok.email.smtp.username` | `SMTP_USERNAME` | *none* | Authentication username for SMTP relay. |
| `settings.rustok.email.smtp.password` | `SMTP_PASSWORD` | *none* | Authentication secret for SMTP relay. Scrubbed on debug inspect. |

---

## 12. Verification and Operational Runbooks

For specific operational workflows, consult the dedicated guides:
- [Safe Database Migrations & Squashing](../guides/safe-database-migrations-and-squashing.md)
- [Tenant Runtime Profiles Specification](./tenant-runtime-profiles.md)
- [Security Audit & Vulnerability Runbook](../guides/security-audit.md)
- [Continuous Code Review & Slop Remediation Standards](../CONTINUOUS_CODE_REVIEW.md)
