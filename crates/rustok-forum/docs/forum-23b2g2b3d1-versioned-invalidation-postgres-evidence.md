# FORUM-23B2G2B3D1 versioned Search invalidation PostgreSQL evidence harness

Status: `source_ready_maintainer_execution_pending`

## Purpose

This slice turns the PostgreSQL ingress subset of the accepted `FORUM-23B2G2B3D0` protocol into one reproducible, environment-gated integration target. It does not claim that the target was executed and does not replace the remaining persistent-Iggy evidence.

The harness uses the real `SearchModule` migration list inside a unique PostgreSQL schema for every case. It constructs a new `ForumSearchContractIngress` after durable admission to model process-local runtime reconstruction without relying on sleep, polling, lease expiry, wall-clock ordering or broker mocks.

## Covered evidence

The target proves the source path for:

- legacy-root delivery before the caused typed invalidation;
- caused typed invalidation before legacy-root delivery;
- one durable `search_projection_inbox` row in both orders;
- retention of the first Search-owned positive `ingest_sequence`;
- typed redelivery through a newly constructed ingress instance;
- exact tenant, scope and root-envelope duplicate recognition;
- fail-closed UUID collision handling when the retained root row has different scope or payload identity;
- stable non-retryable semantic-poison code `forum.search_projection.contract_inbox_identity_conflict`;
- independence of Forum `owner_revision` from Search `ingest_sequence`.

The harness inserts the legacy root only through the same physical `ON CONFLICT (event_id) DO NOTHING` collapse contract used by the existing inbox. The production adapter remains responsible for the post-conflict exact identity verification.

## Isolation and cleanup

The test reads `RUSTOK_SEARCH_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each case:

1. creates a uniquely named schema;
2. sets `search_path` to that schema plus `public`;
3. applies every real `SearchModule` migration;
4. runs one deterministic delivery-order or identity-conflict scenario;
5. reads the durable inbox row directly;
6. drops the isolated schema.

No production migration, runtime flag, consumer group, public API or projection behavior changes in this slice.

## Remaining FORUM-23B2G2B3D evidence

`FORUM-23B2G2B3D` remains open for maintainer-executed evidence covering:

- successful PostgreSQL harness output capture;
- persistent Iggy cursor restart before and after acknowledgement;
- acknowledgement failure after durable inbox admission;
- raw and semantic poison receipt, DLQ publication and redelivery;
- deterministic DLQ duplicate suppression;
- owner-checkpoint repair after missed delivery;
- the complete `LINK-FORUM-03` publish/moderate/delete/ACL ordering proof.

## Maintainer validation

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-search \
  --test forum_contract_ingress_postgres_test \
  -- --nocapture --test-threads=1

cargo check -p rustok-search --all-targets
node scripts/verify/verify-forum-search-versioned-invalidation-postgres-harness.mjs
```

These commands were not run by the implementation agent.
