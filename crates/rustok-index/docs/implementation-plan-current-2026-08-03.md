# Current `rustok-index` implementation plan — 2026-08-03

Status overlay rechecked through
`main@78ea5461d2ff8f071318ef23e6fa08aa6aea2f94` and active branch
`agent/index-m6-drift-candidate-confirmation-20260806`.

The default-branch commits after the PostgreSQL candidate-reader merge bound Commerce GraphQL
query diagnostics and add a Page Builder inline adapter. They do not modify `crates/rustok-index`,
Index storage, Index source registries, Index services/transports, or Index guards changed by this
branch.

When this dated overlay conflicts with the older canonical plan, this file is the current source of
truth. Historical architecture and milestone context remain in `implementation-plan.md`.

## Current cursor

`M6 - persist confirmed stale and orphan findings`

The database-neutral stale/orphan candidate contract, PostgreSQL bounded reader, application
confirmation boundary, PostgreSQL materialized observer, and internal composition helper are source
complete.

Candidate confirmation now:

- consumes exactly one typed candidate;
- observes exact materialized identity/version/link state before owner calls;
- uses exact one-key targeted source loads and explicit retained absence watermarks;
- double-reads authoritative absence or source-link/target state;
- observes the materialized candidate again before returning a confirmed outcome;
- returns only typed `Confirmed` or closed-enum `NotCandidate` outcomes;
- maps dependencies to bounded retryable/permanent machine codes;
- performs no finding write, lifecycle transition, scheduling, transport mounting, or repair.

Finding persistence, lifecycle commands, repair, and retained execution evidence remain open.

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
  `source_complete`
- M6 PostgreSQL drift candidate reader:
  `source_complete`
- M6 bounded candidate confirmation and PostgreSQL materialized observer:
  `source_complete_finding_persistence_pending`
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

## M6 stale entity and orphan-link discovery

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
- [x] Use only bounded `limit + 1` keyset SQL and one deterministic stale-to-orphan transition.
- [x] Keep the reader unmounted: no server extension or public transport.
- [x] Add `IndexDriftCandidateConfirmer` over one candidate only.
- [x] Observe exact materialized state before and after provisional confirmation.
- [x] Confirm stale candidates only through an authoritative delete or admitted absence watermark
      with a stable version not below the indexed version.
- [x] Confirm orphan candidates only while the same source version/link/ordinal/target remains
      authoritative and the target has stable delete/absence evidence.
- [x] Add the PostgreSQL observer for exact source row/version/link/target-absence shape.
- [x] Map changed source, link, target, or materialized state to typed `NotCandidate` or bounded
      dependency failure without recording a finding.
- [x] Add a composition helper that returns the confirmer without publishing it.
- [ ] Derive a deterministic finding request from `IndexDriftConfirmedCandidate`.
- [ ] Revalidate write-time identity/version assumptions inside the persistence transaction.
- [ ] Delegate idempotently to Index-owned finding storage without exposing raw evidence JSON.
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

Add one internal idempotent persistence adapter for `IndexDriftConfirmedCandidate`.

The adapter must:

- accept only a confirmed missing-entity or orphan-link outcome;
- map missing entities to one exact entity finding scope;
- map orphan links to the exact source entity scope with a deterministic check identity that binds
  link name, ordinal, target schema/entity/locale, and target absence version;
- derive bounded deterministic SHA-256 evidence from typed fields rather than accept caller JSON;
- ensure expected and actual evidence digests differ by construction;
- re-read the exact materialized identity/version/link shape inside or immediately adjacent to the
  write transaction and return `NotRecorded` when it changed;
- delegate idempotently to the existing `PostgresIndexDriftFindingWriter` contract or a narrowly
  extended Index-owned writer;
- expose only finding receipt or typed `NotRecorded` plus bounded failures;
- perform no page loop, public transport, lifecycle command, scheduler registration, or repair.

Keep the persistence adapter internal and unmounted. Do not expose confirmed candidates or finding
writes through GraphQL, HTTP, CLI, MCP, or native-admin transports in this slice.

## Owner verification for this slice

```bash
cargo test -p rustok-index drift_candidate_confirmation -- --nocapture
node scripts/verify/verify-index-drift-candidate-confirmation.mjs
node scripts/verify/verify-index-postgres-drift-candidate-reader.mjs
node scripts/verify/verify-index-drift-candidate-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were executed by
the implementation agent.
