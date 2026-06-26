---
id: doc://docs/guides/rate-limiting.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Rate Limiting in RusToK

## Overview

RusToK uses a sliding-window HTTP rate limiter in `apps/server` to protect API paths from brute force, abuse, and short traffic spikes.

This limiter is an HTTP-layer control. It does not replace `rustok-core::security::RateLimiter`, which remains an internal security primitive.

## Current Contract

- client identity starts from IP address;
- IP priority is `X-Forwarded-For` -> `X-Real-IP` -> `ip:unknown`;
- spoofable headers such as `X-User-ID` are ignored;
- rate-limit violations return `429 Too Many Requests`;
- responses expose `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset`.

## Trusted Dimensions

When `rustok.rate_limit.trusted_auth_dimensions=true`, the limiter keeps the base IP bucket and extends it only from verified bearer-token claims:

- direct bearer token -> `ip + tenant`
- OAuth bearer token -> `ip + tenant + oauth_app`

Requests without a valid bearer token stay on the plain IP bucket. This is especially important for `/api/oauth/token` and `/api/oauth/revoke`, where client input is not trusted before verification.

## Namespaces

Current wiring in `apps/server/src/app.rs` uses separate limiter namespaces:

- `/api/*` -> `api`
- `/api/auth/login`, `/api/auth/register`, `/api/auth/reset*` -> `auth`
- `/api/oauth/token`, `/api/oauth/revoke`, `/api/oauth/authorize` -> `oauth`

This lets runtime guardrails, metrics, and backend health be tracked separately per limiter policy.

## Configuration

Relevant settings live under `rustok.rate_limit`:

- `enabled`
- `backend`
- `redis_key_prefix`
- `requests_per_minute`
- `burst`
- `auth_requests_per_minute`
- `auth_burst`
- `oauth_requests_per_minute`
- `oauth_burst`
- `trusted_auth_dimensions`

## Recommended Profiles

Single-instance / dev profile:

```yaml
settings:
  rustok:
    rate_limit:
      enabled: true
      backend: memory
      requests_per_minute: 60
      burst: 10
      auth_requests_per_minute: 20
      auth_burst: 5
      oauth_requests_per_minute: 30
      oauth_burst: 5
      trusted_auth_dimensions: true
```

Multi-instance / Redis-backed profile:

```yaml
settings:
  rustok:
    rate_limit:
      enabled: true
      backend: redis
      redis_key_prefix: rate-limit:v1
      requests_per_minute: 60
      burst: 10
      auth_requests_per_minute: 20
      auth_burst: 5
      oauth_requests_per_minute: 30
      oauth_burst: 5
      trusted_auth_dimensions: true
    runtime:
      guardrails:
        rollout: observe
```

Recommended rollout:

1. switch only the backend to `redis`;
2. keep guardrails in `observe`;
3. verify `/health/runtime` and `/metrics`;
4. switch to `enforce` after backend stability is confirmed.

Repository preset:

- [`production.redis.example.yaml`](/C:/проекты/RusTok/apps/server/config/production.redis.example.yaml)

Startup validation:

- if `rustok.rate_limit.enabled=true` and `backend=redis`, server now fails fast unless `RUSTOK_REDIS_URL` or `REDIS_URL` is set;
- if rate limiting is disabled, server no longer requires Redis just because `backend=redis` is configured for a future rollout.

## Response Contract

Successful response:

```http
HTTP/1.1 200 OK
X-RateLimit-Limit: 300
X-RateLimit-Remaining: 299
X-RateLimit-Reset: 60
```

Rate-limited response:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 42
X-RateLimit-Limit: 20
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 42
Content-Type: text/plain

Rate limit exceeded
```

## Verification

```bash
cargo test -p rustok-server rate_limit
cargo check -p rustok-server --bin rustok-server
```

## Related Docs

- [`runtime-guardrails.md`](/C:/проекты/RusTok/docs/guides/runtime-guardrails.md)
- [`principles.md`](/C:/Users/user/Documents/New%20project/RusTok/docs/architecture/principles.md)

## Trusted proxy contract

- By default the limiter keys requests by the real peer/socket IP and falls back to `ip:unknown`.
- `Forwarded` / `X-Forwarded-*` participate in IP derivation only when `settings.rustok.runtime.request_trust.forwarded_headers_mode=trusted_only` and the peer IP matches `trusted_proxy_cidrs`.
- The middleware now selects exactly one policy per request in priority order `oauth -> auth -> api`, so `/api/auth/*` no longer consumes multiple limiter buckets for a single call.
