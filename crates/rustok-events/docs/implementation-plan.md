# Implementation plan for `rustok-events`

Status: canonical ownership event contracts have already been moved to a separate module;
current work is to maintain the compatibility path and schema discipline without drift.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: Update this block after each increment.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- keep `rustok-events` as the only canonical source for event contracts;
- synchronize schema registry, envelope shape and local docs;
- do not return ownership event contracts back to `rustok-core`.

## Current state

- `DomainEvent` and `EventEnvelope` already live in `rustok-events`;
- `rustok-core::events` already works as a compatibility re-export layer;
- internal `rustok-*` crates should already import event contracts directly from `rustok-events`;
- schema coverage, versioning guidance and contract tests already form the base release gate.

## Stages

### 1. Contract stability

- [x] move canonical ownership event contracts to a separate crate;
- [x] preserve compatibility path through `rustok-core::events`;
- [x] cover schema registry, validation and roundtrip contract tests;
- [ ] maintain sync between event types, registry and consumer imports (tenant lifecycle family updated: added `tenant.module.toggled`).

### 2. Release discipline

- [ ] bring documented release gate to a sustainable process around schema changes;
- [ ] continue cleaning residual direct imports from compatibility path;
- [ ] document breaking/deprecating changes together with versioning plan.

### 3. Operability

- [ ] keep outbox/replay/reindex guidance synchronized with event contracts;
- [ ] synchronize local docs and `README.md` when schema surface changes;
- [ ] expand compatibility checks as new event families appear.

## Verification

<!-- compatibility anchor: contract tests cover all public use-cases -->
- [ ] Contract tests cover public event-contract use cases.
- `cargo xtask module validate events`
- `cargo xtask module test events`
- targeted tests for schema coverage, validation, compatibility aliases and JSON roundtrip

## Update rules

1. When changing event contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing event versioning policy, update related architecture/outbox docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
