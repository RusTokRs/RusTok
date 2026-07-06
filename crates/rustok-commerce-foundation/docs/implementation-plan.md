# Implementation plan for `rustok-commerce-foundation`

Status: support crate already serves as shared substrate for the split commerce family;
the key task is to keep it minimal and prevent rebuilding
the monolith in the foundation layer.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- keep `rustok-commerce-foundation` as a dependency-only support crate;
- synchronize shared DTO/entities/error contracts and local docs;
- prevent moving domain/runtime logic from split commerce modules into the foundation layer.

## Current state

- crate already contains shared DTOs, entities, errors and search/query helpers;
- consumer modules already use it as a common reuse layer;
- umbrella `rustok-commerce` relies on this crate for common contracts of the split family;
- the crate has no standalone transport/runtime surface and should not acquire one.

## Stages

### 1. Contract stability

- [x] lock foundation crate as a common dependency layer for commerce family;
- [x] keep shared error/entity/DTO surface unified for consumer crates;
- [ ] maintain sync between foundation contracts, consumer crates and local docs.

### 2. Boundary hardening

- [ ] move only truly shared contracts here;
- [ ] do not pull domain-owned services and orchestration logic here;
- [ ] cover incompatible changes with targeted compile/tests in consumer crates.

### 3. Operability

- [ ] document foundation surface changes simultaneously with changing consumer expectations;
- [ ] keep local docs and `README.md` synchronized;
- [ ] update umbrella commerce docs when split-family contracts change.

## Verification

- structural verification for docs and shared boundary;
- targeted compile/tests when DTO/entity/error surface changes;
- consumer sync across split commerce crates.

## Update rules

1. When changing shared commerce foundation contract, update this file first.
2. When changing public surface, synchronize `README.md` and `docs/README.md`.
3. When changing consumer expectations, update related docs in split commerce crates.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
