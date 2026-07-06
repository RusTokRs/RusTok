# Implementation plan for `rustok-taxonomy`

Status: shared dictionary baseline is already working; the module is used by several
domains and is maintained as a vocabulary layer without capturing attachment ownership.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- maintain `rustok-taxonomy` as a shared vocabulary module;
- synchronize dictionary contracts, scope rules and local docs;
- do not let taxonomy turn into shared product storage.

## Current state

- term dictionary, translations and aliases are already implemented as module-owned storage;
- term identity remains tenant-scoped and locale-independent;
- blog, forum, product and profiles already use taxonomy-backed relations through their own attachment tables;
- locale normalization and fallback already rely on the shared content contract.

## Stages

### 1. Contract stability

- [x] lock dictionary baseline for `kind = tag`;
- [x] maintain scope model `global | module`;
- [x] introduce taxonomy-backed relations in the first consumer modules;
- [ ] maintain sync between dictionary contracts, consumer integrations and module metadata.

### 2. Expansion

- [ ] expand kinds and lookup semantics only when there is real domain pressure;
- [ ] add new consumer modules only through explicit module-owned attachment tables;
- [ ] keep alias/slug uniqueness and locale fallback guarantees covered by targeted tests.

### 3. Operability

- [ ] document new dictionary guarantees concurrently with runtime surface changes;
- [ ] develop runbooks for dictionary drift and integration incidents as pressure arises;
- [ ] synchronize local docs, README and central references when module role changes.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- targeted tests for CRUD, alias lookup, scope restrictions and consumer-module sync

## Update rules

1. When changing taxonomy contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing consumer-module integration rules, update related docs of owning modules.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
