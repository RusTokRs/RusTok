# Canonical event contract in `rustok-events` (Phase 2/3)

- Date: 2026-02-19
- Status: Proposed

## Context

Currently, the canonical `DomainEvent` definition remains in `rustok-core`, while `rustok-events` works as a compatible re-export layer. This leaves tight coupling: adding or changing a domain event payload requires modifying the core crate, which violates Open/Closed and increases the blast radius for platform releases.

At the same time, there is an already accepted Phase 1 step: the import point for event contracts has been aligned to `rustok-events`, so the migration can be completed without a one-time big-bang.

## Decision

1. Make `rustok-events` the canonical source of `DomainEvent` and payload schemas (Phase 2).
2. Keep a temporary compatibility-layer in `rustok-core` (re-export + deprecation note) until the next release train.
3. In the next breaking phase, remove the legacy re-export in `rustok-core` and migrate all imports to `rustok-events` (Phase 3).
4. Finalize the migration checklist:
   - update imports in all `rustok-*` modules;
   - update event schema snapshots/code generation;
   - verify serialization backward compatibility (`event_type`, `schema_version`).

## Consequences

**Positives**
- New domain events can evolve without changing `rustok-core`.
- Reduced coupling between the platform foundation and domain crates.
- A clear responsibility boundary for event contracts emerges.

**Risks and negatives**
- Requires coordinated migration of imports and tests across all modules.
- Potential breaking impact for external integrations importing events from `rustok-core`.

**Follow-up**
- Prepare a separate PR for Phase 2 (canonical move + compatibility).
- Prepare a separate PR for Phase 3 (remove legacy layer) after communicating the breaking change.
