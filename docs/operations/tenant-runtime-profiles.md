# Tenant runtime profiles

## Purpose

`settings.rustok.tenant.profile` is the source of truth for tenant selection behavior. It prevents production behavior from being inferred from a boolean flag or an invalid string mode.

## Profiles

| Profile | Tenant source | `tenant.enabled` compatibility value | Production |
| --- | --- | --- | --- |
| `multi_tenant` | Request-derived through the configured resolution mode | `true` | Allowed |
| `single_tenant` | `tenant.default_id` | `false` | Allowed |
| `development` | Request-derived; optional default-tenant fallback | `true` | Forbidden |

`tenant.enabled` remains temporarily as a compatibility field. Startup validation rejects any disagreement between it and `tenant.profile`; runtime behavior is selected by the profile, not by the boolean.

## Resolution modes

Request-derived profiles use the typed `tenant.resolution` value:

- `header`: resolve from `tenant.header_name`, with the compatibility `X-Tenant-Slug` assertion supported.
- `host` and `domain`: resolve from the validated effective host.
- `subdomain`: extract a tenant identifier only beneath an explicitly configured `tenant.base_domains` entry.

Unknown modes fail configuration deserialization. There is no catch-all runtime fallback.

## Fallback policy

`tenant.fallback_mode: default_tenant` is valid only when all of the following are true:

1. `tenant.profile` is `development`.
2. `tenant.resolution` is `header`.
3. The process is not running with the production deployment profile.

Production rejects the entire `development` profile, even when fallback is disabled.

## Examples

### Production multi-tenant

```yaml
settings:
  rustok:
    tenant:
      profile: multi_tenant
      enabled: true
      resolution: header
      header_name: X-Tenant-ID
      fallback_mode: disabled
```

### Production single-tenant

```yaml
settings:
  rustok:
    tenant:
      profile: single_tenant
      enabled: false
      default_id: 00000000-0000-0000-0000-000000000001
      fallback_mode: disabled
```

### Local development fallback

```yaml
settings:
  rustok:
    tenant:
      profile: development
      enabled: true
      resolution: header
      header_name: X-Tenant-ID
      default_id: 00000000-0000-0000-0000-000000000001
      fallback_mode: default_tenant
```

## Observability

Tenant resolution records the dedicated metric:

```text
rustok_tenant_resolutions_total{transport,source,outcome}
```

The bounded labels identify the transport (`http` or `graphql_ws`), the typed resolution source, and the final success or failure class. Tenant resolution is not reported as a cache operation.

## Verification

```bash
cargo test -p rustok-server tenant_policy_
cargo test -p rustok-server middleware::tenant_resolution::tests
cargo test -p rustok-server graphql_ws_tenant_handshake_fails_closed
cargo test -p rustok-server --test tenant_resolver_invariants_test
cargo test -p rustok-telemetry --test metrics_test
node scripts/verify/verify-tenant-resolution-architecture.mjs
```

The negative integration coverage includes REST-style middleware paths, GraphQL HTTP, GraphQL WebSocket handshake resolution, and storefront paths. Missing, malformed, unknown, conflicting, and disabled tenant assertions must fail closed.
