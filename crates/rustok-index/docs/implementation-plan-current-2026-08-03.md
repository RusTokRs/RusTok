# Current `rustok-index` implementation plan — 2026-08-03

Status overlay rechecked through
`main@e903e9e5d2a0e186432e436a4dc353b752218219` and active branch
`agent/index-m6-targeted-drift-repair-20260806`.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - compose targeted repair evidence and owner`

The database-neutral stale/orphan candidate contract, PostgreSQL bounded reader, application
confirmation boundary, serializable finding persistence, fail-closed finding lifecycle commands, and
generic targeted-repair orchestration with durable PostgreSQL reservations/receipts are source
complete.

Targeted repair now:

- accepts one exact tenant/finding/command identity and one typed missing-entity or orphan-link target;
- authorizes before minting a non-public store/owner capability;
- verifies the typed target as the cryptographic preimage of the stored finding identity and evidence;
- reserves at most one active command per finding under a serializable database fence;
- requires admitted before evidence, one target-kind owner call, and admitted after evidence;
- persists one immutable terminal repaired/not-repaired receipt;
- makes exact command replay resumable or terminally idempotent;
- leaves ambiguous prepared commands fail-closed rather than silently expiring them;
- remains unmounted from runtime extensions and public transports.

A concrete evidence reader, concrete idempotent repair owner, prepared-command recovery policy, and
retained execution evidence remain open.

## Rechecked status

- M0 reset and M1 domain/application core: `complete`
- M2 JSONB storage decision and replacement benchmark packet: `complete`
- M3 production storage/evidence tooling: `source_complete_owner_execution_pending`
- M4 query planning/compiler/runtime and privacy shadow: `source_complete_owner_execution_pending`
- M5 inbox deduplication and monotonic source versions: `complete`
- M5 mutation event registry and commit-before-ack orchestration:
  `source_complete_broker_wiring_pending`
- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`
- M6 replay jobs, checkpoints, multi-page execution, cancellation, retry/dead-letter, and generic
  host scheduling: `source_complete_owner_execution_pending`
- M6 bounded drift-finding persistence and inspection:
  `source_complete_owner_execution_pending`
- M6 snapshot-pair digest producer and missing-only selector: `source_complete`
- M6 source-version-fenced PostgreSQL exact drift snapshot reader:
  `source_complete_owner_execution_pending`
- M6 guarded exact-entity diagnosis and bounded GraphQL transport:
  `source_complete_owner_execution_pending`
- M6 source-page missing diagnosis, confidential continuation, private server keyring, and sealed
  GraphQL transport: `source_complete_owner_execution_pending`
- M6 explicit source absence watermark registry and Product locale provider:
  `source_complete_owner_execution_pending`
- M6 Product locale absence PostgreSQL harness:
  `source_ready_owner_execution_pending`
- M6 bounded stale-entity and orphan-link candidate contract: `source_complete`
- M6 PostgreSQL drift candidate reader: `source_complete`
- M6 bounded candidate confirmation and PostgreSQL materialized observer: `source_complete`
- M6 confirmed-candidate finding persistence: `source_complete`
- M6 drift finding lifecycle commands: `source_complete`
- M6 generic targeted drift repair boundary and durable receipt store:
  `source_complete_owner_composition_pending`
- M7 Product/ProductVariant/SalesChannel bounded replay graph:
  `source_complete_owner_execution_pending`

## M5 incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [x] Add inbox deduplication and monotonic source versions.
- [x] Add a database-neutral mutation-source event registry and commit-before-ack orchestration.
- [ ] Register production owner event routes and compose a concrete broker consumer/acknowledger.
- [ ] Add batch transactions, bounded backoff, dead-letter state, and lag metrics around production
      event routes.
- [ ] Retain crash-between-commit-and-ack and redelivery evidence.

## M6 replay and scheduling

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add source-call timeouts, no-write dry-run, and cooperative page interruption safe points.
- [x] Add bounded retry/dead-letter transitions, authorized requeue, and generic host scheduling.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Retain multi-host scheduler, restart, graceful-shutdown, and command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [ ] Add targeted, full, and shadow rebuild modes.

## M6 exact diagnosis and missing discovery

- [x] Add bounded drift-finding persistence and inspection.
- [x] Add one exact snapshot-pair digest producer with deterministic SHA-256 digests.
- [x] Add the source-version-fenced PostgreSQL exact drift snapshot reader.
- [x] Add explicit retained absence/tombstone watermark proof with canonical source-owner parity.
- [x] Register Product locale absence for Product schema versions 1 and 2.
- [x] Expose exact diagnosis through bounded GraphQL `diagnoseIndexEntity`.
- [x] Add the missing-only selector and one bounded internal source-page diagnosis runtime.
- [x] Keep the raw owner cursor server-internal and expose only authenticated encrypted continuation.
- [x] Compose the private server `SecretRef` keyring and sealed one-page service boundary.
- [x] Expose one bounded source-page GraphQL mutation over the sealed method only.
- [ ] Run and admit retained exact GraphQL, Product absence, PostgreSQL, continuation, and rotation
      evidence.

## M6 stale entity, orphan-link, lifecycle, and targeted repair

- [x] Add a database-neutral bounded candidate contract with one exact tenant/schema scope and page
      size at most 32.
- [x] Require fence and cursor together, immutable fence identity, advancing cursor, bounded pages,
      exact candidate scope, and strict deterministic order.
- [x] Expose only typed stale entity identity/version and typed orphan source/link/target identity.
- [x] Add `PostgresIndexDriftCandidateReader` over `index_entities` and `index_links`.
- [x] Run one read-only repeatable-read transaction per page and capture one scope-bound PostgreSQL
      transaction-snapshot fence.
- [x] Use only bounded `limit + 1` keyset SQL and one deterministic stale-to-orphan transition.
- [x] Add `IndexDriftCandidateConfirmer` over one candidate only.
- [x] Observe exact materialized state before and after provisional confirmation.
- [x] Confirm stale/orphan candidates only through stable authoritative source/delete/absence evidence.
- [x] Add the PostgreSQL exact materialized observer.
- [x] Derive deterministic finding identity and SHA-256 evidence from confirmed candidates.
- [x] Revalidate write-time entity/version/link/target state in one serializable transaction.
- [x] Create, refresh, reopen, or suppress through the established Index finding contract.
- [x] Add fail-closed resolve/ignore commands with authorization capability and immutable audit rows.
- [x] Add typed targeted-repair command, authorization capability, evidence and owner ports.
- [x] Verify typed repair targets against exact persisted check/finding/evidence commitments.
- [x] Add one active-command-per-finding serializable reservation and idempotent terminal receipt.
- [x] Preserve finding and lifecycle rows; successful repair does not silently resolve a finding.
- [x] Keep repair unmounted from server extensions, public transports, schedulers, and page loops.
- [ ] Compose one concrete admitted evidence reader.
- [ ] Compose one concrete idempotent repair owner for the smallest supported target kind.
- [ ] Add prepared-command lease/abandon/recovery policy and lifecycle coordination.
- [ ] Retain migration, crash-window, owner-idempotency, and PostgreSQL concurrency evidence.

## M7 production graph and cutover

- [x] Add Product, ProductVariant, and SalesChannel schemas and bounded current-state sources.
- [x] Add stable replay identities and retained deletes.
- [x] Add Product-to-ProductVariant graph materialization.
- [ ] Add production owner event routes and concrete incremental consumer wiring.
- [ ] Persist and enforce per-tenant schema readiness.
- [ ] Complete durable Product-to-SalesChannel relation semantics and retained evidence.
- [ ] Admit tombstone purge, freshness/outage/restart/backlog recovery, and delete/recreate evidence.
- [ ] Admit live PostgreSQL/reference query equivalence and one full partition packet.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## Next implementation step

Compose one concrete bounded evidence reader and one concrete idempotent repair owner for the smallest
supported confirmed finding kind.

The next slice must:

- reuse the existing source registry, admitted absence proof, and exact materialized observation;
- derive before/after evidence from typed state rather than caller digests or payloads;
- support exactly one target kind and reject all others before owner mutation;
- make the owner mutation idempotent by the durable repair command UUID;
- apply at most one exact mutation through an existing owner/storage contract;
- return a bounded owner receipt digest without exposing source or Index payload;
- preserve the prepared reservation across retryable source/owner/evidence failure;
- add no public transport, scheduler, automatic finding iteration, or lifecycle transition.

Prepared-command recovery policy, a second repair kind, public authorization transport, and automatic
repair remain separate later slices.

## Owner verification for this slice

```bash
cargo test -p rustok-index drift_repair -- --nocapture
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios, workflows, or
CI were executed by the implementation agent.
