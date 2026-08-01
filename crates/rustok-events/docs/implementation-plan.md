# Implementation plan for `rustok-events`

## Source of truth

This file owns shared typed event payloads, envelope validation, schema registry, and
deterministic contract digest generation. Domain transactions and transport execution
remain in producer owners, Outbox, broker runtimes, and consumers.

Last reconciled with `main`: 2026-08-01.

## Current state

- Root and typed contract envelopes validate at publication, transport, and decode
  boundaries.
- External crates cannot implement `EventContract` or publish arbitrary typed-family
  names.
- Tenant, actor, correlation, causation, trace, retry, and timestamp facts remain
  envelope metadata.
- `contracts/event-contract-digests.json` is generated only by the repository-owned
  command after intentional schema review.

Merged PR #2867 registers canonical built-in role mutation contracts:

- `rbac.user_role_replaced`;
- `rbac.user_role_assignment_repaired`.

Their owner policy and same-transaction publication live in `rustok-rbac` and the Auth
admin adapter. Exact canonical role replay emits neither event.

Draft PR #2866 adds the remaining artifact permission family:

- `rbac.artifact_role_permission.assignment_changed` v1;
- mutation, idempotency receipt, relation state, and event share one owner transaction;
- exact retry and state no-op emit no event;
- tenant and actor remain envelope metadata;
- required Outbox failure rolls back the mutation and receipt.

## Delivered results

- [x] Seal typed contract families.
- [x] Validate payload and envelope metadata.
- [x] Keep explicit event type and schema version registry entries.
- [x] Generate deterministic JSON schemas and contract digests.
- [x] Register merged RBAC role replacement/repair contracts from #2867.
- [x] Register artifact role-permission assignment contract in draft #2866.
- [x] Add artifact contract validation and envelope round-trip tests.
- [x] Add source guards for owner transaction and rollback ordering.
- [ ] Generate and review the exact-head digest for #2866.
- [ ] Execute Events/RBAC/server contract, transaction, verifier, and module gates.

## Open work

### P0. Digest and exact-head verification

- [x] Reconstruct #2866 as one commit on the latest `main`.
- [ ] Run `event_contract_digests -- --write` on that exact head.
- [ ] Review and commit generator output only; never guess or hand-edit hashes.
- [ ] Run all-target Events compilation and focused RBAC event tests.

### P1. Producer and consumer evidence

- [ ] Execute artifact mutation success, exact retry, state no-op, publication failure,
  and rollback cases.
- [ ] Identify approved artifact permission event consumers.
- [ ] Require consumers to be tenant-bound, idempotent, replay-safe, and
  non-authoritative for access decisions.
- [ ] Define retention and replay guidance before remote consumption.

### P1. Existing transport evidence

- [ ] Execute retained Iggy restart, redelivery, acknowledgement, DLQ, reconnect,
  rebalance, and multi-replica ownership packets.
- [ ] Prove exact-byte durable DLQ receipts and acknowledgement-only recovery.
- [ ] Preserve poison messages and source coordinates for undecodable payloads.

## Verification commands

```bash
cargo run -p rustok-events --example event_contract_digests -- --write
cargo check -p rustok-events --all-targets
cargo test -p rustok-events rbac_role_mutation
cargo test -p rustok-events --test rbac_artifact_permission_contracts
cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite
cargo test -p rustok-server --lib artifact_permission
node scripts/verify/verify-rbac-owner-role-mutation-contract.mjs
node scripts/verify/verify-rbac-artifact-permission-outbox.mjs
cargo xtask module validate events
cargo xtask module test events
cargo xtask module validate rbac
cargo xtask module test rbac
```

No command above was executed in this connector-only source slice.

## Completion gates

- A new event family is not reviewed until its generated digest is committed.
- A reviewed contract is not operationally verified until producer transaction and
  consumer replay/idempotency evidence pass.
- Authorization consumers may react to RBAC events but cannot authorize from them.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-01`
- Scope inspected: `merged owner role mutation contracts; artifact permission sealed family; envelope registry; owner transaction ordering; digest synchronization`
- Findings: `P0=1, P1=1, P2=0, P3=0`
- Fixed in this pass: `draft PR #2866 adds sealed artifact role-permission assignment events through the canonical Outbox in the same RBAC owner transaction. Exact retry and state no-op publish nothing, while required publication failure rolls back mutation and idempotency receipt. The branch is reconstructed as one commit on the latest main.`
- Remaining risks or blockers: `The #2866 generated contract digest is absent. Events/RBAC/server compilation, focused tests, Node/module gates, approved consumers, replay guidance, and runtime transport evidence remain absent.`
- Evidence: `source review confirms the artifact family composes additively with merged #2867 role mutation contracts, retains typed validation and transaction-bound publication, and is one commit zero behind current main. No generator or runtime execution is claimed.`
- Next action: `generate and review the #2866 digest, then execute exact-head Events/RBAC/server contract and transaction gates`
- Resume command: `cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs`
