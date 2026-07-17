# Request trust, strict tenant fallback and forwarded-header policy

- Date: 2026-04-03
- Status: Accepted

## Context

`apps/server` used request host, client IP and proto information in multiple places (`tenant`, `channel`, rate limiting, OAuth browser-session cookies), but forwarded header trust was implicit and inconsistent. Tenant header resolution could also silently fall back to the default tenant in configurations where a tenant identifier was expected on every request.

These two behaviors weakened tenant isolation and made production safety depend too heavily on external ingress correctness.

## Decision

- Introduce a single runtime policy `settings.rustok.runtime.request_trust` with:
  - `forwarded_headers_mode = "ignore" | "trusted_only"`
  - `trusted_proxy_cidrs = []`
- Default to `ignore`, meaning `Forwarded` / `X-Forwarded-*` are not trusted unless an operator explicitly opts in.
- Route tenant, channel, rate-limit and OAuth secure-cookie transport decisions through one shared request-trust helper.
- Add explicit tenant fallback policy with `settings.rustok.tenant.fallback_mode = "disabled" | "default_tenant"`.
- Keep the default production posture as strict:
  - `resolution=header` + `fallback_mode=disabled` => missing tenant header returns `400`
  - disabled tenants are rejected during tenant middleware resolution with `403`
  - `resolution=subdomain` requires configured `base_domains` and only accepts a single left-most label.

## Consequences

- Production deployments become safer by default against forwarded-header spoofing and accidental tenant confusion.
- Installations behind trusted ingress must explicitly declare proxy CIDR ranges before forwarded headers start influencing routing or rate limiting.
- Dev/test environments can still opt into `default_tenant` fallback when convenient, but this must now be explicit in configuration.
- Documentation and tests must keep the request-trust contract aligned across tenant, channel, rate limiting and OAuth browser-session flows.


## Canonical tenant context loading

HTTP middleware and self-resolving transports such as GraphQL WebSocket use the same cache-aware tenant read-port pipeline. Transport code may choose an identifier, but it must not query tenant persistence directly or reconstruct `TenantContext` independently. Resolution policy remains internal to the server crate; the public middleware facade exposes only the middleware and operational cache controls.
