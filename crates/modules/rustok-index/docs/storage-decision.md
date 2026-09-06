# Index PostgreSQL storage decision

The storage benchmark comparison is evidence, not an automatic model selector. After replacement `100k` and `1m` packets have been generated from the same commit and the official comparator reports `decision_ready: true`, maintainers record a manual decision and finalize the ADR.

## Tooling entrypoint

Use the stable command router instead of remembering individual script names:

```bash
# Static repository contracts; does not execute PostgreSQL benchmarks.
node scripts/verify/index-storage-tooling.mjs contract

# Comparator, decision, and ADR fixture suites.
node scripts/verify/index-storage-tooling.mjs fixtures

# Validate an already generated packet.
node scripts/verify/index-storage-tooling.mjs packet \
  --scale 100k \
  --root evidence/index-storage/100k

# Generate a same-commit cross-scale comparison.
node scripts/verify/index-storage-tooling.mjs compare \
  --input evidence/index-storage/100k \
  --input evidence/index-storage/1m \
  --output evidence/index-storage/comparison
```

The router dispatches Node directly without shell evaluation. It exposes the canonical static guards, packet validator, comparator, exact-byte hashing, decision preparation, ADR finalization, and saved-ADR verification paths.

Global `--help` and `-h` are accepted only as the sole router argument. The `packet` command rejects repeated `--scale` or `--root` options before invoking ordering checks or evidence validation. This prevents malformed scripted invocations from silently changing the effective evidence scale or packet root.

Only `comparison.json` emitted through the official comparator wrapper is valid decision input. Direct output from `compare-index-storage-evidence-core.mjs` is intentionally incomplete because it does not finalize the observed PostgreSQL database-settings methodology. Do not add the missing fields by hand.

A real comparison attempt revokes any previous `comparison.json` decision-input marker before packet ordering preflight. The comparator core and PostgreSQL methodology finalization run in a unique private staging directory. Finalized Markdown is published before `comparison.json`, so JSON is the last success marker exposed to decision preparation. Core failure, incomplete output, methodology failure, or publication failure leaves no decision-input JSON and no staging residue.

The accepted methodology contains the exact ordered `comparable_database_fields` contract:

- `server_version_num`;
- `shared_buffers`;
- `effective_cache_size`;
- `work_mem`;
- `random_page_cost`;
- `jit`;
- `standard_conforming_strings`;
- `timezone`;
- `date_style`;
- `extra_float_digits`.

Its `database_settings_source` must state that the comparison uses `read-report.json` database metadata observed from its active PostgreSQL benchmark session only after exact equality was verified against the active-session metadata archived in `mutation-report.json` and `maintenance-report.json`. The comparator rejects intra-packet or cross-scale drift in any field, and decision preparation, the direct renderer, and finalization all reject a comparison whose field list, order, or source string differs from the canonical contract. The former read-only source string is no longer valid decision methodology.

## Prepare the decision

Create a draft from the exact `comparison.json` that will be reviewed:

```bash
node scripts/verify/index-storage-tooling.mjs prepare \
  --comparison evidence/index-storage/comparison/comparison.json \
  --selected typed_eav \
  --owner "Index maintainers" \
  --date 2026-07-24 \
  --output evidence/index-storage/comparison/decision.json
```

`prepare` requires an explicit prototype choice. It does not rank candidates or select a winner. It validates the decision-ready comparison and its exact observed database-settings methodology, copies the evidence commit, computes the SHA-256 of the exact comparison-file bytes, creates rejection entries for exactly the two unselected prototypes, and refuses to overwrite an existing decision unless `--force` is provided. The draft is written to a staged file and renamed only after the complete JSON is on disk.

The generated draft has `status: proposed`. Change it to `accepted` only after the evidence and rationales have been reviewed. The finalizer rejects a proposed decision and accepts only a real `YYYY-MM-DD` calendar date with a year from `0001` through `9999`, including the correct Gregorian leap-year rules.

The preparation command accepts only `--comparison`, `--selected`, `--owner`, `--date`, and `--output`, plus one standalone `--force` flag. Help is valid only as the sole argument, and unknown, incomplete, mixed-help, or duplicate options fail before changing decision state. A valid forced replacement creates a unique same-directory staging location before withdrawing the stale draft, then validates the comparison and publishes the complete replacement with one rename. Failure after stale-draft withdrawal leaves no decision file and no staging residue.

The generated draft contains `TODO(index-storage-decision):` markers. Replace every marker with measured and operational reasoning before finalization. The finalizer rejects the exact marker at any position inside selection, rejection, operations, migration, or rollback rationale text.

[`storage-decision.example.json`](storage-decision.example.json) shows the same decision fields and references [`storage-decision.schema.json`](storage-decision.schema.json). Its relative `$schema` is valid because the two files are colocated in the documentation directory. A generated decision under `evidence/index-storage/...` intentionally omits `$schema` rather than recording a false relative path; `$schema` remains an optional finalizer field when it correctly points to a colocated schema file.

The decision schema requires at least one non-whitespace character in the owner and every selection, rejection, operational, migration, and rollback narrative. Whitespace-only text is rejected before a decision is treated as schema-valid. This keeps schema validation aligned with preparation, rendering, and finalization, which all trim required human-authored text before accepting it.

The example is intentionally not finalizable until its markers are replaced. The decision must explain:

- why the selected prototype is preferred;
- why each of the other two prototypes was rejected;
- operational trade-offs;
- migration strategy;
- rollback strategy.

`selected_prototype` must be one of `jsonb`, `typed_eav`, or `hot_projection`. `comparison_commit` must match the full Git commit recorded by both scale packets, and `comparison_sha256` must match the exact bytes of the reviewed `comparison.json`.

For an independent digest check:

```bash
node scripts/verify/index-storage-tooling.mjs hash \
  evidence/index-storage/comparison/comparison.json
```

Hash help is accepted only as the sole argument. Mixed help/path invocations and zero or multiple comparison paths fail without producing a digest. The helper hashes the exact file bytes without JSON normalization.

## Finalize the ADR

```bash
node scripts/verify/index-storage-tooling.mjs render \
  --comparison evidence/index-storage/comparison/comparison.json \
  --decision evidence/index-storage/comparison/decision.json \
  --output crates/rustok-index/docs/adr-postgresql-storage.md
```

Finalization snapshots the exact comparison and decision bytes before rendering. The generated ADR records both `Comparison SHA-256` and `Decision SHA-256`, so reviewers can verify the two source documents used to produce it.

The finalizer accepts only `--comparison`, `--decision`, and `--output`; help is valid only as the sole argument. Malformed command lines and output paths that collide with either input are non-destructive. A valid replacement attempt rejects any input path inside the output staging namespace, revokes the existing ADR and every matching stale staging path before evidence is read, then publishes through a unique same-directory staging directory. If the replacement evidence, accepted decision, renderer, or publication step fails, no stale ADR or staging residue is left behind.

The finalizer fails closed unless:

- the decision status is `accepted`;
- `decision_date` is a real ISO calendar date using Gregorian month and leap-year rules;
- the comparison contains the exact eight-field methodology envelope, including the ordered ten-field observed PostgreSQL database-settings contract;
- the comparison is decision-ready;
- every decision-contract flag is true;
- `100k` and `1m` evidence are present and share the same full commit;
- automatic winner selection is explicitly disabled;
- every displayed metric and cross-scale ratio is present and numeric;
- the decision identifies the same comparison commit;
- `comparison_sha256` matches the exact comparison-file bytes;
- no preparation placeholder remains anywhere in required rationale text;
- selection, rejection, operations, migration, and rollback rationales are all present.

The standalone renderer enforces the same methodology contract even when invoked directly. Recomputing `comparison_sha256` after removing or changing the methodology does not make the input acceptable.

The directly invocable renderer accepts help only as a sole argument and rejects unknown, incomplete, or duplicate options before changing files. Output collisions with comparison or decision inputs are also non-destructive. A real render attempt withdraws any stale output before evidence validation, writes into a unique same-directory staging location, and publishes the completed Markdown with one rename. Failure leaves neither an old final ADR nor staging residue. The stable `index-storage-tooling.mjs render` command continues to use the stricter accepted-decision finalizer.

## Verify the saved ADR

After saving or reviewing the generated Markdown, verify that it still represents the exact source files:

```bash
node scripts/verify/index-storage-tooling.mjs verify-adr \
  --comparison evidence/index-storage/comparison/comparison.json \
  --decision evidence/index-storage/comparison/decision.json \
  --adr crates/rustok-index/docs/adr-postgresql-storage.md
```

`verify-adr` recalculates both digest lines from exact file bytes, snapshots the same comparison and decision bytes, repeats deterministic finalization including the observed database-settings gate, and requires the saved ADR to match the regenerated Markdown byte for byte. Any manual edit, formatting change, stale decision, replaced evidence file, or methodology drift is rejected.

The saved-ADR verifier accepts only `--comparison`, `--decision`, and `--adr`. Help is valid only as the sole argument; unknown, incomplete, mixed-help, or duplicate options fail before any supplied comparison, decision, or ADR file is read.

The generated ADR includes storage size, read latency, mutation latency, WAL, churn, and VACUUM evidence for all candidates. It never infers or ranks a winner. Its Markdown depends on evidence and decision content, not on the filesystem paths used to invoke the tooling.

## Validation boundary

The tooling router, direct renderer, ADR finalizer, and saved-ADR verifier do not replace benchmark execution, evidence-packet validation, production migration rehearsal, or production observability. They expose the existing contracts consistently and turn an already validated official comparison plus an explicit human decision into a reviewable, byte-bound document.
