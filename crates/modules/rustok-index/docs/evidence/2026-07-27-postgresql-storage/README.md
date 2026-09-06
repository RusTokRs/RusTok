# Accepted Index PostgreSQL storage evidence

This directory preserves the exact decision inputs and the original validated
GitHub Actions artifacts for the accepted Index PostgreSQL storage ADR.

- Workflow run: `30222913450`
- Repository commit: `eae5f74241e9431bffe2fd8c43cd046fc1c1f679`
- PostgreSQL image: `postgres:16`
- Packet contract: `v2`
- Result digest contract: `ordered_length_prefixed_json_v1`
- Repetitions: `3`
- Churn cycles: `5`
- Selected model: `jsonb`
- Comparison SHA-256: `7d10a3de9f62cf315d578794d1b69caa9a45d72847d1480cd24f9a9da4e9bbd8`
- Decision SHA-256: `ae77267776a38264c9432618459fb80559962842877596fc156ebd4e3a12e883`

The three ZIP files are the original Actions artifacts. `comparison.json`,
`comparison.md`, and `decision.json` are extracted exact files used by
`DECISIONS/2026-07-24-index-storage-layout.md`.

Verify the accepted ADR from the repository root with:

```bash
node scripts/verify/index-storage-tooling.mjs verify-adr \
  --comparison crates/rustok-index/docs/evidence/2026-07-27-postgresql-storage/comparison.json \
  --decision crates/rustok-index/docs/evidence/2026-07-27-postgresql-storage/decision.json \
  --adr DECISIONS/2026-07-24-index-storage-layout.md
```

The failed predecessor run `30220486083` is diagnostic only and is not part of
this archive.
