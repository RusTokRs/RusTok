# Pages / Page Builder Repair-to-Rollback Continuity Actualization

Date: 2026-08-07  
Status: current-source-overlay / repair-rollback-continuity-source-ready / execution-open

This overlay continues `pages-page-builder-activation-recovery-implementation-actualization-2026-08-07.md` after the missing-binding activation recovery merged in PR #3167.

## Main recheck

The following source capabilities remain ready and must not be reopened as missing implementation:

- registered Pages metadata contribution with bespoke `PageMetadataEditor` removed;
- reviewed static Page Builder publish and immutable publish manifests;
- immutable rollback owner plus public/admin transports;
- retained rebuild provenance independent of immutable artifact lifetime;
- bounded integrity audit and explicit append-only rebuild;
- strict existing-binding activation;
- bounded missing-binding activation after physical source artifact loss;
- event-driven cache boundary and immutable public readers;
- authenticated real-DOM authoring, authoring asset delivery, same-origin admin launch and release composition.

Execution evidence for those source-ready areas remains separate and pending where recorded.

## Newly confirmed continuity gap

PR #3167 makes physical-loss recovery complete through explicit activation, but the next owner operation exposed a stale assumption in rollback.

`page_publish_operation_artifacts` has an artifact foreign key with `ON DELETE CASCADE`. Physical loss therefore removes the exact manifest row that `rollback_to_previous` historically requires to identify the current publish cursor.

After recovery the current binding points at the rebuilt artifact id, while the reviewed publish `artifact_set_hash` remains unchanged because its identity is locale + artifact hash + materialization hash. Retained publish provenance and exact rebuild/activation receipts already prove why the storage id changed.

The old rollback path rejected this valid current state before it could choose the previous distinct publish.

## Source change

The publish-manifest loader now has two ordered paths:

1. strict original manifest validation;
2. only after `RollbackTargetUnavailable`, a bounded current-cursor reconstruction from the exact active artifact set, retained publish provenance, and rebuild/activation receipts.

The fallback requires complete locale coverage, exact aggregate publish hash parity, revalidated provenance hashes, exact repair receipts for every changed artifact id, and at least one rebuilt/activated locale. Retained provenance by itself cannot replace a missing manifest.

Every surviving manifest row remains authoritative evidence and must still match retained provenance exactly. An unchanged locale must retain its original manifest row. If a repaired locale has lost its manifest row, the historical source artifact must be absent as well; a missing manifest with a still-live source artifact is not admitted as physical-loss recovery.

Database errors are not masked by fallback.

## Historical targets stay fail-closed

The fallback is intentionally current-state-only.

Rollback target selection skips publish receipts with the current artifact-set hash before loading a target. A historical target therefore cannot satisfy the active-set equality required by the recovery branch.

A prior publish whose manifest is missing remains unavailable even when its retained provenance survives. This preserves immutable target authority and prevents provenance from becoming a second historical artifact store.

## Page Builder registry parity correction

The Page Builder FBA registry is actualized in this slice because it lagged merged Pages source:

- Pages metadata no longer reports `PageMetadataEditor_pending_removal`;
- materialization persistence now records `instance_key` in the canonical uniqueness identity;
- explicit artifact audit/rebuild/activation/physical-loss recovery are recorded as Pages-owned source-ready boundaries;
- repair-to-rollback continuity records the strict-manifest-first/current-cursor-only rule, explicit repair-chain requirement, surviving-manifest/provenance identity fence and source-artifact-absent requirement for a missing repaired manifest row;
- execution evidence remains pending and FFA/FBA is not promoted.

## Actualized matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Metadata consumer-property cutover | Source-complete | Conflict/browser evidence pending |
| Reviewed publish + immutable manifest | Source-ready | Accepted runtime evidence pending |
| Immutable rollback | Source-ready | Database/request/browser evidence pending |
| Retained rebuild provenance | Source-ready | PostgreSQL execution pending |
| Explicit rebuild | Source-ready | SQLite/PostgreSQL execution pending |
| Existing-binding activation | Source-ready | Execution pending |
| Physical-loss missing-binding activation | Source-ready | PostgreSQL execution pending |
| Repair -> rollback current-cursor continuity | **Source-ready in this overlay** | PostgreSQL execution pending |
| Historical rollback target without original manifest | **Rejected by source contract** | PostgreSQL negative execution pending |
| Surviving current manifest identity mismatch | **Rejected by source contract** | PostgreSQL negative execution pending |
| Missing repaired manifest while source artifact still exists | **Rejected by source contract** | PostgreSQL negative execution pending |
| Automatic repair / rollback chaining | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Execute the repair-to-rollback PostgreSQL packet and retain successful physical-loss -> rebuild -> activation -> rollback evidence.
2. Retain the three negative results: historical-target missing manifest, surviving-manifest identity mismatch, and missing current manifest with a live source artifact.
3. Execute the previously source-ready activation recovery, provenance, rebuild, audit, repair atomicity/failure/cache and transport packets.
4. Retain metadata conflict/isolation, cache continuity, artifact/HTTP/browser and tenant Wave evidence.
5. Promote no FFA/FBA state until the accepted execution chain is complete.

## Validation boundary

Source review and authoring only. No tests, Node verifiers, Cargo commands, formatting, PostgreSQL scenarios, workflows or CI were executed.
