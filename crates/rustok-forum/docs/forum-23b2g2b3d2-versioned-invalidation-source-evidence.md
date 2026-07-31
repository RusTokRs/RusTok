# FORUM-23B2G2B3D2 versioned invalidation source evidence

Status: `source_ready_maintainer_execution_pending`

## Purpose

This slice continues the accepted `FORUM-23B2G2B3D0` executable evidence protocol after the canonical `FORUM-23` plan reconciliation in D1. It supplies two bounded source-ready proofs without claiming that PostgreSQL, an external Iggy broker, the server worker, DLQ publication or the final cross-module scenarios were executed.

D2 contains:

1. a clean PostgreSQL one-inbox ingress harness; and
2. a transport-level persistent cursor acknowledgement-failure and reconstruction proof.

## PostgreSQL ingress proof

`crates/rustok-search/tests/forum_contract_ingress_postgres_test.rs` uses every real `SearchModule` migration inside a unique PostgreSQL schema per case. It covers:

- legacy-root delivery before the caused typed invalidation;
- caused typed invalidation before legacy-root delivery;
- one durable `search_projection_inbox` row in both orders;
- retention of the first positive Search-owned `ingest_sequence`;
- typed redelivery through a newly constructed `ForumSearchContractIngress`;
- independence of Forum `owner_revision` from Search `ingest_sequence`;
- exact tenant, scope and root-envelope duplicate recognition;
- a conflicting retained root UUID as stable non-retryable semantic poison without rewriting the durable row.

The target reads `RUSTOK_SEARCH_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as fallback. Without a PostgreSQL URL it reports a skip and succeeds. It uses no sleep, polling, lease expiry or second projection path.

## Persistent cursor restart proof

`crates/rustok-iggy/src/contract_consumer_restart_tests.rs` is a crate-local test module so it can construct the same production `PersistentContractConsumerGroup` around a deterministic connector cursor.

The cursor state is shared across two separately constructed consumer-group instances. The first acknowledgement is injected to fail. The proof requires:

- the exact offset to remain uncommitted after acknowledgement failure;
- a reconstructed group to receive the same bytes, partition, offset and event identity;
- a second acknowledgement to commit that exact position;
- no delivery after the successful commit;
- the same behavior for undecodable raw bytes, retaining the deterministic poison delivery ID and stable decode classification across reconstruction.

This is a transport ownership proof. It does not simulate durable server poison receipts or claim an external broker restart.

## Single execution path

D2 changes no Forum or Search production execution policy. Typed and legacy deliveries still converge on the existing root event identity and one `search_projection_inbox` row. The existing Forum Search reconciler and projector remain the only projection lane.

No second inbox, projector, reconciler, watermark or ordering clock is introduced.

## Remaining runtime evidence

`FORUM-23B2G2B3D` remains open for maintainer-executed evidence covering:

- successful PostgreSQL D2 output capture;
- external persistent Iggy cursor and server-worker restart;
- connector-owned raw and semantic poison receipts;
- deterministic DLQ publication, acknowledgement failure and redelivery;
- DLQ duplicate suppression;
- missing-delivery owner-checkpoint repair;
- multi-process serialization;
- deletion, ACL and Search-disabled correlation;
- complete `LINK-FORUM-03` closure.

The next source slice is `FORUM-23B2G2B3D3`, focused only on durable poison receipt and DLQ redelivery evidence.

## Maintainer validation

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-search \
  --test forum_contract_ingress_postgres_test \
  -- --nocapture --test-threads=1

cargo test -p rustok-iggy contract_consumer::restart_tests -- --nocapture
cargo check -p rustok-search --all-targets
cargo check -p rustok-iggy --all-targets
node scripts/verify/verify-forum-search-versioned-invalidation-d2-source-harness.mjs
node scripts/verify/verify-forum-search-versioned-invalidation-runtime-evidence.mjs
cargo xtask module validate forum
cargo xtask module validate search
```

These commands were not run by the implementation agent.
