# Backend Module Guides

This directory is the backend counterpart to the module UI package guides in
`docs/UI/`.

Read these documents before writing or refactoring module backend code:

- [Backend Module Architecture](./module-backend-architecture.md) - ownership, runtime
  boundaries, foundation crates and FBA/CLI split.
- [Backend Module Implementation](./module-backend-implementation.md) - practical crate
  layout, transport adapters, ports, runtime helpers and forbidden patterns.
- [Backend Module Verification](./module-backend-verification.md) - fast guardrails and
  targeted verification commands.

The short rule is: module domain logic belongs to the module, host wiring belongs to
`apps/server`, stable cross-boundary contracts belong to foundation crates, and new backend
work must use the explicit Axum runtime contracts.

The physical shape is also fixed:

- module domain/application code lives in `crates/rustok-<module>/src`;
- module evidence and generated/public contract artifacts live in `contracts/`;
- module-local plans and readiness evidence live in `docs/`;
- optional UI adapters live in `admin/` and `storefront/`;
- optional external command adapters live in module-local `cli/`;
- `apps/server` mounts routes and composes runtime state, but does not own module business
  rules or CLI providers.

Use the foundation crates by boundary: `rustok-api` for stable contracts,
`rustok-runtime` for executable runtime helpers, `rustok-web` for Axum boundary mapping,
`rustok-fba` for backend provider/consumer metadata and `rustok-cli-core` for command
provider contracts.
