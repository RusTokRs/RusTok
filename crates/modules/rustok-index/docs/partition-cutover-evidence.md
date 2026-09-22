# Index M3 partition cutover rehearsal evidence

This owner-operated tool captures the final raw `cutover.json` artifact required by
`index_partition_capture_v1`. It is evidence-only. It does not implement or authorize
production partition cutover, replay, dual-write, cleanup, or durable operation ownership.

## Safety boundary

The runner requires `INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE=1`. It independently
revalidates the immutable prepared manifest, PostgreSQL 16 session settings, ordinary
canonical relations, and the retained evidence-bound shadow catalog.

Each rehearsal:

1. acquires `ACCESS EXCLUSIVE` locks on canonical `index_entities`/`index_links` and
   their retained shadow parents and records the acquisition duration as `lock_ms`;
2. performs rename choreography only on four empty clones in a deterministic
   `index_pe_cutover_<evidence>` schema;
3. rolls the transaction back and verifies the clone OIDs return to their original names;
4. recalculates canonical/shadow logical digests and catalog identities to prove all
   production and retained snapshot-shadow relations remained unchanged.

The evidence schema is retained for owner inspection. An existing schema or output file
fails closed. `cutover.json` is published through temporary-file plus hard-link no-clobber
semantics and contains exactly `manifest.repetitions.cutover` uniquely named runs.

## Owner command

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/rustok_index_evidence \
INDEX_PARTITION_ALLOW_CUTOVER_EVIDENCE=1 \
INDEX_PARTITION_MANIFEST=evidence/index-partition/manifest.json \
INDEX_PARTITION_EVIDENCE_ROOT=evidence/index-partition \
cargo run -p rustok-benchmarks --bin index-partition-cutover-evidence --release
```

The resulting top-level array supplies the packet-required `lock_ms`,
`rollback_verified`, and `production_relations_unchanged` fields, plus lock targets and
clone identity details. The repository owner still executes the real PostgreSQL run,
retains all six raw artifacts, assembles the packet, and validates admission.

## Suggested checks

```bash
cargo check -p rustok-benchmarks --bin index-partition-cutover-evidence
cargo test -p rustok-benchmarks partition_cutover
node scripts/verify/verify-index-partition-cutover-evidence.mjs
node scripts/verify/index-storage-tooling.mjs contract
```

These checks and the real PostgreSQL rehearsal are owner-run and are not executed by the
change author.
