# Pages rollback-activated artifact-loss recovery PostgreSQL evidence actualization — 2026-08-14

Status: `exact-main-evidence-source-ready / registry-admission-pending / terminal-inventory-unchanged`.

## Fresh base

This execution-evidence slice is based on `main@0f2782ef47aa9a5bd6bd222fc40ea2759f403d51` after the terminal inventory recomputation recorded eight remaining Page Builder FBA evidence blockers.

The selected target is exactly:

`/consumers/0/artifact_repair/physical_loss_recovery/rollback_activated_current_set_recovery/executed_evidence`

The canonical registry currently records this node as `pending`. The parent `artifact_repair.executed_evidence` is already `verified`, while the physical-loss parent, repeated-loss recovery, rollback-continuity, cache-consumer and provider-consumer-properties evidence remain separate obligations.

## Existing source packet

The repository already contains the source-only rollback-activated physical-loss recovery packet:

- `crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs`;
- `crates/rustok-pages/contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json`;
- `docs/modules/pages-page-builder-rollback-activated-recovery-actualization-2026-08-07.md`.

That source contract deliberately retains `database_scenario_run=false`, `tests_run=false`, `static_verifier_run=false`, `cargo_run=false` and `workflows_or_ci_run=false`. This slice does not rewrite that historical source claim. Instead it adds a separate exact-main execution contract and durable CI receipt.

## Exact execution boundary

The evidence workflow performs, on the exact event checkout:

1. the existing static source verifier;
2. PostgreSQL 16 execution of `artifact_loss_after_rollback_activation_recovery_postgres` with `--locked` and `--nocapture`;
3. `cargo check --locked -p rustok-pages --all-targets`.

For pull requests this is preflight only. On `push/main`, after a successful preflight, the workflow writes a bounded receipt retaining the exact source commit, workflow run identity, target registry pre-state and SHA-256 hashes of the required evidence/runtime source files.

The workflow has read-only repository permissions and does not mutate the registry, database source state, local readiness plans or any control-plane state.

## Lifecycle boundary

The canonical Page Builder registry and the workflow file itself remain in `pull_request.paths` so changes to either are validated before merge.

They are deliberately excluded from `push/main.paths`. The initial exact-main evidence run is triggered by the new execution contract and this actualization file. A later registry-only admission or workflow-maintenance merge therefore cannot accidentally rerun a pre-admission recorder after the target is already `verified`.

Actual runtime/evidence source changes remain in the `push/main` trigger set. If those sources drift after admission, the receipt's `pending` pre-state guard continues to fail closed rather than silently minting replacement evidence over an admitted node.

## Scope and non-claims

A successful receipt may support only the selected nested node. It does not prove or mutate:

- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence`;
- any rollback-continuity evidence node;
- cache-consumer evidence;
- provider consumer-properties evidence;
- terminal evidence inventory completion;
- owner/platform readiness approval;
- Pages FFA or Page Builder FBA promotion.

The registry transition remains a separate PR after retained exact-main evidence is inspected. A separate terminal inventory recomputation is required after any registry admission.

## Expected receipt status

Successful exact-main execution emits:

`postgres_execution_passed_rollback_activated_current_set_recovery_admission_pending`

The strongest claim is therefore execution evidence retained and registry admission pending.

## Next cursor

1. merge this evidence source after PR preflight succeeds;
2. inspect the exact-main receipt artifact and confirm source hashes, target pre-state and non-claims;
3. in a separate PR, admit only the selected registry node if the retained receipt is valid and unexpired;
4. recompute the terminal inventory after admission;
5. continue with the remaining independent evidence blockers.
