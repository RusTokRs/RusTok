# Auth-owned shared client FFA boundary

- Date: 2026-11-13
- Decision status: Accepted
- Implementation status: In progress
- Owners: Auth module / Leptos host composition
- Extends: [HttpOnly session migration](./2026-09-27-leptos-httponly-session-migration.md)
- Supersedes: frontend package ownership only in the HttpOnly session decision
- Superseded by: None

## Context

The platform has moved domain UI behavior to module-owned FFA packages. A generic
`leptos-auth` library nevertheless owned Auth's shared client lifecycle, while
Auth's admin package and the storefront both consumed it. Moving that code into
admin would couple storefront to admin; importing the server-heavy Auth backend
into WASM would couple browser code to storage and runtime implementation.

## Decision

Auth owns one browser-capable shared client package, `rustok-auth-ui`, under
`crates/modules/rustok-auth/ui`. It exposes framework-independent `core` and
`model`, a selected `transport` facade, and `ui::leptos` bindings. Auth admin
owns its pages and locales separately. Both hosts and other module packages use
the same Auth client boundary. This package does not create a storefront route.
The old library has no implementation, package or compatibility alias.

## Sources of truth and ownership

The Auth backend remains authoritative for credential and session validity;
RBAC and the relevant owner enforce authorization. `rustok-ui-auth` remains the
single framework-neutral client value/expiry library, used through the Auth UI
model. The Leptos provider holds a transient projection, not an authority.
The shared UI package does not depend on the server-heavy Auth backend or either
host/admin package. Hosts compose validated request snapshots and enable the
correct CSR, hydrate or SSR features.

## Invariants

- There is one Auth client provider implementation and one shared value contract.
- Cookie/session wire names and native and GraphQL/cookie paths are preserved.
- A client scope or unverified JWT payload is never server authorization.
- The prior HttpOnly, CSRF, and production cookie-security targets remain in force.
  HttpOnly alone neither prevents XSS actions nor makes browser access tokens secret
  from scripts; deployment behavior must be verified separately.

### Allowed states

A verified session or explicitly established anonymous scope can be projected to
UI consumers. Failed or pending transitions remain unverified; retained identity
may be used only for explicit recovery.

### Forbidden states

Do not introduce parallel providers, duplicate Auth models, a legacy library alias,
server-side policy in the browser, or a storefront dependency on admin. Never
interpret an ambiguous cookie or transport failure as proof of a guest identity.

## Non-goals

This decision does not certify browser/service behavior, add a storefront login
page, change the Next.js implementation, or redefine server authorization.

## Data, transaction, and concurrency boundary

No new database or persisted identity format is introduced. The existing
same-document lease coordinates cooperating providers only; it cannot cancel
already-sent requests, serialize other tabs, or constrain unrelated scripts.
The Auth provider does not yet subscribe to external invalidation notifications:
Cart's bridge invalidates its own state, but other provider projections may stay
stale across roots or tabs. Route guards and some storage-backed transports also
bypass ready-scope getters. These are open implementation gaps, not guarantees of
this decision. The Auth backend retains session rotation and revocation transaction ownership.

## Context dimensions

Tenant and authenticated principal remain authoritative at the server. Hosts
supply their resolved public tenant and validated request snapshot; the browser
must not silently borrow another tenant. Channel and locale are not introduced
by this client lifecycle boundary.

## Events and projections

Non-secret browser notifications let cooperating consumers invalidate projections;
Cart's bridge consumes them, but Auth providers do not yet do so. They are neither
durable outbox events nor cross-tab locks. In-memory user/session projections and
scope stamps cannot prove server state or revoke a request already sent.

## Failure semantics

Malformed, foreign, duplicate, rejected or uncertain credentials fail closed;
logout/refresh uncertainty remains retryable and is not silently confirmed.
The direct-cookie endpoint's absence/rejection ambiguity and redirected browser
fetch limitations remain explicit verification and product gates.

## Migration and cutover

Move the existing implementation and tests into the owner package, update all
consumers and mode features together, and retain only a retirement description
for the old package. Existing wire names and stored tenant hints are not renamed.
The frontend package placement of the earlier ADR is superseded; its session and
security requirements are not. No compatibility path remains.

## Alternatives considered

Retaining a generic framework library obscures the domain owner. Re-exporting it
from Auth preserves the same split ownership. Putting the provider in admin makes
storefront depend on an unrelated surface. Linking the server backend into WASM
introduces server/storage dependencies. All are rejected.

## Verification

Locked native Auth default/SSR tests, the public contract and the Auth admin SSR
check passed on cutover commit `5ce7c6c` in [CI run 43575756113](https://github.com/RusTokRs/RusTok/actions/runs/43575756113).
Auth and Auth admin CSR WASM and the Product buyer island with Auth hydrate also
passed. Both full host builds, the standalone hydrate graph fence and the new SSR
rendering contract still need results. Exercise mounted provider, SSR hydration, native and
direct-cookie login/refresh/logout, tenant mismatch, multiple providers, tabs,
dropped futures and real services before rollout. The Auth plan and Page Builder
backlog retain the outstanding gates.

## Consequences

Auth owns a reusable cross-surface capability without a parallel runtime. The
migration is a source/API cutover, so repository consumers must update together;
external source consumers require a coordinated update. Static guards and pure
state tests alone do not establish runtime parity or production readiness.
