# Pages artifact rollback PostgreSQL evidence actualization — 2026-08-14

Status: `exact-main-evidence-source-ready / main-execution-pending / registry-admission-pending`.

## Scope

This slice defines retained execution evidence for only the canonical Page Builder FBA node:

`/consumers/0/artifact_rollback/executed_evidence`

It does not infer or clear any nested `artifact_repair`, physical-loss recovery, rollback-continuity, cache-consumer, or provider consumer-properties evidence node.

## Preflight validation

Before this permanent workflow was introduced, branch-only PostgreSQL run `31766263433` completed successfully on source SHA `499054c0a59eeadfcc93fd7efa626d94f6eaf90b` after two stale source-verifier markers were corrected in PR #3542.

That run passed:

- `node crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs`;
- `node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs`;
- `cargo test --locked -p rustok-pages --test publish_rollback_outbox_cache_postgres -- --nocapture` against PostgreSQL 16;
- `cargo check --locked -p rustok-pages --all-targets`.

The temporary validation workflow was removed before PR #3542 was opened. PR #3542 then merged the two verifier corrections to `main` as `41406b8c6fc7602d06fceeb86b46934f602fc28c`.

The branch run is preflight validation only. It is not the canonical registry admission packet because it was not an exact-main run.

## Permanent exact-main evidence path

`.github/workflows/pages-artifact-rollback-postgres-evidence.yml` runs the same source verifiers and PostgreSQL integration test on `push` to `main` for rollback/outbox/cache-relevant source changes. Both source and runtime jobs require:

- GitHub Actions `push` event;
- `refs/heads/main`;
- checkout `HEAD == GITHUB_SHA`;
- read-only repository contents permission.

The runtime job uses PostgreSQL 16 and emits only a bounded JSON receipt. The receipt retains exact source commit/run identity, SHA-256 hashes of required sources, command success, and the canonical registry pre-state.

The evidence artifact is retained for 90 days. It does not upload PostgreSQL rows, event payloads, cache values, tenant identity, credentials, database URLs, traces, screenshots, or raw test logs as the evidence artifact.

## Admission boundary

A successful packet has:

- format `pages_artifact_rollback_postgres_execution_v1`;
- status `postgres_execution_passed_artifact_rollback_admission_pending`;
- parent `artifact_rollback` scope only;
- `registry_mutated=false`;
- `artifact_rollback_registry_verified=false`;
- `artifact_repair_registry_verified=false`.

Only a later, separate registry-only admission may change `/consumers/0/artifact_rollback/executed_evidence` from `pending` to `verified`, after the retained exact-main packet and source lineage are rechecked. A later terminal-inventory source recompute is also required after such an admission.

## Non-claims

This source slice does not claim exact-main PostgreSQL execution has happened yet, does not mutate the Page Builder FBA registry, does not clear nested repair evidence, does not complete terminal inventory, does not make owner/platform review ready, and does not promote Pages FFA or Page Builder FBA.
