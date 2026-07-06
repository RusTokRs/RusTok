# Loco Mailer and Storage as server-infra layer (without extracting into a separate module)

- Date: 2026-03-11
- Status: Accepted

## Context

The RusToK server already has an anti-duplication matrix for Loco vs custom implementation (`apps/server/docs/LOCO_FEATURE_SUPPORT.md`).
For two areas, there remains a high risk of infrastructure duplication:

- Mailer subsystem;
- Storage abstraction.

In parallel, the project follows a modular architecture (`ModuleRegistry`, `ModuleKind::Core/Optional`), where domain modules `crates/rustok-*` solve domain-specific problems and should not become a layer of infrastructure wrappers around framework capabilities.

The boundary needs to be fixed: should Mailer/Storage become separate platform modules or remain part of server infrastructure.

## Decision

1. **Mailer and Storage are established as an infrastructure layer of `apps/server`, built on the Loco API**:
   - Mailer: Loco Mailer API as the primary integration contract;
   - Storage: Loco Storage abstraction as the unified upload/assets contract.
2. **Do not create separate platform modules `crates/rustok-*` for Mailer/Storage**.
3. Domain modules use Mailer/Storage through server-level adapters/policies (unified integration points), without their own duplicating infra implementation.
4. Deviations from this rule require a separate ADR with a trade-off justification and migration plan.

## Consequences

### Positives

- Removes the risk of parallel implementations of the same infra layer.
- Maintains clean boundaries: domain modules = domain logic, `apps/server` = runtime/infrastructure.
- Easier to maintain compatibility with upstream Loco.

### Trade-offs

- The center of gravity for infra changes remains in `apps/server`.
- Discipline in API boundaries (adapters/policies) is needed to prevent ad-hoc calls from the domain.

### Follow-up

1. Migrate the current password-reset delivery to the Loco Mailer API.
2. Introduce a unified storage adapter/policy for modular upload/use-cases.
3. Update the anti-duplication matrix and server docs when Mailer/Storage changes occur.
