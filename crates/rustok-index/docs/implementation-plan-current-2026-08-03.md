# Current `rustok-index` implementation plan — 2026-08-03

Status overlay for `implementation-plan.md` rechecked through
`main@9822131a59619805dc34b67fdcecf44f9bbcd766` and the active draft PR #2986.
The forty-four main commits after this branch merge base touch Commerce diagnostics, Forum
GraphQL/admin and route ownership, Pages/Page Builder delivery/routes/evidence, event-delivery
settings, and supporting server configuration. They do not touch `crates/rustok-index`, Product
Index composition, the server Index GraphQL files, Index diagnosis/page composition, or Index guards
changed by this branch. `apps/server/Cargo.toml` and general server settings changed on `main`, but
this PR does not modify them in the continuation slice.

When the older canonical plan's current-state bullets conflict with this dated overlay, this
overlay is the rechecked source of truth. Historical architecture, ownership, and milestone details
remain in `implementation-plan.md`.

## Current cursor

`M6 - expose only the sealed source-page boundary through bounded transport`

The database-neutral digest producer, mismatch-only writer adapter, locale-complete finding scope,
source-version-fenced PostgreSQL snapshot reader, guarded exact-entity diagnosis capability,
explicit owner-retained absence registry, Product locale high-watermark provider, double-read
absence-version fence, Product locale absence PostgreSQL harness, bounded GraphQL exact-entity
transport, missing-only entity candidate outcome, internal one-page missing-entity diagnosis runtime,
authenticated/confidential source continuation codec, private server-owned `SecretRef` keyring, and
sealed internal page boundary are source complete.

The server validates deployment configuration containing only bounded key IDs and secret references.
Secret values must be URL-safe unpadded base64 and decode to exactly 32 bytes. Because secret
resolution is asynchronous, key material is resolved inside the sealed request before token parsing
or source scan, used to construct one short-lived codec, and dropped after the incoming token is
opened and the outgoing cursor is sealed.

Public source-page transport, owner execution evidence, finding lifecycle commands, repair, and
broader Index-only or orphan discovery remain open.

## Rechecked status

- M0 reset and M1 domain/application core: `complete`
- M2 JSONB storage decision and replacement benchmark packet: `complete`
- M3 production storage/evidence tooling: `source_complete_owner_execution_pending`
- M4 query planning/compiler/runtime and privacy shadow: `source_complete_owner_execution_pending`
- M5 inbox deduplication and monotonic source versions: `complete`
- M5 mutation event registry and commit-before-ack orchestration:
  `source_complete_broker_wiring_pending`
- M5/M6 bounded source replay contract: `source_complete_owner_execution_pending`
- M6 one-page replay and durable checkpoint progression: `source_complete`
- M6 replay jobs, attempt fencing, multi-page execution, cancellation, and reconciliation:
  `source_complete_owner_execution_pending`
- M6 replay operator composition and authorization: `source_complete_owner_execution_pending`
- M6 bounded replay interruption, source timeout, and no-write dry-run:
  `source_complete_owner_execution_pending`
- M6 reconciliation retry, dead-letter, recovery, and generic host scheduling:
  `source_complete_owner_execution_pending`
- M6 bounded drift-finding inspection and persistence:
  `source_complete_owner_execution_pending`
- M6 snapshot-pair digest producer and mismatch-only recorder delegation: `source_complete`
- M6 missing-only entity candidate outcome: `source_complete_owner_execution_pending`
- M6 locale-optional persisted entity finding scope:
  `source_complete_owner_execution_pending`
- M6 source-version-fenced PostgreSQL drift snapshot reader:
  `source_complete_owner_execution_pending`
- M6 guarded exact-entity drift diagnosis operator:
  `source_complete_owner_execution_pending`
- M6 bounded GraphQL exact-entity diagnosis transport:
  `source_complete_owner_execution_pending`
- M6 bounded source-page missing-entity diagnosis:
  `source_complete_transport_and_owner_execution_pending`
- M6 authenticated and confidential source continuation codec:
  `source_complete_server_composition_complete_transport_pending`
- M6 server-owned source continuation keyring and sealed page boundary:
  `source_complete_transport_and_owner_execution_pending`
- M6 explicit source absence watermark registry, Product provider, and reader fence:
  `source_complete_owner_execution_pending`
- M6 Product locale absence PostgreSQL harness:
  `source_ready_owner_execution_pending`
- M7 Product/ProductVariant/SalesChannel bounded replay graph:
  `source_complete_owner_execution_pending`

## M5 incremental ingestion

- [x] Add a source replay registry with bounded failure classification.
- [x] Add inbox deduplication and monotonic source versions.
- [x] Add a database-neutral mutation-source event registry and commit-before-ack orchestration.
- [ ] Register production owner event routes and compose a concrete broker consumer/acknowledger.
- [ ] Add batch transactions, retry classification, bounded backoff, dead-letter state, and lag
      metrics around production event routes.
- [ ] Retain crash-between-commit-and-ack and redelivery evidence.

## M6 rebuild, reconciliation, and repair

- [x] Add bounded source scan/targeted-load contracts and stable replay event identities.
- [x] Add durable replay jobs, leases, heartbeats, attempt fences, and checkpoint progression.
- [x] Add bounded multi-page replay, durable cancellation, and bounded multi-pass reconciliation.
- [x] Add production source-call timeouts, bounded no-write dry-run, and cooperative page
      interruption safe points.
- [ ] Bind interruption to active runner lease/cancellation state and already-pending futures.
- [ ] Add targeted, full, and shadow rebuild modes.
- [x] Add bounded retry/backoff transitions, failed-scope dead-letter admission/inspection,
      authorized requeue, and generic host scheduling ownership.
- [ ] Retain multi-host scheduler, retry/dead-letter, restart, graceful-shutdown, and command-
      transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [x] Add bounded drift-finding inspection and persistence for already-computed digest mismatches.
- [x] Add real-migration PostgreSQL finding writer and locale-scope harnesses.
- [ ] Run and admit retained finding writer and locale-scope evidence.
- [x] Add one database-neutral producer for a validated exact source/materialized snapshot pair,
      deterministic SHA-256 digests, and mismatch-only recorder delegation.
- [x] Add one production `PostgresIndexDriftSnapshotReader` that fences exact owner state around one
      `REPEATABLE READ READ ONLY` materialized snapshot.
- [x] Compose the snapshot reader, digest producer, and finding writer behind one request-bound
      `modules:manage` exact-entity diagnosis capability.
- [x] Reject cross-tenant and unauthorized diagnosis before validation or dependency access.
- [x] Add the explicit retained absence/tombstone watermark registry with canonical source-owner
      parity, exact `EntityKey`, and positive source version.
- [x] Register Product locale absence for Product schema versions 1 and 2 and fence it around the
      materialized snapshot.
- [x] Add the source-ready Product locale absence and deterministic concurrent translation-change
      PostgreSQL scenarios.
- [ ] Run and admit `product_locale_absence_postgres` evidence.
- [x] Expose exact-entity diagnosis through bounded GraphQL `diagnoseIndexEntity` with tenant/actor
      derived from request context and authorization before untrusted identity parsing.
- [ ] Retain GraphQL authorization, malformed-input ordering, consistent result, mismatch receipt,
      and dependency-failure execution evidence.
- [x] Add a database-neutral missing-only selector over one validated `IndexDriftSnapshotPair`.
      Record only source `Upsert` plus materialized `Missing`; return `NotCandidate` for every other
      typed-state combination without recorder access or raw-state output.
- [x] Add one internal server-owned source-page missing-entity diagnosis runtime with a maximum page
      size of 32, sequential exact diagnosis, retained-delete skipping, and no loop, checkpoint,
      scheduler, task, repair, or public transport.
- [x] Keep the raw source cursor server-owned and return no source entity identifiers, payloads, or
      captured state through the raw internal page outcome.
- [x] Add an authenticated and confidential transport-neutral source continuation codec using
      AES-256-GCM, a fresh 96-bit nonce, bounded URL-safe envelope, exact tenant/schema/canonical
      source binding, authenticated lifetime, bounded clock skew, and one active plus retained
      decrypt-only rotation keys.
- [x] Reject token tampering, unsupported version, scope mismatch, expiry, oversized input, invalid
      claims, and unavailable or retired key material before returning raw cursor state.
- [x] Compose a server-owned continuation keyring from bounded secret references without embedding
      raw keys in settings, logs, GraphQL, or separately retrievable extension output.
- [x] Validate active-key presence, unique references, resolver policy, key count, and lifetime at
      synchronous composition; resolve and validate exactly 32-byte key material before token parsing
      or source scan inside the sealed request.
- [x] Add a sealed internal page method that opens an incoming continuation before constructing
      `IndexSourceScanRequest`, calls the existing one-page path exactly once, and seals the outgoing
      continuation before returning from the service boundary.
- [x] Return only bounded current-page counters, missing-finding receipts, and one opaque token from
      `IndexDriftSourcePageDiagnosisSealedOutcome`; expose no raw cursor or entity identifier.
- [ ] Add one bounded source-page transport over the sealed method only.
- [ ] Retain authorization, secret resolution, rotation, expiry, and sealed-result execution evidence.
- [ ] Add bounded stale Index-only entity and orphan-link diagnosis without unbounded ID collection.
- [ ] Add resolve/ignore lifecycle commands with actor/reason audit and fail-closed authorization.
- [ ] Add targeted repair with before/after admitted evidence.

## M7 production graph and cutover

- [x] Add Product, ProductVariant, and SalesChannel schemas and bounded current-state sources.
- [x] Add stable Product/ProductVariant/SalesChannel replay identities and retained deletes.
- [x] Add Product-to-ProductVariant graph materialization.
- [ ] Add production owner event routes and concrete incremental consumer wiring.
- [ ] Persist and enforce per-tenant schema readiness.
- [ ] Complete durable Product-to-SalesChannel relation semantics and retained evidence.
- [ ] Admit tombstone purge, freshness/outage/restart/backlog recovery, and delete/recreate evidence.
- [ ] Admit live PostgreSQL/reference query equivalence and one full partition packet.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## Next implementation step

Add one bounded source-page transport that delegates only to
`diagnose_source_page_sealed(context, schema, continuation, limit)`.

The transport must:

- derive tenant and actor only from authenticated request context;
- require effective `modules:manage` before parsing module/entity/version, limit, or token;
- accept one exact schema identity, one optional opaque token, and a limit in `1..=32`;
- contain no caller-selected tenant, actor, source name, owner module, cursor JSON, entity ID list,
  batch, checkpoint, scheduler, lifecycle, or repair input;
- delegate exactly once to the sealed internal method;
- return only current-page aggregate counters, bounded missing-finding receipts, completion state,
  and one optional opaque continuation token;
- expose fixed bounded dependency error codes without resolver causes, secret references, raw cursor
  state, source records, entity identifiers, SQL, or database causes.

Keep the historical raw page method internal and do not mount it. Do not add multi-page iteration,
persist checkpoints, schedule scans, enumerate stale Index-only rows, inspect orphan links, change
finding lifecycle, or repair state. PostgreSQL, cryptographic integration, and GraphQL execution
evidence remain owner-owned and pending.

## Owner verification for this slice

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-index source_absence -- --nocapture
cargo test -p rustok-distribution product_index --features mod-product -- --nocapture
cargo test -p rustok-server index_source_continuation_runtime -- --nocapture
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution \
  --features mod-product \
  --test product_locale_absence_postgres \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-source-continuation.mjs
node scripts/verify/verify-index-source-continuation-server.mjs
node scripts/verify/verify-index-drift-diagnosis-graphql-transport.mjs
node scripts/verify/verify-index-drift-source-page-diagnosis.mjs
node scripts/verify/verify-index-product-absence-postgres-harness.mjs
node scripts/verify/verify-index-source-absence-watermark.mjs
node scripts/verify/verify-index-server-reconciliation-guard.mjs
node scripts/verify/verify-index-drift-snapshot-reader.mjs
node scripts/verify/verify-index-drift-digest-producer.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-server --all-targets --features mod-product
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, PostgreSQL, cryptographic integration, or GraphQL runs,
workflows, or CI were executed by the implementation agent.