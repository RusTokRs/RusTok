# Pages physical-loss recovery PostgreSQL evidence actualization — 2026-08-15

Status: `source-ready / exact-main-execution-pending / registry-admission-pending`.

## Fresh base

Prepared from `main@dfbf002dc4c6a0e46a19a4c6555eef9896476ec1` after the terminal Page Builder FBA blocker inventory was recomputed from 7 to 6.

The canonical Page Builder FBA registry currently records:

- `/consumers/0/artifact_repair/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence = pending`;
- `/consumers/0/artifact_repair/physical_loss_recovery/rollback_activated_current_set_recovery/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence = verified`.

This slice targets only the still-pending physical-loss recovery parent. It does not reopen or re-execute the two already-admitted nested recovery evidence nodes.

## Parent execution boundary

The physical-loss recovery parent owns the direct source-publish recovery boundary that precedes the already-admitted rollback-activated and repeated-loss extensions. The exact-main packet therefore executes both direct PostgreSQL source packets required by the parent:

1. `artifact_loss_activation_recovery_postgres.rs` — single-locale physical source-artifact loss, explicit rebuild and missing-binding activation with the source artifact remaining absent;
2. `artifact_loss_multilocale_activation_recovery_postgres.rs` — two lost locales recovered sequentially from one retained publish authority, plus rejection of an unexplained version gap.

The source guard remains fail-closed through both existing Node verifiers:

- `verify-pages-explicit-artifact-binding-replacement.mjs` validates the complete explicit binding-replacement source boundary and requires the historical source contract to remain unexecuted;
- `verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs` validates the bounded same-publish multi-locale chain and its PostgreSQL packet.

The historical contract `pages-explicit-artifact-binding-replacement-source.json` intentionally remains a source contract with `execution: []` and false validation/execution flags. This evidence workflow does not rewrite those nonclaims into live receipt state.

## Exact-main execution contract

`crates/rustok-pages/contracts/evidence/pages-physical-loss-recovery-postgres-execution.json` defines a separate execution lineage:

- source status: `source_ready_main_execution_pending`;
- target: `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence`;
- required target pre-state: `pending`;
- required nested pre-states: rollback-activated current-set recovery = `verified`, repeated-loss recovery = `verified`;
- required artifact-repair parent pre-state: `verified`;
- exact-main execution on PostgreSQL 16 with Rust 1.96.0;
- retained source SHA-256 hashes and bounded receipt artifact;
- no registry mutation.

Successful execution may produce only `postgres_execution_passed_physical_loss_recovery_parent_admission_pending`. It does not set the registry target to `verified`.

## Workflow lifecycle

`.github/workflows/pages-physical-loss-recovery-postgres-evidence.yml` follows the evidence/admission lifecycle boundary used by the preceding recovery slices:

- PR paths include the workflow and canonical FBA registry so review-time drift fails closed;
- push/main paths include runtime, packet, source/execution contracts and this actualization;
- the FBA registry and workflow file are deliberately excluded from push/main paths;
- therefore a later registry-only admission must not rerun a receipt that requires the pre-admission target state `pending`;
- runtime/source drift still triggers a new exact-main evidence run.

## Validation commands

The PR preflight and exact-main execution run:

```text
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-binding-replacement.mjs
node crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
cargo test --locked -p rustok-pages --test artifact_loss_activation_recovery_postgres -- --nocapture
cargo test --locked -p rustok-pages --test artifact_loss_multilocale_activation_recovery_postgres -- --nocapture
cargo check --locked -p rustok-pages --all-targets
```

No separate execution of rollback-activated current-set recovery or repeated-loss recovery is claimed by this parent slice; their admitted evidence is an explicit precondition rather than inferred from parent execution.

## Nonclaims

This slice does **not**:

- mutate the canonical Page Builder FBA registry;
- mark the physical-loss parent verified before a separate admission PR;
- modify either nested physical-loss recovery child;
- verify or mutate rollback continuity;
- verify cache-consumer or provider-consumer-properties evidence;
- recompute the terminal inventory;
- clear the Pages `execution-rollout-pending` marker;
- make owner/platform review ready;
- promote Pages FFA or Page Builder FBA.

## Next cursor

After a successful retained exact-main artifact:

1. open a separate, one-line registry admission PR changing only `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence` from `pending` to `verified`;
2. verify registry-only admission does not retrigger the pre-admission exact-main receipt;
3. in a later separate PR, recompute the terminal Page Builder FBA blocker inventory from 6 to 5;
4. continue with the remaining rollback-continuity, cache-consumer and provider-consumer-properties evidence nodes without parent/sibling inference.
