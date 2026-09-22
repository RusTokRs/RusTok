# Typed tenant resolution boundary

- Status: Accepted
- Date: 2026-07-17

## Decision

Tenant resolution is a typed domain boundary, not a string switch in HTTP middleware.

`TenantSettings.resolution` uses `TenantResolutionMode`. Unknown configuration values fail during deserialization. Structural combinations are validated once by `TenantSettings`, including subdomain base domains and development-only fallback policy.

`middleware/tenant_resolution.rs` is the single owner of tenant identifier extraction, validation, source classification and typed resolution failures. `middleware/tenant_route_policy.rs` separately owns route-scope classification so transport exposure cannot become resolver logic.

Every tenant-bound transport consumes a `TenantResolution` and records telemetry from its typed source. HTTP derives the resolution from the request; self-resolving handshakes create an explicit typed slug resolution before using the same cache-aware context loader. No transport invents source labels or predicts fallback behavior. When both the configured tenant header and `X-Tenant-Slug` are supplied, the slug is treated as a correlated assertion and must match the tenant loaded by the primary identifier.

## Route scopes

- `TenantBound`: normal HTTP requests resolved by tenant middleware.
- `GlobalOperator`: health, metrics, schema, installer and read-only platform registry surfaces.
- `SelfResolvingHandshake`: GraphQL WebSocket, which requires `connection_init.tenantSlug`, resolves an active tenant and binds authentication to that tenant before executing operations.

Adding a route to either non-tenant scope requires changing the canonical route policy and its tests. Prefix matching is segment-aware, so a route such as `/healthcare` is tenant-bound and cannot inherit `/health` privileges.

## Consequences

- Unsupported modes cannot silently select the default tenant.
- Slug and host identifiers no longer carry fabricated UUID values.
- Development fallback is observable only when it actually occurs.
- Clock failures propagate from cache timestamp creation instead of becoming epoch zero.
- Startup validation remains defense in depth but delegates to the same typed settings policy.
