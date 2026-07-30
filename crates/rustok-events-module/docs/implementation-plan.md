---
id: doc://crates/rustok-events-module/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-28
---

# Events runtime adapter implementation plan

## Scope

Keep the `events` module registration, manifest, and module-owned admin package
boundary cycle-free while the canonical event and delivery implementation
evolves in its owning crates.

## Current State

- `EventsModule` registers the required core module and declares its `outbox`
  dependency.
- The adapter intentionally owns no migrations and delegates concrete runtime
  readiness to the server event runtime.
- Canonical schemas and validation remain in `rustok-events`; transactional
  persistence remains in `rustok-outbox`.
- Leptos and Next admin packages are present, while live transport parity and
  production Iggy evidence remain in progress under the canonical Events plan.
- The Next package exposes a client-safe registration entrypoint separately
  from its server page and API exports. The package declares its host i18n
  dependency, while server translations are resolved by the host route and
  passed into the package boundary.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - module registration and manifest boundary are cycle-free;
  - module-owned Leptos and Next packages are colocated with the adapter;
  - the Next host imports only the client-safe registration entrypoint during
    shell composition, so server-only page exports cannot enter the client
    module graph;
  - canonical event/runtime verification remains tracked by `rustok-events`.
- Last verified at (UTC): 2026-07-29
- Owner: Events module maintainers

## Milestones

1. Keep adapter metadata and module-owned UI placement synchronized with the
   canonical Events capability.
2. Complete native/GraphQL/Next transport parity in the canonical Events plan.
3. Replace delegated health with composed live readiness only when host runtime
   context can be injected without reversing dependencies.

## Verification

- `cargo check -p rustok-events-module`
- `cargo test -p rustok-events-module`
- `cargo xtask module validate events`
- `cargo xtask validate-manifest`

## Update Rules

- Do not move canonical event schemas into this adapter.
- Do not move outbox or Iggy delivery persistence into this adapter.
- Keep local FFA/FBA status synchronized with the central readiness board.
- Track event-contract and delivery milestones in the canonical
  `rustok-events` plan rather than duplicating them here.
