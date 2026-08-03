# Implementation plan for `rustok-events`

## Source of truth

This file owns shared typed event payloads, envelope validation, schema registry, and
deterministic contract digest generation. Domain transactions and transport execution
remain in producer owners, Outbox, broker runtimes, and consumers.

Last reconciled with `main`: 2026-08-03.

## Current state

- Root and typed contract envelopes validate at publication, transport, and decode
  boundaries.
- External crates cannot implement `EventContract` or publish arbitrary typed-family
  names.
- Tenant, actor, correlation, causation, trace, retry, and timestamp facts remain
  envelope metadata.
- `contracts/event-contract-digests.json` is generated only by the repository-owned
  command after intentional schema review.
- The sealed-contract boundary uses one narrow documented `#[expect(private_bounds)]`;
  the touched file contains no broad lint allowance.

Merged PR #2867 registers canonical built-in role mutation contracts:

- `rbac.user_role_replaced`;
- `rbac.user_role_assignment_repaired`.

Draft PR #2870, which supersedes closed #2866, adds the artifact permission family:

- `rbac.artifact_role_permission.assignment_changed` v1;
- the payload carries both the immutable `artifact_permission_id` and the admitted
  installation/key evidence;
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
- [x] Register artifact role-permission assignment contract in draft #2870.
- [x] Bind the event to exact immutable artifact permission identity.
- [x] Add artifact contract validation, nil-identity, and envelope round-trip tests.
- [x] Add source guards for owner transaction, rollback ordering, and exact identity.
- [x] Replace the broad sealed-contract lint allowance with a narrow reasoned expectation.
- [ ] Generate and review the exact-head digest for #2870.
- [ ] Execute Events/RBAC/server contract, transaction, verifier, and module gates.

## Open work

### P0. Digest and exact-head verification

- [ ] Run `event_contract_digests -- --write` on the final #2870 head.
- [ ] Review and commit generator output only; never guess or hand-edit hashes.
- [ ] Run all-target Events compilation and focused RBAC event tests.
- [ ] Re-run after the final merge-base reconciliation if `main` advances.

### P1. Producer and consumer evidence

- [ ] Execute artifact mutation success, exact retry, state no-op, publication failure,
  exact permission identity, and rollback cases.
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
cargo fmt --all -- --check
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

No command above was executed in this connector-only source slice. GitHub workflow
results are recorded only after the corresponding exact-head jobs finish.

## Completion gates

- A new event family is not reviewed until its generated digest is committed.
- A reviewed contract is not operationally verified until producer transaction and
  consumer replay/idempotency evidence pass.
- Authorization consumers may react to RBAC events but cannot authorize from them.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-08-03`
- Scope inspected: `RBAC artifact permission sealed family; immutable permission identity; envelope registry and validation; owner transaction ordering; digest synchronization; touched-file lint policy`
- Findings: `P0=0, P1=1, P2=0, P3=1`
- Fixed in this pass: `draft PR #2870 supersedes closed #2866, propagates exact artifact_permission_id through the sealed assignment event and all contract tests, preserves transaction-bound Outbox publication and rollback semantics, and replaces the broad private_bounds allowance with one narrow documented expectation.`
- Remaining risks or blockers: `The #2870 generated contract digest is absent. Events/RBAC/server compilation, focused tests, Node/module gates, approved consumers, replay guidance, and runtime transport evidence remain absent.`
- Evidence: `static source review confirms a registered v1 payload with six fields, non-nil operation/artifact-permission/role/installation validation, exact identity propagation from the RBAC owner, tenant/actor envelope metadata, and no event for retry or state no-op. No generator or runtime execution is claimed.`
- Next action: `generate and review the #2870 digest, then execute exact-head Events/RBAC/server contract and transaction gates`
- Resume command: `cargo fmt --all -- --check && cargo run -p rustok-events --example event_contract_digests -- --write && cargo check -p rustok-events --all-targets && cargo test -p rustok-events --test rbac_artifact_permission_contracts && cargo test -p rustok-rbac --test artifact_permission_outbox_sqlite && node scripts/verify/verify-rbac-artifact-permission-outbox.mjs`
