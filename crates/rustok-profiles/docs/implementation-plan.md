# Implementation plan for `rustok-profiles`

Status: storage/service/GraphQL foundation is already up; the module is in
rollout hardening mode around profile summary, backfill and UI/read-model
further development.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- maintain `rustok-profiles` as a separate public profile domain;
- synchronize storage, summary contracts, GraphQL surfaces and local docs;
- prevent collapsing `profiles`, `users`, `customer` and future seller surfaces.

## Current state

- `ProfilesModule`, `rustok-module.toml` and permission surface `profiles:*` already exist;
- `profiles` and `profile_translations` already live in module-owned storage;
- `ProfileService`, `ProfilesReader`, batched summary lookup and GraphQL transport are already implemented;
- `blog` and `forum` already use the module as an author presentation boundary;
- taxonomy-backed `profile_tags`, `profile.updated` and explicit backfill path are already part of the live contract.

## Stages

### 1. Contract stability

- [x] lock profile boundary over `users`;
- [x] bring up module-owned storage, service layer and GraphQL baseline;
- [x] introduce `ProfilesReader` as downstream integration contract;
- [ ] maintain sync between runtime contracts, GraphQL surface and module metadata.

### 2. Rollout hardening

- [ ] decide whether a separate projection/read-model is needed beyond direct reading from `profiles + profile_translations`;
- [ ] finalize visibility/media policy and remaining storage decisions around handle uniqueness;
- [ ] keep profile backfill and `profile.updated` semantics compatible with downstream consumers.

### 3. UI and operability

- [ ] add module-owned UI packages after profile-domain contract is locked;
- [ ] develop audit trail, observability and runbook guidance for profile conflicts and rollout effects;
- [ ] document new guarantees concurrently with runtime/API surface changes.

## Verification

- `cargo xtask module validate profiles`
- `cargo xtask module test profiles`
- targeted tests for handle policy, locale fallback, summary batching, GraphQL path and backfill/events

## Update rules

1. When changing profile runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing downstream integration expectations, update consumer docs for `blog`, `forum` and other modules.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
