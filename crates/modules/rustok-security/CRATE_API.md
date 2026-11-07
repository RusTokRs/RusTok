# rustok-security / CRATE_API

## Role

`rustok-security` owns the sovereign domain capability for Security Suite, IP filtering, Rate Limiting, Captcha verification, and Threat Protection in RusToK.

Persistence entities are **crate-private**. Consumers interact exclusively through owner DTOs,
services, or explicit ports. Direct cross-crate SeaORM table access is forbidden.

## Public surface

Deliberate public modules and exports:

- `adapters` — `HoneypotVerifier`, `MokaBruteForceTracker`, `MokaRateLimiter`, `NoopCaptchaVerifier`, `RecaptchaVerifier`, `TurnstileVerifier`;
- `domain` — `SecurityStatus`, `IpRule`, `IpRuleType`, `SecurityAuditLog`, `SecurityPolicy`, `LockoutDecision`, `RateLimitDecision`, `CaptchaProviderKind`;
- `dto` — `CreateIpRuleInput`, `LogSecurityEventInput`, `UpdateSecurityPolicyInput`, `SecuritySummary`;
- `error` — `SecurityError`, `SecurityResult`;
- `events` — `SecurityCreatedEvent`, `SecurityDeletedEvent`, `SecurityDomainEvent`, `SecurityUpdatedEvent`;
- `integrations` — capability bridges for search, cache, media, translation;
- `module` — `SecurityModule` lifecycle and migration provider;
- `ports` — `RateLimiterPort`, `CaptchaVerifierPort`, `IpFilterPort`;
- `services` — `SecurityService` threat protection and transaction orchestration.

`entities` and `migrations` are crate-private.

## Primary entry points

- [`SecurityModule`](src/module.rs): Platform module registration and migration descriptor.
- [`SecurityService`](src/services/mod.rs): Domain application service.
- [`RateLimiterPort`](src/ports.rs): SPI for request rate limiting across IP, tenant, or route keys.
- [`CaptchaVerifierPort`](src/ports.rs): SPI for human challenge verification (Turnstile, ReCaptcha, Honeypot).
- [`IpFilterPort`](src/ports.rs): SPI for tenant-scoped IP whitelist/blacklist enforcement.

## Invariants

- Tenant isolation: All queries, IP rules, policies, and audit logs strictly enforce `tenant_id` scoping.
- Concurrency: Write operations execute within explicit transaction boundaries.
- Fail-Closed: Rate limit and IP evaluation fail-closed when policy constraints are violated.

## Platform Capability Bindings

- **rustok-outbox**: Transactional outbox events and durable idempotency receipts
- **rustok-tenant**: Multi-tenant data isolation (`TenantId`)

## Domain Collaborations

- Threat protection and rate limiting across HTTP/GraphQL admission filters.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateIpRuleInput`: create IP CIDR whitelist or blacklist rule.
- `UpdateSecurityPolicyInput`: configure rate limit windows, lockout thresholds, and captcha provider.
- `LogSecurityEventInput`: record structured security audit event.
- `SecuritySummary`: lightweight module descriptor.

### Domain Invariants
- Tenant isolation: All queries and commands strictly enforce `tenant_id` scoping.
- Concurrency: Write operations execute within explicit transaction boundaries.

### Events / Outbox Side Effects
- Domain mutations record transactional outbox events via `rustok-outbox`.

### Errors / Failure Codes
- Strongly-typed `SecurityError` with structured failure categories (`RateLimitExceeded`, `IpBlocked`, `CaptchaFailed`, `Validation`, `Database`).
