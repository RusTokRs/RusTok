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
work must not depend on Loco runtime APIs.

