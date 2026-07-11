# Implementation plan for `rustok-modules`

## Scope

Own the mandatory module artifact control plane and marketplace without making
the server know optional domain crates.

## Current State

The Core entry point and immutable artifact descriptor/lineage contract are
implemented. Persistence, OCI registry integration, installation operations and
owner transports remain to be moved from the server.

## Milestones

1. Move manifest, composition, governance and tenant lifecycle services/models
   from `apps/server` into this Core module.
2. Persist platform installation, capability-grant, migration and rollback state.
3. Resolve and verify OCI artifacts, then activate them through `rustok-sandbox`.
4. Make Alloy publish, fork and evolve Rhai artifact releases through the same
   immutable descriptor contract.
5. Replace server Cargo module features with static distribution promotion.

## Verification

- Artifact descriptor, executor-selection and lineage contract tests.
- Registry signature/dependency/install/rollback integration tests.
- Tenant isolation and GraphQL/native transport parity tests.

## Update Rules

Update this plan, module registry and central control-plane plan whenever
artifact identity, lifecycle, marketplace governance or sandbox admission changes.

