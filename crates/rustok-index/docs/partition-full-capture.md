# Index M3 full retained partition capture

Status:

- M3 partition cutover rehearsal evidence runner: `complete`.
- M3 retained packet owner orchestration: `complete`.
- Real retained PostgreSQL packet execution: `open`.
- Production partition copy, replay, dual-write, cutover, rollback automation, cleanup, and query-adapter work: `forbidden before one retained admitted packet`.

## No-write preflight plan

Run the same owner command with `--plan` before starting a fresh retained capture:

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_FULL_CAPTURE=1 \
node scripts/verify/index-storage-tooling.mjs partition-capture --plan \
  --manifest evidence/index-partition/manifest.json \
  --query-audit evidence/index-partition/query-audit.json \
  --root evidence/index-partition/retained-run
```

The plan requires the same `DATABASE_URL` presence and full-capture opt-in as the real command. It validates that the manifest and query audit are non-empty regular files, resolves every retained output inside the bundle root, rejects partial-output reuse, and prints the exact eight-stage execution contract as JSON.

Plan mode does not open a PostgreSQL connection, does not start Cargo or Node evidence stages, does not create the bundle directory or any output file, and does not print the `DATABASE_URL` value. A successful plan confirms only local environment and filesystem readiness; PostgreSQL 16 identity, JIT state, and measured evidence remain runtime checks.

## Owner command

Use one fresh immutable manifest and an empty bundle directory. The command refuses partial-output reuse and does not resume a failed attempt. After reviewing the no-write plan, rerun the same command without `--plan`:

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

## Owner review and archive report

After a successful capture, render a read-only review of all nine retained files:

```bash
node scripts/verify/index-storage-tooling.mjs partition-report \
  --root evidence/index-partition/retained-run
```

`partition-report` requires a regular non-symlink bundle and keeps `capture.json`, the packet, the admission file, and all six raw artifacts inside that bundle. It recalculates packet assembly and admission from the retained bytes, rejects raw-artifact, packet, or admission drift, checks that all nine files have distinct filesystem identities, and prints a stable Markdown inventory with exact-byte SHA-256 digests, PostgreSQL identity, provenance, calculated measurements, outcome, and typed reasons.

The report command writes no files, opens no PostgreSQL connection, and starts no Cargo or evidence stage. Shell redirection may save the derived report outside the immutable bundle, but the six raw artifacts, `capture.json`, `partition-packet.json`, and `admission.json` remain the authoritative archive inputs. A report does not change admission and does not authorize production lifecycle work.

A failed attempt may leave evidence schemas or raw artifacts for inspection. Do not edit or reuse them. Prepare a fresh manifest run key and a new empty directory.

This tooling does not authorize or implement production relation rename/drop, copy/replay, dual-write, cutover, cleanup, or query-adapter changes. Admission remains a measured owner decision based on the retained packet.
