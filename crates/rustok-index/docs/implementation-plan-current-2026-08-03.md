# Current `rustok-index` implementation plan — 2026-08-03

Status overlay for `implementation-plan.md` rechecked through
`main@5bbaaaf6cb17b846a5c6c280b8b2501c229cdc64` and the active draft PR #2986.
The forty main commits after this branch merge base touch Commerce diagnostics, Forum GraphQL/admin
and route ownership, Pages/Page Builder delivery/routes/evidence, event-delivery settings, and
supporting server configuration. They do not touch `crates/rustok-index`, Product Index composition,
the server Index GraphQL files, Index diagnosis/page composition, or Index guards changed by this
branch. `apps/server/Cargo.toml` and general server settings changed on `main`, but this PR does not
modify those files in the continuation-codec slice.

When the older canonical plan's current-state bullets conflict with this dated overlay, this
overlay is the rechecked source of truth. Historical architecture, ownership, and milestone
details remain in `implementation-plan.md`.

## Current cursor

`M6 - compose source-continuation keys and seal the internal page boundary`

The database-neutral digest producer, mismatch-only writer adapter, locale-complete finding scope,
source-version-fenced PostgreSQL snapshot reader, guarded exact-entity diagnosis capability,
explicit owner-retained absence registry, Product locale high-watermark provider, double-read
absence-version fence, Product locale absence PostgreSQL harness, bounded GraphQL exact-entity
transport, missing-only entity candidate outcome, internal one-page missing-entity diagnosis
runtime, and authenticated/confidential source continuation codec are source complete.

`IndexSourceContinuationCodec` now encrypts the complete raw owner cursor with AES-256-GCM and binds
it to tenant, exact `SchemaRef`, canonical owner/source identity, version, issued-at, and expiry. It
supports one active sealing key plus retained decrypt-only rotation keys. Tampering, scope mismatch,
unsupported version, expiry, future issuance beyond bounded skew, oversized input, and unavailable
key material fail before raw cursor return.

Server key configuration, secret resolution, a sealed page-runtime method, public source-page
transport, owner execution evidence, finding lifecycle commands, repair, and broader Index-only or
orphan discovery remain open.

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
- M6 snapshot-pair digest producer and mismatch-only recorder delegation:
  `source_complete`
- M6 missing-only entity candidate outcome:
  `source_complete_owner_execution_pending`
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
  `source_complete_server_key_composition_pending`
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
- [x] Add a real-migration PostgreSQL harness for deterministic finding-key serialization and
      create/refresh/reopen/suppression lifecycle preservation.
- [ ] Run and admit `drift_finding_writer_postgres_test` evidence.
- [x] Add a database-neutral producer for one exact source/materialized snapshot pair with one
      bounded consistency token, exact scope revalidation, typed-state validation, deterministic
      SHA-256 production, and mismatch-only recorder delegation.
- [x] Adapt producer mismatches to `PostgresIndexDriftFindingWriter` with bounded retryable/permanent
      failure classification.
- [x] Extend persisted entity finding scope to locale-free `EntityKey` values without changing
      existing locale-bearing finding identities.
- [x] Add a forward migration, source-only key-compatibility contract, and environment-gated
      PostgreSQL writer/inspector harness for locale-free entity findings.
- [ ] Run and admit `drift_finding_locale_scope_postgres_test` evidence.
- [x] Add one production `PostgresIndexDriftSnapshotReader` that fences an exact positive-version
      owner state around one `REPEATABLE READ READ ONLY` materialized PostgreSQL snapshot.
- [x] Reconstruct exact materialized entity/delete/link state, validate registered fingerprints and
      ordinals, and reject owner state changes or unwatermarked absence.
- [x] Add an environment-gated real-migration PostgreSQL harness for stable capture, source-change
      rejection, and missing-watermark rejection.
- [ ] Run and admit `drift_snapshot_reader_postgres_test` evidence.
- [x] Compose the snapshot reader, digest producer, and finding writer behind one request-bound
      `modules:manage` exact-entity diagnosis capability using the frozen source/schema registries.
- [x] Reject cross-tenant and unauthorized diagnosis before request validation, source access,
      materialized reads, digest production, or finding persistence.
- [x] Add the explicit retained absence/tombstone watermark contract as an optional owner-published
      registry with exact `EntityKey`, positive source version, stable schema-identity ownership,
      canonical replay-source owner parity, and fail-closed materialization.
- [x] Keep existing `IndexSource::scan` and `IndexSource::load` contracts source-compatible and keep
      `None` or an empty targeted load non-authoritative.
- [x] Register the Product locale absence provider for `rustok-product::product@1` and `@2`, using
      positive `products.index_revision` only when the exact translation and tombstone are absent.
- [x] Materialize and privately attach the frozen absence registry to the PostgreSQL drift snapshot
      reader from guarded server diagnosis composition.
- [x] Reload and compare the exact positive absence version around the materialized snapshot and
      bind the version into the opaque boundary only for source `Missing`.
- [x] Preserve permanent `index_drift_source_watermark_missing` when provider registration or
      authoritative evidence is unavailable.
- [x] Add the source-ready real-migration Product locale-absence scenario and deterministic
      concurrent translation-change rejection scenario in `product_locale_absence_postgres`.
- [ ] Run and admit `product_locale_absence_postgres` evidence.
- [x] Expose exact-entity diagnosis through bounded GraphQL `diagnoseIndexEntity` with tenant/actor
      derived from request context, authorization-before-identity-parsing, one exact key, bounded
      digest/receipt output, and no batch, scan, lifecycle, scheduler, or repair authority.
- [ ] Retain GraphQL authorization, malformed-input ordering, consistent result, mismatch receipt,
      and dependency-failure execution evidence.
- [x] Add a database-neutral missing-only selector over one validated
      `IndexDriftSnapshotPair`. Record only source `Upsert` plus materialized `Missing`; return
      `NotCandidate` for every other typed-state combination without recorder access or raw-state
      output; preserve general `produce(request)` behavior.
- [x] Add one internal server-owned source-page missing-entity diagnosis runtime over the frozen
      `IndexSource::scan` registry with authorization-before-page-validation, a maximum page size of
      32, sequential missing-only diagnosis of source `Upsert` candidates, retained-delete skipping,
      and no loop, checkpoint store, scheduler, task, repair, or public transport.
- [x] Keep source-page continuation server-owned and return no source entity identifiers, payloads,
      or captured state through the page outcome.
- [x] Add an authenticated and confidential transport-neutral source continuation codec using
      AES-256-GCM, a fresh 96-bit nonce, bounded URL-safe envelope, exact tenant/schema/canonical
      source binding, authenticated lifetime, bounded clock skew, and one active plus retained
      decrypt-only rotation keys.
- [x] Reject token tampering, unsupported version, scope mismatch, expiry, oversized input, invalid
      claims, and unavailable or retired key material before returning raw cursor state.
- [ ] Compose a server-owned continuation keyring from secret references without embedding raw keys
      in settings, logs, GraphQL, or `ModuleRuntimeExtensions` debug output.
- [ ] Add a sealed internal page method that opens an incoming continuation before constructing
      `IndexSourceScanRequest` and seals the outgoing continuation before returning from the service
      boundary.
- [ ] Add a bounded source-page transport only after authorization ordering, secret resolution,
      rotation, expiry, and sealed-result behavior are explicitly admitted.
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

Compose one server-owned continuation keyring from bounded key identifiers and secret references. The
server must resolve 32-byte AES keys through the existing secret resolver boundary, publish only the
opaque codec capability, and redact key material from errors and `Debug`. Composition must fail
closed when the active key is absent, a key reference cannot be resolved, decoded material is not
exactly 32 bytes, or more than the bounded key count is configured.

Then add one internal sealed page method. It must derive canonical scope from the frozen source
registry, authorize before parsing an incoming token, open the token before constructing or invoking
`IndexSourceScanRequest`, call the existing one-page missing-only path exactly once, and seal an
outgoing continuation before returning it. The service result may contain the sealed token but never
raw `IndexSourceCursor` JSON.

Do not mount GraphQL, HTTP, CLI, MCP, or native admin in that slice. Do not add multi-page iteration,
persist checkpoints, schedule scans, enumerate stale Index-only rows, inspect orphan links, change
finding lifecycle, or repair state. PostgreSQL, cryptographic integration, and GraphQL execution
evidence remain owner-owned and pending.

## Owner verification for this slice

```bash
cargo test -p rustok-index source_continuation -- --nocapture
cargo test -p rustok-index drift_digest -- --nocapture
cargo test -p rustok-index source_absence -- --nocapture
cargo test -p rustok-distribution product_index --features mod-product -- --nocapture
cargo test -p rustok-server index_drift_diagnosis -- --nocapture
cargo test -p rustok-server index_drift_source_page_diagnosis -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture

RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution \
  --features mod-product \
  --test product_locale_absence_postgres \
  -- --nocapture --test-threads=1

node scripts/verify/verify-index-source-continuation.mjs
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
