# Marketplace Payout implementation plan

Last reviewed: 2026-07-22

## Ownership

`rustok-marketplace-payout` owns seller payout admission, scheduling, durable
reservation orchestration state, payout/provider execution state, and payout read
projections. It does not own seller balances or ledger entries.

Ledger movement remains owned by `rustok-marketplace-ledger` and is invoked only
through typed ports. Bank, processor, or transfer-provider facts must be normalized
through a payout provider SPI and journal before changing payout state.

## Current source state

- [x] Payout header, item assignment, and schedule command receipt source.
- [x] Typed read and command ports for the existing scheduling slice.
- [x] Exclusive `(tenant_id, ledger_entry_id)` payout assignment.
- [x] Canonical `marketplace_*` entity names.
- [x] Upgrade repair for the original legacy migration table names.
- [x] Durable payout operation and per-order transfer journal schema.
- [x] Typed operation/transfer status and stage entities.
- [x] Tenant-scoped internal foreign keys and database checks.
- [x] Operation/transfer revision, lease, attempt, and safe error-code fields.
- [x] Runtime admission/replay repository for payout operations.
- [x] CAS lease acquisition and expired-lease recovery.
- [x] Per-order `reserve_hold` execution through `MarketplaceLedgerCommandPort`.
- [x] Reverse-order `reserve_release` compensation.
- [x] Create payout only from confirmed ledger transfer responses.
- [x] Process-owned server composition for ledger read/command and payout services.
- [x] Typed payout provider SPI and registry contracts.
- [x] Manual provider baseline that returns `submitted`, never implicit `paid`.
- [x] Durable provider operation journal schema with tenant-scoped payout ownership.
- [ ] Cancellation and reservation release workflow.
- [ ] Payout provider accounts and destination ownership.
- [x] Journaled provider submission admission, lease, checkpoint, and replay runtime.
- [ ] Provider lookup recovery and verified webhook inbox.
- [ ] `payout_settlement` and `payout_reversal` ledger posting.
- [ ] Accounting, operator, and seller surfaces.
- [ ] Live embedded/remote FBA evidence.

## Schema contract

### `marketplace_payout_operations`

Stores one actor-bound durable scheduling operation:

- tenant, actor, seller, and currency identity;
- idempotency key and canonical request hash;
- immutable request JSON needed for restart;
- typed status and stage;
- optional resulting payout ID;
- attempt count and optimistic revision;
- lease owner/expiry;
- safe error code only;
- completion timestamps.

### `marketplace_payout_operation_transfers`

Stores deterministic ledger child operations grouped by source order:

- parent payout operation;
- stable sequence and order identity;
- `reserve_hold` or `reserve_release` kind;
- idempotency key, request hash, and request snapshot;
- total minor-unit amount;
- ledger transfer and transaction IDs after confirmation;
- attempt, revision, safe error code, and completion timestamps.

A ledger balance-transfer command intentionally accepts references from one order.
Multi-order payouts therefore create multiple child rows. Compensation executes
confirmed reserve rows in descending sequence order.

### `marketplace_payout_provider_operations`

Stores one durable provider operation per payout and operation kind:

- tenant and payout owner identity;
- `submit`, `lookup`, or `cancel` operation kind;
- provider identity and provider-scoped idempotency key;
- immutable request hash and request JSON;
- typed pending/executing/provider-result/reconciliation/committed state;
- provider reference and normalized result only after a confirmed response;
- attempt count, optimistic revision, lease owner/expiry;
- safe error code only;
- provider-completed and local-committed checkpoints.

A provider success is not a local payout commit. `provider_succeeded` records the
external result; `committed` is allowed only after the owner persists the local state
that follows from that result. Unknown outcomes remain `reconciliation_required` and
must not trigger automatic settlement.

## Provider contract

`PayoutProvider` exposes typed `submit`, `lookup`, and `cancel` operations. The
registry validates provider identity, capabilities, health, request identity, minor
unit amount, currency, metadata shape, external references, and result status.

The manual baseline exists for operator-driven flows. A manual submit produces
`submitted`, not `paid`; an operator or later verified fact must explicitly advance
the payout. This prevents the baseline adapter from fabricating external completion.

The submit runtime now:

1. replays a completed provider checkpoint before reading mutable payout state;
2. admits one immutable `submit` operation per payout and provider-scoped key;
3. claims pending or retryable work through revision and lease CAS;
4. executes the provider from the persisted request snapshot;
5. checkpoints submitted, processing, paid, and confirmed failed results before payout mutation;
6. routes invalid responses, unknown results, unknown outcomes, and expired executing leases to
   `reconciliation_required` without automatic resubmission;
7. leaves payout state and Reserved-to-Paid settlement unchanged for a later commit slice.

## Host composition contract

`apps/server` owns one process-scoped `MarketplacePayoutRuntime`. Its in-process
composition is:

1. `MarketplaceAllocationService` as `MarketplaceAllocationReadPort`;
2. `MarketplaceCommissionService` as `MarketplaceCommissionReadPort`;
3. one `MarketplaceLedgerService` exposed as both `MarketplaceLedgerReadPort` and
   `MarketplaceLedgerCommandPort`;
4. one `MarketplacePayoutService` configured with those two ledger interfaces.

The runtime is stored in `ServerRuntimeContext` and attached to every
`HostRuntimeContext`. Repeated host initialization reuses the same payout and ledger
service instances instead of rebuilding an independent command owner.

## Migration contract

The original migration generated `payouts`, `payout_items`, and `payout_receipts`
while entities used `marketplace_*` names. The operation migration:

1. detects a complete legacy or complete canonical table set;
2. rejects mixed states;
3. renames the complete legacy set to canonical names;
4. records a repair marker only when it performed the rename;
5. creates operation persistence;
6. restores legacy names on down only when the marker exists.

Required evidence:

- [ ] clean SQLite apply/down/reapply;
- [ ] upgraded SQLite apply;
- [ ] clean PostgreSQL apply/down/reapply;
- [ ] upgraded PostgreSQL apply under existing rows;
- [ ] clean MySQL apply/down/reapply;
- [ ] mixed legacy/canonical state fails closed;
- [ ] existing payout data survives rename;
- [ ] tenant composite foreign keys reject cross-tenant links;
- [ ] provider operation checks reject invalid lease/result/commit states;
- [ ] provider-scoped idempotency rejects duplicate external effects.

## Execution order

1. [x] Repair canonical table names and add operation schema.
2. [x] Add operation admission and completed-result replay.
3. [x] Resolve selected ledger entries and group by order.
4. [x] Persist deterministic reserve children.
5. [x] Execute/adopt `reserve_hold` children.
6. [x] Create payout/items from confirmed reserve responses.
7. [x] Complete operation and replay the resulting payout.
8. [x] Add reverse-order `reserve_release` compensation.
9. [x] Compose ledger read/command and payout services in the server host.
10. [x] Add payout provider SPI, registry, and durable provider-operation schema.
11. [ ] Add provider account and destination ownership.
12. [x] Add journaled provider submit execution and durable result replay.
13. [ ] Add lookup recovery and verified webhook inbox.
14. [ ] Add settlement/reversal ledger transfers.
15. [ ] Add cancellation and reservation release workflow.
16. [ ] Add operator/seller transports and UI.
17. [ ] Retain contention, restart, reconciliation, and remote-profile evidence.

## Promotion gate

The module remains `in_progress`. Schema or source inspection alone must not promote
FBA status. Promotion requires compiled contracts, clean/upgraded database evidence,
concurrent reservation proof, process restart recovery, provider redelivery, ledger
reconciliation, mounted transports, and operator workflows.

## Implemented reservation slice

- durable operation admission and request-hash replay
- lease/revision claim before side effects
- deterministic per-order `reserve_hold` children
- persisted ledger request and response payloads for crash recovery
- existing atomic payout receipt transaction reused after reservation
- reverse-order `reserve_release` compensation based on Reserved credit entries
- retryable compensation and operator reconciliation states
- process-owned server composition of the payout and ledger command path

## Implemented provider-contract slice

- typed provider capabilities, descriptor, health, registration, and registry
- typed submit, lookup, cancel requests and normalized result
- manual provider baseline without implicit payment completion
- provider identity and response validation
- durable provider operation entity and migration
- provider-scoped idempotency and one operation-kind row per payout
- lease/revision/attempt and provider/local completion checkpoints
- safe provider boundary errors and source/schema regression tests


## Implemented provider-submission slice

- provider-scoped admission and immutable request-hash replay
- one submit operation per payout
- revision/lease claim before the external side effect
- persisted normalized provider result before local payout mutation
- confirmed failed results stored as `provider_failed`, not `provider_succeeded`
- unknown/invalid outcomes routed to durable reconciliation
- normalized `unknown` results retain provider reference, result JSON, and completion time
- expired executing leases never automatically resubmit
- payout status, `paid_at`, and ledger settlement remain unchanged

Provider host composition, lookup recovery, local payout commit, and Reserved-to-Paid settlement remain separate follow-up slices.
