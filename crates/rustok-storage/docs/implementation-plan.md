# Implementation plan for `rustok-storage`

Status: storage abstraction baseline is already working; further work involves
maintaining the backend boundary and carefully expanding the backend-support matrix.

## Execution checkpoint

- Current phase: plan_sync
- Last checkpoint: Initial bootstrap by registry workflow.
- Next step: Synchronize the plan with the current code and select the first incomplete item.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block.
- Last updated at (UTC): 2026-05-20T00:00:00Z

## Scope of work

- maintain `rustok-storage` as a shared storage abstraction layer;
- synchronize backend contracts, path-safety guarantees and local docs;
- do not allow domain logic to blur into the storage layer.

## Current state

- `StorageBackend`, `UploadedObject` and `StorageService` already form the base contract;
- local backend is already implemented and used by the platform;
- path generation, public URL construction and basic health semantics are already part of the live surface;
- future S3-compatible backends are treated as additive extensions, not as a reason to break the existing contract.

## Stages

### 1. Contract stability

- [x] lock a single storage backend contract;
- [x] maintain path traversal protection and backend abstraction inside the crate;
- [ ] maintain sync between storage surface, host wiring and local docs.

### 2. Backend expansion

- [ ] add production-grade external backends as additive feature-based extensions;
- [ ] cover backend-specific failure semantics and config edge-cases with targeted integration tests;
- [ ] keep public URL and deletion semantics compatible across backends.

### 3. Operability

- [ ] evolve storage health, metrics and runbook guidance together with backend expansion;
- [ ] keep local docs synchronized with `rustok-media` and host/runtime docs;
- [ ] document new guarantees concurrently with storage contract changes.

## Verification

- structural verification for docs and storage boundary;
- targeted compile/tests when changing `StorageBackend`, `StorageService` or config contracts;
- integration checks for backend implementations and health semantics.

## Update rules

1. When changing storage contract, first update this file.
2. When changing public surface, synchronize `docs/README.md` and related consumer docs.
3. When changing host/storage wiring expectations, update consumer runtime docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
