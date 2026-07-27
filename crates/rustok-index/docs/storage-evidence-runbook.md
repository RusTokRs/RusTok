# Index M2 replacement evidence runbook

This runbook records the owner-operated M2 replacement evidence procedure that
produced the accepted storage decision. It is retained as reproducibility history;
new JSONB-only regression runs do not reopen or replace the archived decision run.

## Trigger model

Pull requests run only the lightweight contract job. That job checks the
JavaScript syntax for the Index verification surface, the reusable workflow
input contract, repository guards, direct validator arguments, and the canonical
fixture suites. It does not allocate PostgreSQL or a heavy runner.

Heavy replacement evidence runs only through an explicit `workflow_dispatch`.
Select the exact branch, tag, or commit that should own both replacement packets.
One dispatch uses one selected Git ref and fans out `100k` and `1m` from the same checkout SHA.
The comparison job starts only after both validated scale jobs succeed.

The scale workflow intentionally has no automatic push trigger. This prevents an
ordinary branch update from unexpectedly allocating the large runner or creating
partial replacement evidence.

## Before dispatch

1. Select an immutable commit that contains every benchmark, SQL, verifier, and
   workflow correction intended for the decision run.
2. Confirm that no later code change is expected to alter candidate DDL, dataset
   generation, workload SQL, report shape, provenance, or validation.
3. Configure `INDEX_BENCH_LARGE_RUNNER` when a larger runner is available. When it
   is absent, the `1m` job uses `ubuntu-latest` but still fails before building if
   the root filesystem has less than 35,000,000,000 free bytes.
4. Start **Index Storage Scale Evidence** manually and select the same ref reviewed
   in step 1.

## Expected job graph

The dispatch executes this fail-closed sequence:

1. `contract` on `ubuntu-slim`;
2. independent `evidence-100k` and `evidence-1m` reusable jobs;
3. `comparison` only after both scale jobs succeed.

Each scale job builds the three release executables, captures runner resources,
runs read, mutation, and maintenance evidence, validates the complete packet, and
uploads the validated directory. The comparison job downloads both packets by the
same `${{ github.sha }}` artifact identity, runs the official comparator wrapper,
requires `decision_ready: true`, and uploads the comparison directory.

## Artifact identity and retention

Successful evidence and comparison artifacts are retained for 90 days. Their
canonical names are:

- `index-storage-100k-<full commit SHA>`;
- `index-storage-1m-<full commit SHA>`;
- `index-storage-comparison-<full commit SHA>`.

Failure diagnostics are retained for 14 days and are not decision evidence. A
failed or cancelled scale job prevents the comparison job from running.

Do not combine artifacts from different run IDs or commit SHAs. Do not rename a
diagnostic artifact into a successful evidence identity. The report-level run
provenance and packet validator remain authoritative even when artifact names
look correct.

## Archival handoff

Before the 90-day retention window expires:

1. download all three successful artifacts from one workflow run;
2. preserve their directory layout as `100k`, `1m`, and `comparison`;
3. record the workflow run ID, exact full commit SHA, PostgreSQL image,
   repetitions, churn cycles, and runner labels in the evidence review;
4. re-run the canonical packet and comparison commands against the downloaded
   directories before committing or otherwise retaining them as project evidence;
5. review `comparison.md` and `comparison.json` together before preparing the
   manual decision.

The official commands are:

```bash
node scripts/verify/index-storage-tooling.mjs packet \
  --scale 100k --root evidence/index-storage/100k
node scripts/verify/index-storage-tooling.mjs packet \
  --scale 1m --root evidence/index-storage/1m
node scripts/verify/index-storage-tooling.mjs compare \
  --input evidence/index-storage/100k \
  --input evidence/index-storage/1m \
  --output evidence/index-storage/comparison
```

Regenerating the comparison is expected to revoke a stale comparison success
marker before validation and to publish `comparison.json` last. A regenerated
comparison must remain byte-consistent with the reviewed evidence contract before
it is used to prepare a decision.

## Completion boundary

The repository owner performs the benchmark and validation commands. A workflow
success alone is not permission to select a model. Reviewers must compare buffers,
planner stability, latency, ingestion, relation size, WAL, dead tuples, VACUUM,
and operational complexity across both scales.

The canonical replacement run is `30222913450` on commit
`eae5f74241e9431bffe2fd8c43cd046fc1c1f679`. Both validated packets, the
decision-ready comparison, and the accepted decision are archived under
`docs/evidence/2026-07-27-postgresql-storage/`. The accepted ADR selects JSONB.
The rejected typed-EAV and hot-projection benchmark implementations and schemas
are deleted. M2 is complete. The remaining JSONB-only runner may be used for
selected-layout regression evidence while M3 implements production persistence.
