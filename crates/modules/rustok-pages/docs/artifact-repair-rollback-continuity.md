# Explicit Artifact Repair Rollback Continuity

Date: 2026-08-07  
Status: source-ready / execution-unvalidated

## Problem

Pages rollback historically identifies the current immutable publication through the active artifact-set hash and then validates the corresponding `page_publish_operation_artifacts` manifest before selecting an older distinct publish.

Explicit immutable repair introduced a legitimate state where the current content identity still belongs to the same reviewed publish but one storage artifact id changed:

```text
reviewed publish B
-> immutable artifact B
-> physical artifact loss or explicit repair need
-> retained page_publish_rebuild_source survives
-> explicit rebuild appends B'
-> explicit activation binds B'
```

For physical loss, the original manifest row cannot remain because `page_publish_operation_artifacts.artifact_id` references the immutable artifact with `ON DELETE CASCADE`. The active repaired artifact still has the exact reviewed artifact/materialization hashes of publish B, but the original manifest can no longer prove its storage id.

Without a bounded recovery rule, `rollback_to_previous` fails before it can select publish A even though the current repaired binding is fully traceable through retained provenance, rebuild and activation receipts.

## New current-cursor rule

`load_publish_manifest_in_tx` keeps the original manifest path authoritative and executes it first.

Only when that strict path returns `RollbackTargetUnavailable` may the loader attempt current-cursor recovery. Database failures and other error classes are not converted into recovery.

The recovery path is accepted only when all of these facts hold in the rollback transaction:

1. the page is still published and its current version is not older than the selected publish result version;
2. the exact current binding set is readable and passes immutable artifact identity checks;
3. the current locale/hash/materialization set hashes to the selected publish operation's exact `artifact_set_hash`;
4. the selected publish has one complete retained `page_publish_rebuild_source` row per current locale;
5. every retained source revalidates its provenance hash and exact publish review hash;
6. the retained provenance set independently reproduces the same publish `artifact_set_hash`;
7. every surviving manifest row still matches the exact retained locale, source artifact id, artifact hash and materialization hash;
8. an unchanged locale may keep the exact original source artifact id only while its original manifest row still survives;
9. when a repaired locale has no surviving manifest row, its historical source artifact must also be absent; a missing manifest while the source artifact still exists is not physical-loss recovery;
10. any locale whose current artifact id differs from retained source identity must have one exact `page_artifact_rebuild_operations` receipt for that source and current artifact;
11. that rebuild receipt must reproduce the retained artifact/materialization hashes and exact `rebuild:<operation-id>` storage identity;
12. the current replacement artifact must have one exact `page_artifact_binding_replacement_operations` receipt for that rebuild;
13. the activation receipt must bind the same source body, locale, historical source artifact id, replacement artifact id and replacement hashes;
14. the activation result version must be after the source publish result and no later than the locked current page version;
15. at least one locale must be explicitly rebuilt and activated; provenance alone never heals a missing current manifest.

The fallback returns the **current verified artifact members**. It does not recreate or rewrite the historical manifest. A surviving manifest row with changed identity is an integrity conflict, not repair evidence, and remains fail-closed.

## Historical target remains strict

The fallback is not a general replacement for publish manifests.

`find_previous_publish_target_in_tx` skips any publish receipt whose `artifact_set_hash` equals the current set before it tries to load a rollback target. Therefore a historical target necessarily has a different artifact-set hash from the current bindings, and current-cursor recovery cannot admit it.

A historical target with a missing manifest row or missing immutable artifact remains `RollbackTargetUnavailable`, even when retained rebuild provenance exists for that old publish.

This preserves the existing rollback rule: target artifacts must still be live immutable records backed by their original publish manifest.

## Physical-loss continuity

The source packet covers this chain:

```text
publish A
-> unpublish
-> save reviewed document B
-> publish B
-> remove B binding + B manifest reference
-> physically delete B source artifact
-> explicit rebuild B'
-> explicit activation B'
-> rollback_to_previous
-> publish A restored
```

The rollback receipt must point to publish A, advance the page version once and remain exactly replayable.

The same packet also retains three negative boundaries:

- a historical target whose original manifest is missing remains unavailable;
- a surviving current manifest row whose identity no longer matches retained provenance is not healed by an otherwise valid rebuild/activation chain;
- a repaired current locale whose manifest row is missing while its source artifact still exists is not treated as physical-loss recovery.

## Preserved boundaries

This slice does not:

- add or modify migrations;
- recreate deleted source artifacts or manifest rows;
- allow retained provenance to become a historical rollback target by itself;
- allow provenance alone to replace a missing current manifest without an explicit repair receipt chain;
- ignore surviving manifest identity drift;
- infer source-artifact loss from a missing manifest row;
- change rollback DTOs, GraphQL, HTTP, OpenAPI or admin controls;
- combine repair and rollback into one command;
- schedule repair or rollback automatically;
- mutate caches inline;
- claim FFA/FBA promotion.

## Validation boundary

The accompanying PostgreSQL harness and Node source guard are source-only. Tests, verifiers, Cargo commands, formatting, PostgreSQL execution, workflows and CI were not run by the authoring workflow.
