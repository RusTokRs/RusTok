# Current `rustok-index` implementation plan — 2026-08-03

Status overlay for `implementation-plan.md` rechecked through
`main@df21dcb5164e29b61248bb0ac68152f8c5d5d858` and active draft PR #3037.

The default-branch commits after the bounded candidate-contract merge harden Commerce Return
Completion and Commerce Admin Fulfillment Reconciliation diagnostics. They do not modify
`crates/rustok-index`, Product Index composition, Index transports/services, or Index guards changed
by this branch.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - confirm bounded stale and orphan candidates`

Exact drift diagnosis, Product locale absence proof, missing-only page classification, confidential
source continuation, private server keyring composition, sealed one-page execution, bounded GraphQL
transport, the database-neutral stale/orphan candidate contract, and the PostgreSQL bounded candidate
reader are source complete.

The PostgreSQL reader now:

- accepts one exact tenant/schema `IndexDriftCandidateRequest`;
- runs one `REPEATABLE READ READ ONLY` transaction per page;
- captures a scope-bound `txid_current_snapshot()` fence on the first page;
- excludes post-fence inserted/updated row versions through `txid_visible_in_snapshot(xmin, fence)`;
- performs `limit + 1` keyset reads with stale entities before orphan links;
- carries only version, scope, phase, and the last ordering tuple in the private cursor;
- exposes only typed identities and positive source versions;
- performs no source call, finding write, lifecycle transition, scheduling, or repair.

Candidate confirmation, retained PostgreSQL/concurrency evidence, finding lifecycle commands, and
repair remain open.

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
- M6 bounded stale-entity and orphan-link candidate contract:
  `source_complete_postgres_reader_complete_confirmation_pending`
- M6 PostgreSQL drift candidate reader:
  `source_complete_candidate_confirmation_pending`
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

## M6 rebuild, reconciliation, diagnosis, and repair

### Replay and scheduling

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add source-call timeouts, no-write dry-run, and cooperative page interruption safe points.
- [x] Add bounded retry/dead-letter transitions, authorized requeue, and generic host scheduling.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Retain multi-host scheduler, restart, graceful-shutdown, and command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [ ] Add targeted, full, and shadow rebuild modes.

### Exact diagnosis and missing discovery

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

### Stale entity and orphan-link discovery

- [x] Add a database-neutral bounded candidate contract with one exact tenant/schema scope and page
      size at most 32.
- [x] Require fence and cursor together, immutable fence identity, advancing cursor, bounded pages,
      exact candidate scope, and strict deterministic order.
- [x] Expose only typed stale entity identity/version and typed orphan source/link/target identity.
- [x] Add `PostgresIndexDriftCandidateReader` over `index_entities` and `index_links`.
- [x] Run one read-only repeatable-read transaction per page and capture one scope-bound PostgreSQL
      transaction-snapshot fence.
- [x] Filter row insertion versions through `txid_visible_in_snapshot` so late commits and
      post-fence updates cannot add candidates to continuation pages.
- [x] Use only bounded `limit + 1` keyset SQL and permit one deterministic stale-to-orphan phase
      transition without cross-page accumulation.
- [x] Keep payloads, fields, fingerprints, owner records, graph aggregates, SQL, and database causes
      out of candidate results and failures.
- [x] Keep the reader unmounted: no server runtime extension, GraphQL, HTTP, CLI, MCP, or native
      admin surface.
- [ ] Confirm stale entity candidates with exact source load or admitted absence proof before any
      finding write.
- [ ] Confirm orphan links by re-reading the exact source link and typed target state before any
      finding write.
- [ ] Require candidate identity and indexed source version to remain unchanged through
      confirmation.
- [ ] Add confirmed/not-candidate typed outcomes separately from persistence.
- [ ] Add resolve/ignore lifecycle commands with actor/reason audit and fail-closed authorization.
- [ ] Add targeted repair with before/after admitted evidence.

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

Add one internal database-neutral candidate confirmation boundary. It must consume a single typed
`IndexDriftCandidate` and return a bounded typed confirmation outcome without writing a finding.

### Stale entity confirmation

- re-read the exact candidate key through the existing source registry;
- use the existing admitted absence-watermark registry when targeted source load is empty;
- require positive authoritative source version;
- compare the candidate indexed source version and exact materialized key against a fresh bounded
  materialized observation;
- return `ConfirmedMissing`, `NotCandidate`, or bounded retryable/permanent dependency failure;
- do not record a mismatch in this first confirmation slice.

### Orphan-link confirmation

- re-read the exact materialized source entity/version and exact link identity;
- verify that the link is still present with the same ordinal and typed target;
- re-read the exact target materialized state and, where required, owner visibility/lifecycle state;
- return `ConfirmedOrphan`, `NotCandidate`, or bounded dependency failure;
- expose no source/target payload, graph aggregate, SQL, or database cause.

The confirmation slice must remain internal. Do not add public transport, background loops,
cross-page accumulation, finding lifecycle, scheduling, or repair.

## Owner verification for the completed reader slice

```bash
cargo test -p rustok-index drift_candidate_reader -- --nocapture
cargo test -p rustok-index drift_candidates -- --nocapture
node scripts/verify/verify-index-postgres-drift-candidate-reader.mjs
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
