# Pages / Page Builder Rollback-Activated Repair-to-Rollback Continuity Actualization

Date: 2026-08-07  
Status: current-source-overlay / rollback-activated-repair-rollback-continuity-source-ready / execution-open

This overlay continues `pages-page-builder-rollback-activated-recovery-actualization-2026-08-07.md` after PR #3184 made physical-loss recovery source-ready when an older reviewed publish became current again through an exact rollback receipt.

## Main recheck

The current source-ready owner chain now covers:

```text
reviewed publish
-> immutable manifest + retained rebuild provenance
-> explicit rollback activation of an older publish
-> physical source-artifact loss
-> explicit rebuild from retained reviewed provenance
-> explicit activation from the exact rollback activation anchor
```

The recheck found one remaining continuity mismatch after that new admission path.

## Confirmed source gap

Repair admission after rollback already accepts a **publish-or-rollback activation anchor**. If the selected publish was reactivated by rollback, the first recovery activation starts at `page_rollback_operations.result_version`.

Repair-aware rollback reconstruction still proved its minimal physical-loss activation prefix from the selected publish's original `page_publish_operations.result_version`.

For a valid history such as:

```text
publish P0
-> publish P1
-> publish P2
-> rollback P2 -> P1
-> lose P1 artifact
-> rebuild P1
-> activate rebuilt P1 at rollback.result_version
-> rollback repaired P1 -> P0
```

the activation receipt is intentionally **not** contiguous with P1's old publish result version. The intervening P2 publication and rollback are durable lifecycle history. Therefore the final rollback incorrectly rejected a current repaired P1 cursor even though repair admission had already proven the exact rollback-to-P1 activation anchor.

## Source change

`crates/rustok-pages/src/services/page/artifact_set.rs` now applies the same exact anchor contract when reconstructing the minimal physical-loss activation prefix.

The prefix anchor is:

1. the selected publish `result_version` when no later exact rollback-to-that-publish receipt exists; or
2. the latest rollback receipt at or before the current page version whose `target_publish_operation_id` and `target_artifact_set_hash` resolve to that exact publish.

A rollback anchor is accepted only when:

- tenant and page match the selected publish;
- `target_publish_operation_id` is exactly the selected publish id;
- `target_artifact_set_hash` is exactly the selected publish artifact-set hash;
- source and target artifact-set hashes are distinct;
- rollback result version is later than the source publish and no later than the current page version;
- request hash is valid SHA-256 and recomputes from the canonical rollback request identity using `expected_version = result_version - 1`.

The rollback receipt remains only activation/version evidence. It is not rebuild provenance or content authority.

## Minimal prefix remains strict

After the resolved publish-or-rollback activation anchor, rollback reconstruction retains all earlier physical-loss proof:

- at most 256 activation receipts;
- query physically bounded to 257 rows;
- contiguous `expected_version -> result_version` chain;
- unique locales until all required lost-manifest locales are proven;
- canonical activation request hashes;
- exact same-publish retained rebuild sources;
- exact rebuild receipt/provenance identity;
- exact receipt-bound rebuilt artifact instance/hash/materialization identity;
- stop as soon as all required physical-loss locales are proven, so later ordinary page-version work stays outside the prefix.

A missing or corrupted matching rollback anchor does not make the old publish version magically current. Reconstruction falls back to the publish anchor and therefore fails closed when the repair activation actually began later.

## Preserved boundaries

This slice does not change:

- strict original publish-manifest-first behavior;
- current-cursor-only repair fallback;
- propagation of database errors instead of masking them as repair fallback;
- complete retained provenance requirements;
- source-artifact-absence requirement for a missing repaired manifest row;
- exact rebuild and activation receipt requirements;
- full immutable artifact verification during rebinding;
- historical rollback target rule: original manifest and live immutable artifact records remain mandatory;
- migration, DTO, GraphQL, HTTP, OpenAPI, admin UI or cache contracts;
- automatic audit -> rebuild -> activation -> rollback behavior, which remains absent.

## PostgreSQL source packet

New execution-pending packet:

```text
crates/rustok-pages/tests/artifact_rollback_activated_repair_rollback_continuity_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-repair-rollback-continuity.mjs
crates/rustok-pages/contracts/evidence/pages-rollback-activated-repair-rollback-continuity-source.json
```

It retains two source scenarios:

1. **three-publish success** — `P0 -> P1 -> P2 -> rollback to P1 -> physical loss of P1 -> rebuild -> activation from rollback anchor -> rollback to P0`, followed by exact idempotent replay;
2. **durable anchor corruption** — after successful repair activation, replace the rollback anchor request hash with a different SHA-shaped value; the final rollback must reject without changing page version, binding, or creating another rollback receipt.

The packet is authored only. PostgreSQL was not executed.

## Actualized matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Direct-publish physical-loss recovery | Source-ready | PostgreSQL execution pending |
| Rollback-activated physical-loss recovery | Source-ready | PostgreSQL execution pending |
| Direct repaired-current rollback reconstruction | Source-ready | PostgreSQL execution pending |
| Rollback-activated repaired-current rollback reconstruction | **Source-ready in this overlay** | PostgreSQL execution pending |
| Canonical rollback-anchor hash revalidation during reconstruction | **Source-ready in this overlay** | PostgreSQL negative execution pending |
| Historical target provenance fallback | Deliberately absent | Not allowed |
| Automatic repair/rollback chaining | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Execute the rollback-activated repair-to-rollback PostgreSQL packet and retain success plus corrupted-anchor rejection evidence.
2. Execute the prior rollback-activated activation recovery, direct single-/multi-locale recovery and repair-to-rollback packets.
3. Execute prior provenance, audit, rebuild, repair atomicity/failure/cache, transport, metadata and browser packets.
4. Promote no FFA/FBA state until accepted execution evidence exists.
5. A separate future source design is still required if Pages should support recovery after physical loss of an already rebuilt replacement artifact; this slice does not generalize repair lineage to repeated locale recovery.

## Validation boundary

Source review and authoring only; execution remains pending. No tests, Node verifiers, Cargo commands, formatting, migrations, PostgreSQL scenarios, workflows, CI or `git diff --check` were executed by this authoring workflow.
