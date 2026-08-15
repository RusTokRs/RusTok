# Pages rollback-activated repair-to-rollback PostgreSQL evidence actualization — 2026-08-15

Status: `exact-main-evidence-source-ready / execution-pending / registry-admission-separate`.

## Fresh boundary

This execution slice is based on fresh `main@404c1eb70a2471125f64aea91e1fba40ab84a8ee`, after terminal inventory recomputation to four blockers.

The canonical Page Builder FBA registry still records:

- `/consumers/0/artifact_repair/rollback_continuity/rollback_activated_repair_to_rollback/executed_evidence = pending`;
- `/consumers/0/artifact_repair/rollback_continuity/physical_loss_activation_prefix/executed_evidence = verified`;
- `/consumers/0/artifact_repair/rollback_continuity/executed_evidence = pending`;
- `/consumers/0/artifact_repair/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence = verified`.

No open competing PR for this target was found at the fresh-base recheck.

## Existing source packet

The source-ready packet remains:

- `crates/rustok-pages/tests/artifact_rollback_activated_repair_rollback_continuity_postgres.rs`;
- `crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-repair-rollback-continuity.mjs`;
- `crates/rustok-pages/contracts/evidence/pages-rollback-activated-repair-rollback-continuity-source.json`;
- `docs/modules/pages-page-builder-rollback-activated-repair-rollback-continuity-actualization-2026-08-07.md`.

The historical source contract deliberately remains `pages_rollback_activated_repair_rollback_continuity_source_unvalidated`, with empty `execution` and false execution/validation flags. Those fields are source-contract nonclaims and are **not** mutated by later workflow execution. Exact execution evidence is carried by the separate execution contract and retained exact-main receipt defined by this slice.

The target PostgreSQL test retains both source scenarios:

1. a three-publish lifecycle `P0 -> P1 -> P2 -> rollback to P1 -> physical loss -> rebuild -> activation from the exact rollback anchor -> rollback repaired P1 to P0`, including idempotent replay;
2. corruption of the durable rollback-anchor request hash after repair activation, which must reject the final rollback without changing page version, binding or rollback receipt count.

The verifier also keeps the repeated-loss latest-state rollback regression connected to the source boundary, so this evidence workflow executes that regression without changing or re-admitting its already verified registry node.

## Execution contract

This slice adds:

- `.github/workflows/pages-rollback-activated-repair-rollback-continuity-postgres-evidence.yml`;
- `crates/rustok-pages/contracts/evidence/pages-rollback-activated-repair-rollback-continuity-postgres-execution.json`;
- this retained actualization.

The workflow requires PostgreSQL 16 and Rust 1.96.0 and runs:

```text
node crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-repair-rollback-continuity.mjs
cargo test --locked -p rustok-pages --test artifact_rollback_activated_repair_rollback_continuity_postgres -- --nocapture
cargo test --locked -p rustok-pages --test artifact_repeated_loss_recovery_postgres -- --nocapture
cargo check --locked -p rustok-pages --all-targets
```

The exact-main receipt can be recorded only on `push` to `main`, after the preflight succeeds and after the recorder confirms the exact canonical registry pre-state required by the execution contract.

## Workflow lifecycle

The evidence workflow intentionally uses different PR and push path boundaries:

- PR paths include the canonical FBA registry and the workflow file itself so contract/lifecycle changes are validated before merge;
- push/main paths exclude the registry and workflow file while retaining runtime, tests, source contracts, execution contract and actualization dependencies.

This prevents a later registry-only `pending -> verified` admission from rerunning a receipt that requires the pre-admission target state to remain `pending`. Runtime/source drift still triggers fail-closed exact-main re-execution.

## Receipt scope

A successful exact-main receipt has format `pages_rollback_activated_repair_rollback_continuity_postgres_execution_v1` and status `postgres_execution_passed_rollback_activated_repair_to_rollback_admission_pending`.

It records only that the exact-main source commit passed the source verifier, the target PostgreSQL success/corrupted-anchor scenarios, the repeated-loss rollback regression and `cargo check --all-targets`, while the canonical target was still `pending` and its admitted prerequisites remained in the required states.

The receipt does **not** mutate or admit the registry.

## Nonclaims

This evidence slice does not claim or perform:

- mutation of the historical source contract's execution flags;
- registry admission for the target;
- re-admission of physical-loss activation-prefix or repeated-loss evidence;
- rollback-continuity parent evidence;
- cache-consumer evidence;
- provider consumer-properties evidence;
- terminal inventory completion;
- owner/platform readiness approval;
- Pages FFA promotion;
- Page Builder FBA promotion.

`execution-rollout-pending` therefore remains an independent Pages terminal blocker, and the current four-blocker terminal inventory is not recomputed by this evidence PR.

## Next cursor

After a successful retained exact-main receipt, the next change is a **separate one-line registry admission PR** for exactly:

`/consumers/0/artifact_repair/rollback_continuity/rollback_activated_repair_to_rollback/executed_evidence: pending -> verified`.

Only after that admission should terminal inventory be recomputed separately from four blockers to three. Rollback-continuity parent evidence must remain independent and may not be inferred from either child.
