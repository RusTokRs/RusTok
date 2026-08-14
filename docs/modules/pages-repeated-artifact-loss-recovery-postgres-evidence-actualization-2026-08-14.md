# Pages repeated artifact-loss PostgreSQL evidence actualization — 2026-08-14

Status: `source-ready / exact-main-postgres-execution-pending / registry-admission-separate`.

## Fresh boundary

This evidence slice starts from canonical `main@7613bd01c7b78fe1f2047d81a5eaa1dae44b5db9` after the terminal evidence inventory was recomputed to seven blockers.

The canonical Page Builder FBA registry records:

- `/consumers/0/artifact_repair/physical_loss_recovery/rollback_activated_current_set_recovery/executed_evidence = verified`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence = pending`;
- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence = pending`.

This packet targets only the repeated-loss child. It does not infer the physical-loss parent from a successful child execution.

## Source packet

The retained source authority remains:

- `crates/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs`;
- `crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs`;
- `docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md`.

The historical source contract intentionally keeps `execution: []`, all validation flags false, and its source-contract execution nonclaims false. A later CI execution is retained separately and does not rewrite those historical source nonclaims.

The PostgreSQL packet covers four bounded scenarios:

1. the same locale is recovered, loses the rebuilt immutable artifact, and is recovered again with replay idempotency preserved;
2. repeated recovery is rejected while the prior rebuilt artifact remains physically present;
3. another locale can recover after the first locale has already been recovered twice;
4. rollback continuity reaches the latest repeated recovery state rather than stopping at the first locale occurrence.

## Exact-main execution contract

`crates/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-postgres-execution.json` requires:

- PostgreSQL 16;
- Rust 1.96.0;
- exact event SHA checkout;
- `node crates/rustok-pages/scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs`;
- `cargo test --locked -p rustok-pages --test artifact_repeated_loss_recovery_postgres -- --nocapture`;
- `cargo check --locked -p rustok-pages --all-targets`.

PR execution is preflight only. A retained receipt is created only by a successful `push` on exact `main` after merge.

## Admission boundary

The retained receipt may support a later, separate registry transition only for:

`/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence`

from `pending` to `verified`.

The workflow itself does not mutate the registry. The following remain separate nonclaims:

- physical-loss recovery parent verification;
- any mutation of the already verified rollback-activated current-set sibling;
- rollback-continuity verification;
- cache-consumer verification;
- provider consumer-properties verification;
- terminal inventory completion;
- owner/platform review readiness;
- Pages FFA promotion;
- Page Builder FBA promotion.

After any later registry admission, terminal inventory recomputation is also a separate change.

## Workflow lifecycle

`.github/workflows/pages-repeated-artifact-loss-recovery-postgres-evidence.yml` deliberately has asymmetric path triggers:

- PR paths include the canonical FBA registry and the workflow itself, so admission/lifecycle changes are checked before merge;
- `push/main` paths exclude the FBA registry and workflow file, so a later registry-only admission does not rerun a pre-admission receipt that requires the target to remain `pending`;
- actual Pages runtime/source, test, verifier, execution-contract or evidence-actualization drift still triggers exact-main re-execution.

This preserves fail-closed evidence lifecycle without burning CI on the separate admission step.

## Retained receipt boundary

The exact-main artifact retains only bounded provenance and source hashes:

- exact source commit and workflow run identity;
- source verifier / PostgreSQL integration test / all-target cargo-check success facts;
- registry pre-state for the target, parent and neighboring blockers;
- SHA-256 for each explicitly bound source file;
- explicit governance nonclaims.

It does not retain the database URL, tenant identity, raw database rows, event payloads or credentials. Artifact retention is 90 days and the receipt is bounded to at most 1 MiB.

## Next cursor

1. merge this evidence packet only after PR preflight succeeds;
2. require the post-merge exact-main preflight, receipt and gate to succeed and inspect the retained artifact;
3. only then create a separate registry admission PR for the repeated-loss child;
4. only after admission, separately recompute terminal inventory from seven to six if no concurrent canonical blocker changes supersede that count.
