# Pages artifact repair PostgreSQL evidence actualization — 2026-08-14

Status: `exact-main-evidence-source-ready / parent-artifact-repair-execution-pending / nested-repair-evidence-unchanged`.

## Fresh source recheck

Authored from `main@fd32f0ffdfba840fbb262d36ddb6bd171815119d` after the canonical JSONB sanitization fix had already merged through PR #3555. The current terminal evidence inventory has nine Page Builder FBA blocker nodes. Seven are the parent/nested artifact-repair lineage, with provider consumer-properties and cache-consumer as the other independent blockers.

This slice does not rebase or recreate the already-merged JSONB work. It advances the next execution cursor using the existing explicit artifact-repair PostgreSQL harness that has remained source-ready but unvalidated since 2026-08-07.

## Existing source boundary

The retained source contract `pages_explicit_artifact_repair_postgres_source_v1` and `explicit_artifact_repair_postgres.rs` already cover the parent explicit repair mechanics on real PostgreSQL migrations:

- reviewed publish creates durable rebuild provenance;
- explicit rebuild appends a distinct immutable artifact without moving the live binding or page version;
- rebuild replay and idempotency conflict stay bounded;
- migration-owned rebuild receipt uniqueness is exercised transactionally;
- stale activation is rejected;
- successful activation switches the exact locale binding, advances the page version once and writes one `NodeUpdated` plus one `NodePublished` event;
- activation replay/reuse rejection and receipt-conflict rollback are exercised.

The historical source packet intentionally keeps `execution=[]` and every validation field false. This actualization does not rewrite that historical source claim.

## Exact-main execution source

This continuation adds:

- `.github/workflows/pages-artifact-repair-postgres-evidence.yml`;
- `crates/rustok-pages/contracts/evidence/pages-artifact-repair-postgres-execution.json`.

For pull requests the workflow executes the source verifier, PostgreSQL 16 integration test and `cargo check --locked -p rustok-pages --all-targets` against the event SHA. This validates the proposed merge context without minting deployment/governance evidence.

Only `push` to `main` may retain the bounded evidence receipt. The receipt is bound to exact `GITHUB_SHA`, workflow/run identity and SHA-256 of the required source files. The workflow has read-only repository permissions and does not mutate registry, plans, runtime state or control plane.

## Admission boundary

A successful exact-main packet may support a later, separate admission review for exactly:

`/consumers/0/artifact_repair/executed_evidence`

The expected pre-state is `pending`. This workflow does not change it.

The packet must not infer any of the nested repair blockers:

- physical-loss recovery, including rollback-activated and repeated-loss recovery;
- repair-aware rollback continuity and its nested activation-prefix/rollback-activated paths;
- cache-consumer execution;
- provider consumer-properties execution.

It also does not remove the Pages `execution-rollout-pending` marker, complete the terminal inventory, perform owner/platform review, or promote Pages FFA / Page Builder FBA.

## CI/resource boundary

The workflow uses one concurrency group per ref with `cancel-in-progress: true`, so superseded runs of this evidence workflow cancel automatically. No temporary validation workflow is retained.

## Next cursor

1. Merge this source slice only after PR preflight is green.
2. Require the resulting exact-main evidence run to succeed and retain its 90-day bounded receipt.
3. In a separate admission change, verify the retained source commit/digests and change only the parent `artifact_repair.executed_evidence` node if the packet is valid.
4. Recompute the terminal blocker inventory separately.
5. Continue the nested physical-loss / rollback-continuity / cache / provider evidence nodes independently; do not collapse them into the parent packet.
