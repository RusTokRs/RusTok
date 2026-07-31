# FORUM-23B2G2B3D2 PostgreSQL shared-inbox proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds the first executable subset of the
`FORUM-23B2G2B3D` runtime-evidence matrix. It exercises the Search-owned
PostgreSQL ingress boundary directly and generates a retained JSON evidence
artifact after all covered scenarios pass.

The machine-readable proof contract is:

```text
crates/rustok-forum/contracts/forum-search-versioned-invalidation-postgres-ingress-proof.json
```

The executable test is:

```text
crates/rustok-search/tests/forum_versioned_invalidation_postgres.rs
```

## Covered scenarios

The test creates an isolated PostgreSQL schema, applies the real `SearchModule`
migrations and covers four durable-admission cases:

1. a causation-bound typed invalidation creates one shared inbox row keyed by the
   exact legacy root ID and receives one positive Search `ingest_sequence`;
2. legacy-first delivery is recognized as an exact durable duplicate without
   replacing the root envelope or allocating a second sequence;
3. typed-first delivery remains the one durable row when the legacy root arrives
   later through the same `ON CONFLICT (event_id) DO NOTHING` boundary;
4. a colliding root ID with different tenant, scope and payload identity fails
   with `forum.search_projection.contract_inbox_identity_conflict` and leaves the
   conflicting row unchanged.

The proof compares complete stored root-envelope identity, not UUID alone. The
typed transport envelope ID is recorded separately and must differ from the
shared projection identity.

## Generated evidence

After all scenarios pass and the isolated schema is cleaned up, the test writes:

```text
target/forum-search-versioned-invalidation-postgres-ingress-evidence.json
```

The artifact records the exact `git rev-parse HEAD` source commit, generation
time, PostgreSQL backend, scenario results, tenant/root/typed IDs, scope keys,
positive ingest sequences, row counts and the stable conflict code. It is test
output and must not be hand-edited or committed as a static fixture.

If no PostgreSQL URL is configured, the test follows the repository's existing
PostgreSQL test convention and skips without generating evidence.

## Deliberate boundary

D2 proves Search durable ingress and duplicate identity only. It does not start
Iggy or the server worker and therefore does not claim:

- source acknowledgement or persistent cursor restart;
- connector-owned poison receipts or deterministic DLQ publication;
- projection execution or owner-checkpoint advancement;
- missing-delivery repair, multi-process advisory-lock serialization,
  deletion/ACL ordering or Search-disabled recovery;
- completion of `FORUM-23B2G2B3D` or `LINK-FORUM-03`.

Those scenarios remain separate executable follow-up slices. This separation
prevents a PostgreSQL adapter test from being presented as broker or end-to-end
evidence.

## Compatibility and degraded mode

No production Rust path, migration, event schema, digest, public DTO, Search
query, broker configuration, dependency or `Cargo.lock` entry changes. The
legacy root remains mandatory, the typed consumer remains default-off and
SQLite remains validation-only for this persistent ingress boundary.

## Maintainer verification

```bash
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" \
  cargo test -p rustok-search --test forum_versioned_invalidation_postgres \
  -- --nocapture --test-threads=1
node scripts/verify/verify-forum-search-versioned-invalidation-postgres-ingress-proof.mjs
cargo xtask module validate forum
cargo xtask module validate search
git diff --check
```

No command above was run by the implementation agent, per maintainer request.
