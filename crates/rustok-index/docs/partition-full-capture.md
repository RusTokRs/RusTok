# Index M3 full retained partition capture

Status:

- M3 partition cutover rehearsal evidence runner: `complete`.
- M3 retained packet owner orchestration: `complete`.
- Real retained PostgreSQL packet execution: `open`.
- Production partition copy, replay, dual-write, cutover, rollback automation, cleanup, and query-adapter work: `forbidden before one retained admitted packet`.

## Owner command

Use one fresh immutable manifest and an empty bundle directory. The command refuses partial-output reuse and does not resume a failed attempt.

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_FULL_CAPTURE=1 \
node scripts/verify/index-storage-tooling.mjs partition-capture \
  --manifest evidence/index-partition/manifest.json \
  --query-audit evidence/index-partition/query-audit.json \
  --root evidence/index-partition/retained-run
```

The orchestration command runs, in order:

1. `index-partition-snapshot-capture`;
2. `index-partition-query-evidence`;
3. `index-partition-mutation-evidence`;
4. `index-partition-maintenance-evidence`;
5. `index-partition-cutover-evidence`;
6. `index-partition-capture-finalize`;
7. exact-byte packet assembly;
8. admission validation.

`index-partition-capture-finalize` verifies all six retained raw files, reads the PostgreSQL 16 server version, JIT state, `pg_control_system()` system identifier, and current database name, and publishes `capture.json` once with runner provenance bound to the immutable manifest.

The final retained directory contains `baseline.json`, `shadow.json`, `query.json`, `mutation.json`, `maintenance.json`, `cutover.json`, `capture.json`, `partition-packet.json`, and `admission.json`.

A failed attempt may leave evidence schemas or raw artifacts for inspection. Do not edit or reuse them. Prepare a fresh manifest run key and a new empty directory.

This tooling does not authorize or implement production relation rename/drop, copy/replay, dual-write, cutover, cleanup, or query-adapter changes. Admission remains a measured owner decision based on the retained packet.
