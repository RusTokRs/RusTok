# Pages physical-loss activation-prefix PostgreSQL evidence actualization — 2026-08-15

Status: `source-ready / exact-main-postgres-execution-pending / registry-admission-separate`.

## Fresh boundary

This evidence slice is prepared from fresh `main@6cb7d26734661b17f9b2ca8fead6e46c552bc3eb` after the terminal Page Builder FBA inventory was recomputed to five blockers.

The immediately preceding Pages/Page Builder merge was inventory PR #3587 (`887f29f55eaedd45c2794fd5173cc767e5934667`). Main then advanced through Taxonomy and RBAC-only work. The full drift intersection from #3587 through this branch base does not touch Cargo files, Pages runtime, Page Builder runtime, the canonical FBA registry, this source packet, Pages plans or terminal-readiness sources.

The canonical Page Builder FBA registry records:

- `/consumers/0/artifact_repair/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence = verified`;
- `/consumers/0/artifact_repair/rollback_continuity/physical_loss_activation_prefix/executed_evidence = pending`;
- `/consumers/0/artifact_repair/rollback_continuity/rollback_activated_repair_to_rollback/executed_evidence = pending`;
- `/consumers/0/artifact_repair/rollback_continuity/executed_evidence = pending`.

This slice targets only the physical-loss activation-prefix child. Parent and sibling evidence remain independent.

## Source packet

The retained source authority is:

- `crates/rustok-pages/contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-multilocale-repair-rollback-evidence.mjs`;
- `crates/rustok-pages/tests/artifact_multilocale_repair_rollback_evidence_postgres.rs`;
- `crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs`;
- `docs/modules/pages-page-builder-multilocale-rollback-evidence-actualization-2026-08-07.md`;
- `docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md`.

The historical source contract intentionally remains `pages_multilocale_repair_rollback_latest_state_evidence_source_unvalidated`, with `execution: []`, every validation field false and every source-contract execution flag false. Exact-main CI execution is retained separately and does not rewrite those historical nonclaims.

## Child execution boundary

The `physical_loss_activation_prefix` registry node describes rollback reconstruction of a minimal bounded repair prefix. The source verifier requires both existing PostgreSQL packets because the child includes latest-state-per-locale semantics:

1. `artifact_multilocale_repair_rollback_evidence_postgres.rs` proves direct multi-locale physical-loss recovery can reconstruct the repaired current cursor for rollback, and rejects a noncanonical activation request hash or a noncontiguous activation prefix without mutating the page/bindings;
2. `artifact_repeated_loss_recovery_postgres.rs` proves rollback reconstruction follows the latest repeated recovery state for a locale after the prior rebuilt artifact was itself physically lost.

Executing the repeated-loss test here does not re-admit or mutate its already verified physical-loss child registry node. It is executed because latest-state-per-locale rollback reconstruction is an explicit part of the activation-prefix source contract.

The separate `rollback_activated_repair_to_rollback` packet remains outside this child. Its own source contract/test/verifier must be executed and admitted independently.

## Exact-main execution contract

`crates/rustok-pages/contracts/evidence/pages-physical-loss-activation-prefix-postgres-execution.json` requires:

- PostgreSQL 16;
- Rust 1.96.0;
- exact event SHA checkout;
- `node crates/rustok-pages/scripts/verify/verify-pages-multilocale-repair-rollback-evidence.mjs`;
- `cargo test --locked -p rustok-pages --test artifact_multilocale_repair_rollback_evidence_postgres -- --nocapture`;
- `cargo test --locked -p rustok-pages --test artifact_repeated_loss_recovery_postgres -- --nocapture`;
- `cargo check --locked -p rustok-pages --all-targets`.

The exact-main recorder requires the target child to remain `pending`, the rollback-continuity parent and rollback-activated repair-to-rollback sibling to remain `pending`, and the already admitted artifact-repair / physical-loss / repeated-loss evidence preconditions to remain `verified`.

Successful execution may produce only `postgres_execution_passed_physical_loss_activation_prefix_admission_pending`. The workflow never changes the canonical registry.

## Workflow lifecycle

`.github/workflows/pages-physical-loss-activation-prefix-postgres-evidence.yml` follows the established evidence/admission lifecycle:

- PR paths include the FBA registry and workflow itself so review-time state/lifecycle drift is preflighted;
- push/main paths deliberately exclude the registry and workflow file;
- source/runtime/test/verifier/execution-contract/actualization drift remains a push/main trigger;
- therefore the initial evidence merge triggers an exact-main receipt through the newly added execution contract/actualization, while a later registry-only admission cannot rerun a receipt requiring target pre-state `pending`.

## Retained receipt boundary

The bounded artifact retains:

- exact source commit and workflow run identity;
- source-verifier, both PostgreSQL test and all-target Cargo-check success facts;
- registry pre-state for the target, rollback-continuity parent, sibling and already admitted dependencies;
- SHA-256 for every explicitly bound source file;
- explicit governance nonclaims.

It does not retain database URLs, tenant identity, raw database rows, event payloads or credentials. Artifact retention is 90 days and `receipt.json` is bounded to 1 MiB.

## Nonclaims

This slice does **not**:

- mark `/consumers/0/artifact_repair/rollback_continuity/physical_loss_activation_prefix/executed_evidence` verified;
- re-admit or mutate repeated-loss recovery merely because its test is executed as part of this source packet;
- verify or mutate the `rollback_activated_repair_to_rollback` sibling;
- verify the rollback-continuity parent;
- verify cache-consumer or provider-consumer-properties evidence;
- recompute the terminal inventory;
- clear Pages `execution-rollout-pending`;
- make owner/platform review ready;
- promote Pages FFA or Page Builder FBA.

## Next cursor

1. merge this evidence packet only after exact PR-head preflight succeeds;
2. require successful post-merge exact-main preflight, retained receipt and gate and inspect the retained artifact;
3. only then create a separate registry admission PR changing the physical-loss activation-prefix child from `pending` to `verified`;
4. only after admission, separately recompute terminal inventory from five to four if no concurrent canonical blocker changes supersede that count;
5. keep `rollback_activated_repair_to_rollback`, rollback-continuity parent, cache and provider evidence as independent later slices.
