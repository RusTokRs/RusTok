# `rustok-api` as a thin and unified shared host/API layer

- Date: 2026-03-23
- Status: Accepted

## Context

RusToK is moving towards a platform model where optional modules can be installed and updated as standalone packages, and `apps/server` should not know module-specific details about them. For this, a stable shared contract between the runtime host and module web adapters is needed:

- request/auth/tenant context;
- GraphQL helpers and error contract;
- request-level locale/tenant resolution;
- host-facing transport helpers that are not related to domain logic.

Such a layer has already appeared as `crates/rustok-api` and is actually used by both the server and a number of module crates. At the same time, the risk of architectural drift remains:

1. moving shared transport/helper types back into `apps/server`;
2. starting to create parallel helper-layer crates next to individual modules;
3. turning `rustok-api` into "yet another server" by pulling in module-specific resolvers, controllers, and domain behavior.

The boundary must be fixed explicitly to preserve the third-party module model and avoid having multiple incompatible implementations of the same host contract.

This decision is compatible with already accepted boundaries:

- `apps/server` remains the composition root and server-infra/runtime host;
- module-specific transport code lives in module crates;
- infrastructure-level capabilities are not extracted without a separate ADR into new platform modules.

## Decision

1. `rustok-api` is established as the **thin and only shared host/API layer** for RusToK.
2. `rustok-api` may only contain the common host-level contract that is actually reused between `apps/server` and module crates:
   - `AuthContext`, `TenantContext`, `RequestContext`;
   - GraphQL helpers, pagination/error contract, module-enabled guard;
   - locale/tenant/request extraction primitives;
   - minimal transport/runtime helpers without domain logic.
3. `rustok-api` **must not** contain:
   - module-specific resolvers and controllers;
   - module-specific business logic;
   - module manifests, module settings schema, UI policy, registry specifics of a particular module;
   - composition-root wiring at the `apps/server` level.
4. `apps/server` may include and re-export `rustok-api`, but must not develop a second parallel implementation of the same shared host/API layer.
5. Module crates may depend on `rustok-api` for the shared host contract, but their own transport/domain code remains local to the module.
6. A new helper may enter `rustok-api` only if it:
   - is genuinely shared between at least several modules or between a module and the server;
   - belongs to the host/API boundary, not to the domain behavior of a specific module.
7. Any attempt to:
   - expand `rustok-api` into a platform business layer;
   - introduce a second shared API/helper layer;
   - override these boundaries,
   requires a separate ADR.

## Consequences

### Positives

- A single contract surface for third-party modules is preserved.
- `apps/server` does not degrade back into a place where shared transport/helper types are scattered.
- The risk of multiple incompatible shared-layer implementations is reduced.
- It is easier to move towards a WordPress-like install/uninstall model from admin, where the server knows the generic host contract rather than module details.

### Trade-offs

- Discipline is required during code review: not every helper should end up in `rustok-api`.
- Some temporary shim/re-export layers in `apps/server` still remain until the next stage of codegen/composition cleanup.
- This decision itself does not remove the manual composition root assembly; it only fixes the boundary of the shared host/API layer.

### Follow-up

1. Use `rustok-api` as the canonical entry point for new shared host/API helpers.
2. Do not add new parallel helper crates with the same role.
3. Continue extracting module-specific GraphQL/REST adapters into module crates.
4. Separately drive codegen/composition-root automation so that `apps/server` no longer manually knows about optional modules.
