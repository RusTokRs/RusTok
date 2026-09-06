# rustok-workflow — API overview

> This document is a **curated overview**, not a manual snapshot of signatures.
> The canonical reference comes from code and generated artifacts.

## Source of truth

- Crate source: `crates/rustok-workflow/src/**`
- Rustdoc (locally):
  - `cargo doc -p rustok-workflow --no-deps`
- Server runtime wiring:
  - `apps/server/src/modules/workflow.rs`

## Module registration contract

`WorkflowModule` registers as an optional module in the module registry and publishes:

- module slug/name/kind;
- workflow bounded context migrations;
- permissions for workflow authoring/execution read paths.

For exact values (slug, permissions, migration list), use the code,
not duplication in markdown.

## Public surface map

The crate's public API is divided into the following areas:

- `controllers` — REST endpoints and route wiring;
- `graphql` — query/mutation surface for workflow authoring and reads;
- `services` — domain orchestration layer (`WorkflowService`, execution/use-case logic);
- `steps` — runtime step contracts and registries;
- `templates` — builtin workflow templates and metadata;
- `dto`/`entities`/`error` — transport contracts, storage model and domain errors.

## Domain invariants (must-hold)

- All write/read operations are tenant-scoped.
- Workflow execution path is separated from the authoring path.
- Step execution goes through a typed step runtime (`WorkflowStep` contract).
- Trigger paths (manual/webhook/event/cron) converge into a single execution orchestration.
- Execution and configuration errors must be returned as typed domain errors,
  without silent fallback.

## Execution model

- `WorkflowService` is responsible for CRUD and orchestration use-cases.
- `WorkflowEngine` executes steps and manages step registry/runtime dispatch.
- `WorkflowTriggerHandler` integrates the event-driven trigger path.
- `WorkflowCronScheduler` covers the schedule-driven trigger path.

Important: for actual methods/signatures, refer to the source code and rustdoc.
This document captures roles and boundaries, not hand-written API.

## Extensibility contract

Custom steps are connected via the `WorkflowStep` trait and registration
in the engine runtime (`with_step(...)`).

Requirements for steps:

- deterministic behavior given the same input context;
- correct error handling via `WorkflowResult`;
- no tenant-boundary bypass;
- no hidden side effects outside the step contract.

## Transport entry points

- GraphQL: workflow query/mutation roots.
- REST: workflow controllers/routes, including webhook trigger path.

For exact route/query names and payload contracts, see source + generated schema.

## Documentation maintenance rule

When changing workflow transport contract, execution semantics, or error model:

1. Update this overview (roles, invariants, boundaries).
2. Do not duplicate hand-written signatures in markdown.
3. Add/update links to generated reference in relevant docs.

## Hotspot contract (DOC-12 / H4)

- Hotspot: `H4` (Workflow/Public API contracts).
- Doc contracts updated: `crates/rustok-workflow/CRATE_API.md`.
- Owner scope: workflow module owner.
- Residual drift risk:
  - until DOC-09 (B12 CI artifacts) is closed, there may be a gap between curated overview
    and actual exported reference artifacts in a PR;
  - when changing transport payload shapes without updating generated references,
    risk remains high.
