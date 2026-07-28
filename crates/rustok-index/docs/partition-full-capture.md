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

## Machine-readable admitted archive manifest

After the report is reviewed and the retained outcome is `admitted`, print a deterministic machine-readable archive manifest. Save the derived manifest outside the immutable bundle:

```bash
node scripts/verify/index-storage-tooling.mjs partition-archive-manifest \
  --root evidence/index-partition/retained-run \
  > evidence/index-partition/retained-run.archive-manifest.json
```

`partition-archive-manifest` runs the same retained-bundle inspection as `partition-report`, refuses any outcome other than `admitted`, and prints JSON containing the evidence identity, packet digest, provenance, PostgreSQL identity, all nine relative paths, exact-byte SHA-256 digests, byte counts, and total retained bytes. Its `manifest_digest` is the SHA-256 digest of canonical JSON for the manifest payload before the `manifest_digest` field is added, under `canonical_json_without_manifest_digest_v1`.

The archive-manifest command writes no files, opens no PostgreSQL connection, and starts no Cargo or evidence stage. Shell redirection may save this derived index outside the immutable bundle. The manifest does not replace or modify the nine authoritative retained files, does not change admission, and does not authorize production lifecycle work.

## Verify a saved archive manifest

Before moving or accepting an archived packet, verify the saved manifest against the current immutable bundle:

```bash
node scripts/verify/index-storage-tooling.mjs partition-archive-verify \
  --root evidence/index-partition/retained-run \
  --manifest evidence/index-partition/retained-run.archive-manifest.json
```

`partition-archive-verify` requires the saved manifest to be a non-empty regular non-symlink file outside the immutable bundle. It rejects lexical or canonical paths inside the bundle and rejects a hard-link alias to any of the nine retained files. It reads the saved manifest through one stable file descriptor, verifies the saved `manifest_digest`, reruns the full retained-bundle inspection, rebuilds the admitted archive manifest, and canonical-compares the complete saved manifest to the recalculated result.

Before publishing success, the verifier rereads all nine retained files through stable file descriptors and compares their current byte counts and exact-byte SHA-256 digests to the completed inspection. It fails closed on post-inspection exact-byte drift, current retained-file aliasing, or identity/content drift while a file is being read. This closes the gap where a bundle could change between inspection and receipt publication.

A successful verification prints a deterministic `index_partition_retained_archive_verification_v1` receipt with `retained_files_rechecked: true`, the evidence ID, packet and manifest digests, exact-byte SHA-256 of the saved manifest file, retained file count, total retained bytes, and `production_lifecycle_authorized: false`. The verifier writes no files, opens no PostgreSQL connection, and starts no Cargo or evidence stage. The receipt is derived metadata, not a tenth authoritative evidence input and not production authorization.

A failed attempt may leave evidence schemas or raw artifacts for inspection. Do not edit or reuse them. Prepare a fresh manifest run key and a new empty directory.

This tooling does not authorize or implement production relation rename/drop, copy/replay, dual-write, cutover, cleanup, or query-adapter changes. Admission remains a measured owner decision based on the retained packet.
