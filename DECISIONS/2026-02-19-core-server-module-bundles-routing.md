# Auto-registration of HTTP routes and `core-server` / `module-bundles` split

- Date: 2026-02-19
- Status: Proposed

## Context

`apps/server/src/app.rs` contains a centralized manual assembly of routes across all domain modules. When connecting a new module, the central server layer must be modified, creating coupling and impairing the scalability of the modular architecture.

In the architecture improvements document, this item is recorded as strategic (2.14), but without an ADR the implementation should not start.

## Decision

1. Introduce a modular HTTP contract (e.g., `HttpModule`) on top of `RusToKModule`, which exposes the module's routes.
2. Convert `apps/server` to a two-tier model:
   - `core-server`: base system routes and infrastructure;
   - `module-bundles`: pluggable HTTP modules from the registry.
3. Routes of optional modules are connected automatically from the registry/bundle layer without manual modification of the central `app.rs`.
4. Introduce migration guardrails:
   - feature flag for phased rollout;
   - parity tests to match existing routes before/after migration;
   - rollback plan for one release.

## Consequences

**Positives**
- A new module can be added without changing the central routing glue.
- Clearer separation between platform foundation and domain extensions.
- Improved testability and ownership boundaries.

**Risks and negatives**
- Migration of the current route wiring is required, and the bootstrap sequence may need restructuring.
- Startup orchestration becomes more complex during the initial implementation phase.

**Follow-up**
- Prepare a technical design with an example `HttpModule` contract.
- Add parity/integration routing tests before migrating the production configuration.
