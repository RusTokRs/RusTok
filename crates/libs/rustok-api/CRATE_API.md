# rustok-api / CRATE_API

## Role

`rustok-api` is the canonical platform contract library for RusToK. It provides shared web,
request, tenant, authentication, locale, permission, and port abstractions for `apps/server` and
sovereign domain modules.

`rustok-api` contains **no domain business logic** and no direct table entities. It acts as the
stable transport-neutral foundation for multi-tenancy, authorization, and cross-boundary ports.

## Public surface

Deliberate public modules and exports:

- `context` — typed request contexts for multi-tenant and channel-aware execution:
  - `TenantContext`, `OptionalTenant`, `TenantContextExt`
  - `AuthContext`, `AuthPrincipalContext`, `AuthPrincipalKind`, `OptionalAuthContext`
  - `ChannelContext`, `ChannelResolutionOutcome`, `OptionalChannel`
  - `HostAuthorityContext`, `HostAuthority`
- `permissions` — platform-wide authorization vocabulary:
  - `Permission`, `Action`, `Resource`
  - `has_effective_permission`, `has_any_effective_permission`
- `ports` — cross-boundary application port mechanics:
  - `PortContext`, `PortCallPolicy`, `PortError`, `PortErrorKind`, `PortOperationKind`
  - `PortActor`, `PortActorKind`
- `runtime` (under `runtime` / `server` features) — host-managed runtime handles:
  - `HostRuntimeContext`, `HostSettingsSnapshot`
  - `tenant_module_settings`, `tenant_module_settings_in_tx`
- `richtext` — neutral document AST representation:
  - `RichTextDocument`, `RichTextNode`, `RichTextMark`, `RichTextView`, `RichTextProfileId`
- `locale` — canonical Unicode/CLDR locale tagging:
  - `RuntimeLocale`, `StoredLocale`, `TenantLocale`, `normalize_locale_tag`, `build_locale_candidates`
- `tenant_rbac` — tenant-scoped RBAC definitions:
  - `TenantRbacCatalog`, `TenantRbacRole`, `TenantRbacPermission`
- `artifact_ui` & `artifact_permissions` — redacted UI projection contracts for host components.
- `digest` — constant-time SHA-256 and HMAC verification utilities.

## Feature flags

- `default` — Core context types, permissions, ports, richtext, locale, and neutral DTOs.
- `runtime` — Enables `HostRuntimeContext` and database transaction settings access (`sea-orm`).
- `server` — Enables Axum extractors, Async-GraphQL scalar/type conversions, and request locale helpers.

## Primary entry points

- [`TenantContext`](src/context/mod.rs): Scopes database queries, caches, and storage to a single tenant.
- [`AuthContext`](src/context/mod.rs): Carries validated actor identity and permission claims.
- [`Permission`](src/permissions/mod.rs): Strongly-typed platform permission identifier.
- [`PortContext`](src/ports/mod.rs): Invocation envelope for cross-module port execution.
- [`PortCallPolicy`](src/ports/mod.rs): Enforces idempotent write or read-only constraints on port boundaries.
- [`HostRuntimeContext`](src/runtime/mod.rs): Shared runtime provider bag for native adapters and server functions.

## Invariants

1. **Strict tenant scoping**: Request contexts never allow implicit cross-tenant data access.
2. **Fail-closed authorization**: Permission evaluation treats missing or malformed claims as access denial.
3. **Transport neutrality**: Core DTOs are framework-agnostic and serialize cleanly to JSON, GraphQL, and native IPC.
4. **Zero domain coupling**: Does not depend on module-specific entities or application services.
