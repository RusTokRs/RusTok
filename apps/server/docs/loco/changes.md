# Loco.rs changes log (project-local)

Use this file as short log of server-patterns changes,
so developers and AI-agents can quickly find current solutions.

## Format

- YYYY-MM-DD — change summary
  - affected path(s)
  - migration/auth/routing impact
  - breaking/not-breaking

## Entries

- 2026-02-11 — Switched Loco docs to upstream snapshot source-of-truth model
  - `apps/server/docs/loco/upstream/README.md`
  - `apps/server/docs/loco/upstream/VERSION`
  - `apps/server/docs/loco/README.md`
  - `scripts/docs/sync_loco_docs.sh`
  - Added canonical upstream snapshot workflow and agent guidance to consult upstream first
  - Not breaking

- 2026-02-11 — Added initial Loco context pack
  - `apps/server/docs/loco/README.md`
  - Added server-local guidance to reduce framework hallucinations
  - Not breaking
