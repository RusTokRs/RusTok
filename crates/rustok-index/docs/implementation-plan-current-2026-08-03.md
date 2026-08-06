# Current `rustok-index` implementation plan — 2026-08-03

Status overlay rechecked through
`main@d890fd6f29a3c1dc2b883d570af9b3e9c094342d` and active branch
`agent/index-m6-prepared-repair-recovery-20260806`.

The non-overlapping merge-base-to-main review is retained in
`implementation-main-delta-2026-08-06-1240.md`.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - compose concrete orphan-link repair`

The database-neutral stale/orphan candidate contract, PostgreSQL bounded reader, application
confirmation boundary, serializable finding persistence, fail-closed finding lifecycle commands,
generic targeted-repair orchestration, durable repair reservations/receipts, one concrete
missing-entity evidence/owner composition, and authorization-gated prepared-command recovery are
source complete.

Concrete missing-entity repair now:

- accepts only the exact confirmed missing-entity target through a pre-reservation target gate;
- brackets the exact materialized identity with two authoritative source/absence reads;
- requires an absence version strictly newer than the live indexed version;
- applies one typed delete through the established mutation inbox and schema validation;
- uses the durable repair command UUID as the mutation event and delivery identity;
- requires an exact tombstone at the admitted absence version before recording `Repaired`;
- creates one immutable revision-0 active recovery decision for each new reservation;
- requires active recovery state before retry, owner mutation, and receipt completion;
- serializes pause/abandon against the owner call with the exact command advisory fence;
- fails legacy decision-less, paused, and abandoned prepared commands closed;
- remains unmounted from runtime extensions and public transports.

Orphan-link repair, public transport, automatic iteration, and retained execution evidence remain
open.

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
- M6 generic targeted drift repair boundary and durable receipt store: `source_complete`
- M6 concrete missing-entity evidence reader and idempotent mutation owner:
  `source_complete_recovery_aware_owner_execution_pending`
- M6 prepared repair pause/resume/abandon recovery policy:
  `source_complete_owner_execution_pending`
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
- [x] Compose one concrete admitted evidence reader for confirmed missing-entity findings.
- [x] Compose one concrete idempotent missing-entity delete owner through `PostgresMutationStore`.
- [x] Gate unsupported orphan-link targets before durable reservation in the concrete composition.
- [x] Bind mutation retry identity to the durable repair command UUID.
- [x] Add fail-closed prepared-command pause/resume/abandon recovery and lifecycle coordination.
- [x] Retain immutable command-scoped recovery decisions and require active state at owner/completion.
- [ ] Add a concrete orphan-link repair owner behind the admitted recovery boundary.
- [ ] Add time-derived lease expiry only with retained owner-liveness and crash-window evidence.
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

Compose one concrete bounded repair path for confirmed orphan-link findings behind the existing
recovery-aware boundary.

The next slice must:

- accept only the exact `index.confirmed_orphan_link.<sha256>` persisted commitment;
- re-read the exact source entity, link name, ordinal, linked target, and target absence proof;
- reject a changed source link, target identity, ordinal, source version, or absence version;
- apply one typed idempotent link-removal mutation through an established owner boundary rather than
  direct repair SQL;
- bind mutation event and inbox identity to the durable repair command UUID;
- preserve the active recovery fence through the owner call and completion trigger;
- require admitted before and after evidence before recording `Repaired`;
- add no public transport, automatic scanner, scheduler loop, or lifecycle auto-resolution.

Public authorization transport, automatic iteration, time-derived leases, and retained production
evidence remain separate later slices.

## Owner verification for this slice

```bash
cargo test -p rustok-index drift_repair -- --nocapture
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios,
workflows, or CI were executed by the implementation agent.
