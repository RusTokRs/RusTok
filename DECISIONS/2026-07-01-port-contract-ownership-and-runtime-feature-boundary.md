# Port contract ownership and runtime feature boundary

- Date: 2026-07-01
- Status: Accepted

## Context

`Port*`, permission, and locale contracts were defined in `rustok-core`, but
published to clients through `rustok-api`. Therefore, a transport-neutral consumer of the
API would pull in the entire core runtime, while the `server` feature maintained a reverse dependency
`rustok-api -> rustok-core`.

## Decision

1. `rustok-api` is the sole owner of `Port*`, `Permission`, `Action`,
   `Resource`, platform locale normalization/matching/candidates, and
   `Accept-Language` parsing.
2. `rustok-api` does not depend on `rustok-core` in the default, `runtime`, or `server` feature.
3. The dependency graph is directed only as `rustok-core -> rustok-api`.
4. `rustok-core` owns runtime policy: `UserRole`, `UserStatus`, `Rbac`,
   `PermissionScope`, `SecurityActorKind`, `SecurityContext`, and role inference.
   `SecurityContext::system()` is the trusted runtime authority, while anonymous
   storefront/GraphQL reads use `SecurityContext::public_read()`.
5. Core modules/re-exports and compatibility aliases for relocated contracts are removed.
6. The outbox-specific adapter belongs to `rustok-outbox`; `rustok-api` does
   not depend on `rustok-outbox`.
7. All module ports use the canonical path `rustok_api::ports::*` or
   root re-exports `rustok_api::*`.
8. The SeaORM-backed `HostRuntimeContext` is available through the neutral
   `runtime` feature. The `server` feature includes `runtime` and separately adds
   Axum and Async-GraphQL, so backend runtime helpers do not force transport
   frameworks into standalone module owners.
9. `rustok-rbac` owns persisted role-membership queries and permission decisions.
   Authenticated host snapshots are inputs through `PermissionResolver`; hosts
   select and bind exact tenant/actor request evidence and compose cache adapters.
   Single, any, and all decisions use the module tenant policy engine. A snapshot
   never establishes current persisted grants or an atomic revocation fence;
   destructive owner commands use the current transaction-backed decision API.

## Scope clarification (2026-11-06)

This decision places shared platform primitives in `rustok-api`. Module-specific ports,
DTOs and integration-event contracts remain owned by their domain module. They may live
with its implementation or in an optional owner contracts package. This ADR neither
requires one package per module nor requires a separate contracts package.

Runtime composition may construct local services or configured transport adapters. The
host's knowledge of deployment configuration, TLS and credentials is not a transfer of
domain ownership. A contracts-only owner build requires separate dependency verification;
it is not implied by the neutral shared API feature boundary established here.

## Consequences

Secret capability authority follows the same owner boundary. Its canonical
scope combines exact installation capability authority with the independently
persisted opaque secret-instance identity. Catalogs use tenant/owner/instance
keys; stateless installations need no artifact-data persistence contract.
The repository-owned `module.artifact.secret_bound` initial payload and schema
are replaced in place with installation/owner/instance identities. The publisher
is the owner binding service; repository search finds no other publisher or
payload reader requiring an adapter. No prior-shape reader or version family is
retained. Resolver aliases, keys, and values remain absent from the event.
Host current-grant, revocation, and operation-specific use fences still require
real composition and runtime evidence; identity alone is not such a fence.

- Clients of neutral/default `rustok-api` do not compile the core runtime.
- The dependency graph is directed from runtime modules to the contract layer without a cycle.
- Old core permission/locale/port paths are removed atomically and are not maintained via aliases.
- User/service port actors receive authority only after strict parsing of UUID,
  roles, and permission claims; system authority is allowed only for `PortActorKind::System`.
- The absence of `AuthContext` on a public read endpoint is no longer elevated to system
  authority: such requests receive `SecurityActorKind::Public` and pass only
  through public/published/channel-visible read paths.
- Consumers use the owner-provided outbox runtime contract rather than a
  framework-specific adapter.
- `rustok-runtime` and its module-owner consumers compile against
  `rustok-api/runtime` without Axum or Async-GraphQL in their normal dependency graph.
