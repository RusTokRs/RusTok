# Pages artifact-repair rollback-continuity parent PostgreSQL evidence actualization — 2026-08-15

Status: `source-ready / exact-main-execution-pending / separate-registry-admission-required`.

## Fresh boundary

This evidence slice starts from fresh `main@e3716445b4fd1d146f70b936ed8ccb721e313e6e`, the terminal-inventory `4 → 3` merge from PR #3600.

The canonical Page Builder FBA target remains:

`/consumers/0/artifact_repair/rollback_continuity/executed_evidence = pending`.

Required already-admitted prerequisites are independently `verified`:

- `/consumers/0/artifact_repair/executed_evidence`;
- `/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence`;
- `/consumers/0/artifact_repair/rollback_continuity/physical_loss_activation_prefix/executed_evidence`;
- `/consumers/0/artifact_repair/rollback_continuity/rollback_activated_repair_to_rollback/executed_evidence`;
- `/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence`.

Cache-consumer and provider consumer-properties evidence remain unrelated pending nodes. This slice does not infer them and does not mutate the registry.

## Parent source packet

The historical parent source contract remains deliberately unvalidated and unchanged:

- `crates/rustok-pages/contracts/evidence/pages-artifact-repair-rollback-continuity-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-artifact-repair-rollback-continuity.mjs`;
- `crates/rustok-pages/tests/artifact_repair_rollback_continuity_postgres.rs`;
- `crates/rustok-pages/docs/artifact-repair-rollback-continuity.md`;
- `docs/modules/pages-page-builder-repair-rollback-continuity-actualization-2026-08-07.md`.

That parent PostgreSQL harness covers the parent contract directly: successful physical-loss rebuild + explicit activation + rollback continuity, idempotent rollback replay, historical-target manifest rejection, surviving-manifest identity mismatch rejection, and fail-closed rejection when a current manifest is missing while the historical source artifact still exists.

The historical source contract keeps `execution=[]`, all validation flags false and all execution flags false. Live execution is represented only by the separate execution contract and retained exact-main receipt created by this slice.

## Retained child evidence and no-drift boundary

The two direct rollback-continuity child obligations already have retained exact-main PostgreSQL evidence and separate registry admissions:

- physical-loss activation-prefix: merge `3fb9ffbf3f66c9832a6b58ca8f135882fe1053c0`, run `31867845157`, artifact `9242726793`, digest `sha256:201900cba75d789d57a95cb99bba3fc0890945b0a525bfcdb54e4db347d5661e`;
- rollback-activated repair-to-rollback: merge `4c02c5594de7ab30eff44cb62cc9d453ddba74b4`, run `31885669589`, artifact `9247281979`, digest `sha256:53b99de5d56c9a631001a6b2f00719c5b2736ab6b937f1684cad5bdbc474427b`.

The fresh compare from the latest direct-child evidence merge `4c02c559...` to current `main@e3716445...` changes only Index workflow/verifier files, Forum plan, the one-line child registry admission, and terminal-inventory source/verifier/actualization files. It does not touch Cargo, Pages runtime, the parent source contract/verifier/test, either child PostgreSQL test/runtime packet, Page Builder runtime or Outbox runtime.

The wider compare from the activation-prefix evidence merge also contains repository-wide workflow/tooling and unrelated module work plus the later rollback-activated evidence packet, but still does not modify the Pages artifact-repair runtime or the activation-prefix PostgreSQL test/source packet. Therefore this parent slice does not waste CI by re-executing already-retained child packets whose evidence-bound source did not drift.

This is not inference from child evidence. The parent has its own exact-main PostgreSQL execution; the child packets are prerequisites whose already-admitted states and source continuity are retained separately.

## Execution contract

New source-ready execution packet:

- `.github/workflows/pages-artifact-repair-rollback-continuity-postgres-evidence.yml`;
- `crates/rustok-pages/contracts/evidence/pages-artifact-repair-rollback-continuity-postgres-execution.json`;
- this actualization.

PR-side preflight executes on the exact PR head:

1. `node crates/rustok-pages/scripts/verify/verify-pages-artifact-repair-rollback-continuity.mjs`;
2. `cargo test --locked -p rustok-pages --test artifact_repair_rollback_continuity_postgres -- --nocapture` against PostgreSQL 16;
3. `cargo check --locked -p rustok-pages --all-targets`.

On `push/main`, only after that preflight succeeds, the workflow records a bounded receipt tied to exact `GITHUB_SHA`. The recorder requires the parent target to remain `pending` and all listed prerequisites to remain `verified`. It retains source SHA-256 bindings and records no database URL, tenant identity, raw rows, event payloads or credentials.

## Lifecycle boundary

`pull_request.paths` includes the canonical FBA registry, so a later separate registry admission still reruns the parent source verifier/PostgreSQL packet/Cargo check while the exact-main receipt job is skipped on PR events.

`push/main.paths` deliberately excludes the canonical registry and the workflow file itself. The initial evidence merge still triggers exact-main execution through the new execution contract and this actualization. A later registry-only `pending → verified` admission or workflow-only maintenance merge cannot mint a new pre-admission receipt after the target changes state.

Runtime/source/test/execution-contract drift remains a `push/main` trigger and therefore fails closed against the target pre-state rather than silently replacing admitted evidence.

## Non-claims

This slice does not:

- change `/consumers/0/artifact_repair/rollback_continuity/executed_evidence`;
- re-admit or mutate either direct child evidence node;
- change cache-consumer or provider consumer-properties evidence;
- recompute terminal inventory;
- clear Pages `execution-rollout-pending`;
- claim owner/platform review readiness;
- promote Pages FFA or Page Builder FBA;
- mutate runtime, schema, migration, local readiness plans or the central readiness registry.

## Next cursor

After a successful retained exact-main receipt, create a separate one-line registry admission PR for only `/consumers/0/artifact_repair/rollback_continuity/executed_evidence: pending → verified`. Then, in another separate PR, recompute terminal inventory `3 → 2` if the canonical registry still has no concurrent blocker changes.
