---
id: doc://docs/standards/patterns-vs-antipatterns.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# RusToK — Patterns vs Antipatterns

A comprehensive reference of correct and incorrect approaches when developing on the RusToK platform.

Each section contains: what to do correctly (✅), what is forbidden (❌), why, and a link to the detailed document.

> **Status:** Living document. Update when adding new modules, patterns, or discovering new antipatterns.

---

## Table of Contents

1. [Architecture](#1-architecture)
2. [Code Quality (Rust)](#2-code-quality-rust)
3. [Data and DB](#3-data-and-db)
4. [Event System](#4-event-system)
5. [Auth and RBAC](#5-auth-and-rbac)
6. [Multi-Tenancy](#6-multi-tenancy)
7. [API (GraphQL + REST)](#7-api)
8. [Frontend](#8-frontend)
9. [Testing](#9-testing)
10. [Observability](#10-observability)
11. [Security](#11-security)
12. [DevOps and CI/CD](#12-devops-and-cicd)

---

## 1. Architecture

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 1.1 | Module implements `RusToKModule` trait and registers in `build_registry()` | Module connects directly in `app.rs` bypassing registry | Bypasses lifecycle, health checks, per-tenant toggle | [architecture/modules.md](../architecture/modules.md) |
| 1.2 | Core modules return `ModuleKind::Core`, optional ones return `ModuleKind::Optional` | All modules have the same `kind()` | Core modules cannot be disabled, a formal boundary is needed | [architecture/principles.md](../architecture/principles.md) |
| 1.3 | `dependencies()` in `RusToKModule` matches `depends_on` in `modules.toml` | Dependencies defined only in Cargo.toml or only in modules.toml | Runtime check doesn't catch desync, module enables without dependency | [modules/manifest.md](../modules/manifest.md) |
| 1.4 | Business logic in domain crates (`crates/rustok-*`), controllers are thin | Business logic in controllers/resolvers | Duplication between REST and GraphQL, untestability | [architecture/overview.md](../architecture/overview.md) |
| 1.5 | Modules interact via EventBus, not direct calls | Direct calls between domain modules | Coupling, violation of event-driven principle | [architecture/overview.md](../architecture/overview.md) |
| 1.6 | Write path — normalized tables, Read path — denormalized index | One set of tables for write and read | Violates CQRS-lite, slow storefront | [architecture/overview.md §CQRS-lite](../architecture/overview.md) |
| 1.7 | Loco hooks (`Hooks::routes`, `after_routes`, `connect_workers`) for lifecycle | Custom "pure Axum" lifecycle | Bypasses Loco initialization, dependency injection, middleware chain | [ai/KNOWN_PITFALLS.md §Loco](../ai/KNOWN_PITFALLS.md) |
| 1.8 | Shared dependencies via `AppContext.shared_store` | Global singleton objects (static, lazy_static) | Untestable, no per-request scope, leaks between tests | [ai/KNOWN_PITFALLS.md §Loco](../ai/KNOWN_PITFALLS.md) |

---

## 2. Code Quality (Rust)

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 2.1 | `Result<T, Error>` for error handling | `unwrap()` / `expect()` in production code | Panic crashes the entire server | [standards/coding.md §2](coding.md) |
| 2.2 | `thiserror` + typed error hierarchy | `anyhow` in library crates, `String` errors | Loses typing, cannot match on error | [standards/errors.md](errors.md) |
| 2.3 | Newtype pattern (`TenantId(Uuid)`, `UserId(Uuid)`) | Bare `Uuid` / `String` for IDs | Can confuse user_id and tenant_id at type level | [standards/coding.md §1.2](coding.md) |
| 2.4 | State machine via enum + transition methods | String-based status with if/else checks | No compile-time guarantees for valid transitions | [guides/state-machine.md](../guides/state-machine.md) |
| 2.5 | `tokio::select!` with cancellation safety | `tokio::spawn` without join + without cleanup | Task leaks, unclosed resources | [standards/coding.md §3.1](coding.md) |
| 2.6 | `Semaphore` for limiting concurrency | Unlimited `tokio::spawn` in a loop | Thousands of tasks, OOM, resource exhaustion | [standards/coding.md §3.2](coding.md) |
| 2.7 | `Cow<'_, str>` to avoid unnecessary clones | `.to_string()` / `.clone()` everywhere | Unnecessary allocations, latency | [standards/coding.md §4](coding.md) |
| 2.8 | Function < 20 lines, module < 500 lines | Functions > 40 lines, modules > 1000 lines | Complexity, unreadability, difficulty testing | [standards/coding.md §9.1](coding.md) |
| 2.9 | < 4 function arguments (or struct for params) | > 6 function arguments | Hard to read, easy to confuse arguments | [standards/coding.md §9.1](coding.md) |
| 2.10 | `#[instrument]` on service methods | No tracing in service methods | Cannot trace request flow | [standards/logging.md](logging.md) |
| 2.11 | Depend on trait objects (`Arc<dyn Repository>`) | Depend on concrete types (`PgOrderRepository`) | Cannot substitute for testing | [standards/coding.md §1.1 (DI)](coding.md) |
| 2.12 | `const` for compile-time known values | `fn get_constant() -> T` for values known at compile time | Runtime overhead without reason | [standards/coding.md §1.3](coding.md) |

---

## 3. Data and DB

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 3.1 | **Always** `WHERE tenant_id = ?` in every query | SELECT/UPDATE/DELETE without tenant_id filter | **Critical vulnerability**: cross-tenant data leak | [architecture/tenancy.md](../architecture/tenancy.md) |
| 3.2 | Parameterized queries via SeaORM | String concatenation for SQL | SQL injection | [standards/security.md](security.md) |
| 3.3 | Migrations via `RusToKModule::migrations()` | Manual SQL scripts bypassing migration system | Schema desync between environments | [architecture/principles.md](../architecture/principles.md) |
| 3.4 | Naming: `mYYYYMMDD_<module>_<nnn>_<description>` | Arbitrary migration names | Breaks execution order, conflicts | [architecture/principles.md](../architecture/principles.md) |
| 3.5 | Separate DTO (Input/Response) vs Entity | DB Entity returned directly in API | Internal field leak, coupling between API and schema | [architecture/api.md](../architecture/api.md) |
| 3.6 | Transaction for write + event (`publish_in_tx`) | Separate write and separate publish | Event goes out but write rolled back (or vice versa) | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 3.7 | SeaORM entities with `#[derive(DeriveEntityModel)]` | Manual SQL strings for CRUD | No type safety, manual mapping, errors | — |
| 3.8 | Soft delete (status = Archived) for business entities | Hard DELETE for products/orders/nodes | Loss of audit history, broken references | — |
| 3.9 | Index tables (`index_products`, `index_content`) for read path | Join 5+ tables for storefront queries | Slow read queries, load on write DB | [architecture/overview.md §CQRS](../architecture/overview.md) |

---

## 4. Event System

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 4.1 | `publish_in_tx()` — event in the same transaction as write | `publish()` (fire-and-forget) for business events | Event can go out when transaction rolls back | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 4.2 | `transport = "outbox"` for production | `transport = "memory"` in production | Event loss on restart, no guarantees | [references/outbox/README.md](../references/outbox/README.md) |
| 4.3 | Outbox relay worker is running | Production without relay worker | Events permanently stuck in `sys_events` | [ai/KNOWN_PITFALLS.md §Outbox](../ai/KNOWN_PITFALLS.md) |
| 4.4 | `DomainEvent` with `tenant_id` in payload | Events without tenant_id | Index cannot determine which tenant the event belongs to | — |
| 4.5 | Idempotent event handlers | Event handler without idempotency check | Data duplication on retry/replay | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 4.6 | Event versioning with backward compatibility | Breaking changes in event payload | Old consumers break | — |
| 4.7 | Use `IggyConfig`/`ConnectorConfig` from code | Invent Iggy configuration | Incompatible parameters, connection errors | [ai/KNOWN_PITFALLS.md §Iggy](../ai/KNOWN_PITFALLS.md) |
| 4.8 | DLQ for failed events + admin replay endpoint | Silent drop of failed events | Data loss without recovery possibility | [architecture/events.md](../architecture/events.md) |

---

## 5. Auth and RBAC

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 5.1 | Permission extractors (`RequireProductsCreate(user)`) | No RBAC check in handler | Any auth user can do anything | [architecture/rbac.md](../architecture/rbac.md) |
| 5.2 | `AuthLifecycleService` for auth business logic | Duplicating auth logic in REST and GraphQL controllers | Behavior desync between transport layers | [architecture/api.md](../architecture/api.md) |
| 5.3 | `SecurityContext` with `get_scope()` in services | Data filtering only at controller level | Customer sees others' orders in list queries | [architecture/rbac.md §SecurityContext](../architecture/rbac.md) |
| 5.4 | JWT secret via env var (`JWT_SECRET`) | Hardcoded JWT secret in code | Compromise of all tokens | [standards/security.md](security.md) |
| 5.5 | Argon2 for password hashing | MD5/SHA256/bcrypt for passwords | Argon2 is the standard, resistant to GPU/ASIC | — |
| 5.6 | Token invalidation on change-password | Old tokens remain valid after password change | Compromised token continues to work | — |
| 5.7 | Public endpoints explicitly marked (health, login, storefront queries) | Endpoint without auth "by default" | Accidental data exposure | [architecture/rbac.md](../architecture/rbac.md) |

---

## 6. Multi-Tenancy

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 6.1 | `TenantContext` extractor in every handler | Handler without tenant resolution | All tenants' data mixed | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.2 | `tenant_id` field in **all** domain tables | Tables without tenant_id | Cannot isolate data | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.3 | Negative cache for non-existent tenants (TTL 60s) | Every request with invalid tenant hits DB | DoS via non-existent tenants | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.4 | Singleflight for cache miss (one DB query) | Each concurrent request makes its own DB query | Cache stampede on cold start | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.5 | Redis pub/sub for cross-instance invalidation | Only local cache invalidation | Stale data on other instances | [architecture/tenancy.md](../architecture/tenancy.md) |
| 6.6 | `validate_registry_vs_manifest()` at startup | Manifest and registry desynchronized | Module declared in manifest but not registered (or vice versa) | [modules/manifest.md](../modules/manifest.md) |

---

## 7. API

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 7.1 | GraphQL for UI clients (admin, storefront) | REST for everything | Over-fetching, N+1 queries, many endpoints | [architecture/api.md](../architecture/api.md) |
| 7.2 | REST for integrations, webhooks, batch jobs | GraphQL for machine-to-machine | Parsing complexity, caching, SDK generation | [architecture/api.md](../architecture/api.md) |
| 7.3 | DataLoaders for N+1 prevention | Inline DB queries in resolvers | O(n) queries instead of O(1) batched | [architecture/dataloader.md](../architecture/dataloader.md) |
| 7.4 | `#[utoipa::path(...)]` for OpenAPI | REST endpoints without OpenAPI annotations | Swagger UI doesn't show endpoint | — |
| 7.5 | `validator` crate for input validation on DTOs | Manual if/else checks in handlers | Inconsistent validation, missed fields | [guides/input-validation.md](../guides/input-validation.md) |
| 7.6 | GraphQL error extensions for structured errors | Plain string errors in GraphQL | Client cannot programmatically handle error | [standards/errors.md](errors.md) |
| 7.7 | Pagination in list queries (limit/offset or cursor) | List without pagination | Loading entire table into memory, OOM | — |
| 7.8 | `MergedObject` for modular GraphQL schema | Single monolithic Query/Mutation type | Coupling, cannot disable module | [architecture/api.md](../architecture/api.md) |

---

## 8. Frontend

### 8.1 Leptos (Admin / Storefront)

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 8.1.1 | `leptos-graphql` for GraphQL queries | Manual fetch + manual JSON parsing | No typing, manual error handling | — |
| 8.1.2 | `leptos-auth` for auth state management | Manual JWT management in localStorage | Race conditions, no refresh logic | — |
| 8.1.3 | `leptos-zustand` for global state | Props drilling through 5+ levels | Unreadable code, component recreation | — |
| 8.1.4 | `leptos-hook-form` for forms | Manual form state + onChange handlers | Boilerplate, no validation | — |
| 8.1.5 | `iu-leptos` components from design system | Custom components with own styles | Visual inconsistency | — |
| 8.1.6 | SSR for storefront (SEO) | CSR-only storefront | No SEO, slow First Contentful Paint | — |
| 8.1.7 | CSR for admin panel (WASM) | SSR for admin panel | Admin doesn't need SEO, CSR is simpler | — |

### 8.2 Next.js (Admin / Frontend)

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 8.2.1 | Packages from `packages/` (leptos-auth, leptos-graphql, etc.) | Code duplication between next-admin and next-frontend | Copy-paste, desync | — |
| 8.2.2 | TypeScript strict mode | `any` types and `@ts-ignore` | Loss of type safety, runtime errors | — |
| 8.2.3 | Server Components for data fetching (Next.js 13+) | `useEffect` + fetch in every component | Waterfalls, no streaming | — |
| 8.2.4 | Clerk auth (next-admin) integrated with server JWT | Separate auth systems on frontend and backend | Session desync | — |

---

## 9. Testing

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 9.1 | Three-level pyramid: unit → integration → contract | Only unit tests or only E2E | Coverage gaps, slow feedback loop | [guides/testing.md](../guides/testing.md) |
| 9.2 | Polling with timeout instead of `sleep()` | `tokio::time::sleep(Duration::from_secs(1))` for waiting async | Flaky tests, false failures | [guides/testing.md](../guides/testing.md) |
| 9.3 | Transaction rollback for DB isolation in tests | Shared DB without cleanup | Tests depend on order, flaky | [guides/testing.md](../guides/testing.md) |
| 9.4 | Mock **ports** (traits), not persistence layer | Mock SeaORM models directly | False confidence — real queries not tested | [guides/testing.md](../guides/testing.md) |
| 9.5 | Property tests for state machines | Only happy-path tests for transitions | Missed invalid transitions | [guides/testing-property.md](../guides/testing-property.md) |
| 9.6 | Integration test for every new `DomainEvent` | Event without tests | Event is published but handler doesn't process | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 9.7 | Idempotency test for event handlers | Only happy-path event test | Data duplication on retry | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 9.8 | `rustok-test-utils` for shared fixtures | Copying test helpers between crates | Desync, duplication | — |

---

## 10. Observability

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 10.1 | `#[instrument(skip(self, input))]` on service methods | No spans in services | Cannot trace request → service → DB | [standards/logging.md](logging.md) |
| 10.2 | Structured fields (`%tenant_id`, `%user_id`) | String formatting (`format!("user={}", id)`) | Cannot filter in Grafana/Loki | [standards/logging.md](logging.md) |
| 10.3 | Single Prometheus registry | Different registries in different modules | Metrics not exported or duplicated | [ai/KNOWN_PITFALLS.md §Telemetry](../ai/KNOWN_PITFALLS.md) |
| 10.4 | One telemetry runtime initialization | Multiple telemetry initializations | Panic, span duplication, memory leak | [ai/KNOWN_PITFALLS.md §Telemetry](../ai/KNOWN_PITFALLS.md) |
| 10.5 | Info level for business events, Error for failures | `tracing::error!` for everything | Alert fatigue, cannot separate important | [standards/logging.md](logging.md) |
| 10.6 | Do NOT log PII and secrets | Logging email, password, tokens | GDPR violation, security breach | [standards/logging.md](logging.md) |

---

## 11. Security

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 11.1 | HTTPS in production | HTTP without TLS | Man-in-the-middle, token interception | [standards/security.md](security.md) |
| 11.2 | CSP, X-Frame-Options, HSTS headers | Without security headers | XSS, clickjacking | [standards/security.md](security.md) |
| 11.3 | Rate limiting on auth endpoints | Without rate limiting | Brute-force password attacks | [standards/security.md](security.md) |
| 11.4 | SSRF allowlist for external URLs | Without URL destination checking | SSRF → access to internal services | [standards/security.md](security.md) |
| 11.5 | `Zeroize` for sensitive data in memory | Sensitive data remains in memory after use | Memory dump → secret leak | [standards/coding.md §8.2](coding.md) |
| 11.6 | Secrets in env vars, not in code | Hardcoded secrets/passwords/keys | Leak on repo compromise | [standards/security.md](security.md) |

---

## 12. DevOps and CI/CD

| # | ✅ Correct | ❌ Incorrect | Why | Details |
|---|-------------|---------------|--------|--------|
| 12.1 | `cargo fmt --all && cargo clippy -- -D warnings` before commit | Commit without formatting/linting | Noisy diffs, hidden issues | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.2 | Conventional commits (`feat:`, `fix:`, `docs:`) | Arbitrary commit messages | Cannot auto-generate CHANGELOG | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.3 | Branch naming: `feature/`, `fix/`, `docs/` | Arbitrary branch names | Confusion, no automation | [CONTRIBUTING.md](../../CONTRIBUTING.md) |
| 12.4 | `cargo deny check` in CI | Without license and advisory checking | Vulnerable dependencies, license violations | — |
| 12.5 | Don't edit CI/CD files without explicit request | Automatic modification of workflow files | Broken CI for everyone | [AGENTS.md](../../AGENTS.md) |
| 12.6 | Update docs when changing code | Change code without updating documentation | Documentation lies, new developers get confused | [AGENTS.md](../../AGENTS.md) |

---

## Related Documents

- [Forbidden Actions (NEVER DO)](./forbidden-actions.md) — hard prohibitions with consequences
- [Code Standards](./coding.md) — detailed guide with examples
- [Error Handling](./errors.md) — error handling patterns
- [Security](./security.md) — OWASP coverage
- [Logging](./logging.md) — structured logging
- [Known Pitfalls](../ai/KNOWN_PITFALLS.md) — traps for AI agents
- [Architecture Principles](../architecture/principles.md) — architectural principles
- [Module Dependency Graph](../architecture/diagram.md) — 12 Mermaid diagrams (including dependency graph)
- [Module Categories A/B/C](../architecture/modules.md) — compile-time vs runtime vs optional
- [Component Registry](../modules/registry.md) — catalog of all crates, apps, packages
