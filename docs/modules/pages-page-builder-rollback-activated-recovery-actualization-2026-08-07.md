# Pages / Page Builder Rollback-Activated Artifact-Loss Recovery Actualization

Date: 2026-08-07  
Status: current-source-overlay / rollback-activated-artifact-loss-recovery-source-ready / execution-open

This overlay continues `pages-page-builder-multilocale-rollback-evidence-actualization-2026-08-07.md` after PR #3181 made rollback reconstruction revalidate the same durable multi-locale physical-loss activation evidence required by activation admission.

## Main recheck

The previous overlay left one explicit source gap: a historical publish can become current again through `page_rollback_operations`, but missing-binding physical-loss recovery still anchored its version proof only at the source publish's original `result_version`.

That rejected a legitimate current state after rollback because rollback advances `pages.version` while restoring the exact older immutable publish set.

The source-ready owner chain now covers both ways that an immutable publish set can become the current repair base:

```text
reviewed publish
-> direct publish activation anchor
   OR exact rollback-to-that-publish activation anchor
-> physical source-artifact loss
-> explicit retained-provenance rebuild
-> explicit one-locale missing-binding activation
-> bounded sequential same-publish activation for additional lost locales
-> repair-aware rollback reconstruction
```

Execution remains separate and pending.

## Exact rollback activation anchor

`PageService::replace_rebuilt_artifact_binding` still loads the retained source publish and requires the rebuild/provenance source to identify that exact publish.

When the source publish `result_version` is older than the current expected page version, recovery now looks for the latest rollback receipt that simultaneously matches:

- the same tenant and page;
- `target_publish_operation_id == source publish id`;
- `target_artifact_set_hash == source publish artifact_set_hash`;
- `result_version <= current expected_version`.

The matching rollback receipt is then independently validated before it may become the activation anchor:

- non-nil identity and non-empty idempotency key;
- valid request/source/target SHA-256 identities;
- source and target artifact-set hashes must differ;
- target artifact-set hash must still equal the selected publish receipt;
- rollback `result_version` must be newer than the source publish and no newer than the current expected page version;
- the canonical rollback request hash is recomputed from:

```text
page_rollback_operation_v1
+ tenant_id
+ page_id
+ (rollback.result_version - 1)
+ target_publish_operation_id
```

A SHA-shaped but noncanonical rollback receipt is rejected.

The rollback receipt is only an **activation anchor**. It is not repair-source authority: retained publish provenance remains the immutable rebuild authority and the historical source artifact must still be physically absent for missing-binding recovery.

## Post-anchor version fence

If the current expected version is later than the selected activation anchor, every intervening version must still be explained by the existing sequential physical-loss recovery contract.

The chain remains fail-closed:

- at most 256 accepted activation steps;
- the database scan is physically capped at 257 rows;
- exact contiguous `expected_version -> result_version` steps;
- unique prior locales and no prior activation for the current target locale;
- canonical activation request hashes;
- exact same-publish rebuild receipts and retained provenance;
- prior repaired bindings still active on the receipt-bound rebuilt artifacts;
- rebuilt artifact instance key, artifact hash and materialization hash still exact.

Therefore rollback only resets the recovery **version anchor**. It does not permit arbitrary metadata, lifecycle, fixture or unrelated version movement after rollback. Unexplained post-rollback version drift remains rejected.

## Preserved boundaries

This slice does not:

- weaken the existing-binding activation path;
- admit a missing binding while the historical source artifact still exists;
- use mutable current draft content as rebuild authority;
- make a rollback receipt a rebuild source;
- recreate deleted source artifacts or publish manifest rows;
- add a new schema or migration;
- change DTO, GraphQL, HTTP, OpenAPI, admin UI or cache contracts;
- automatically trigger rebuild after rollback or activation after rebuild;
- change historical rollback target strictness;
- promote FFA/FBA.

The direct-publish recovery path remains valid. If no exact rollback activation anchor exists, the owner falls back to the original publish anchor; any unexplained version gap from that publish still fails the existing contiguous-activation proof.

## PostgreSQL source packet

New execution-pending packet:

```text
crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs
crates/rustok-pages/contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json
```

It retains three source scenarios:

1. publish A with `en` + `fr` -> publish B -> rollback to A -> physical loss of both A artifacts -> explicit rebuild of both -> activate `en` directly at the rollback version -> activate `fr` through the same-publish sequential chain -> both rebuilt bindings active and exact replay remains idempotent;
2. the rollback receipt request hash is changed to a different SHA-shaped value -> first missing-binding activation is rejected with no page-version, binding or activation-receipt mutation;
3. rollback to A -> source loss/rebuild -> unexplained direct page-version increment -> recovery is rejected because no activation receipt explains the post-anchor version gap.

The packet is source-only and was not executed.

## Actualized matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Direct-publish single-locale physical-loss activation | Source-ready | PostgreSQL execution pending |
| Direct-publish multi-locale sequential recovery | Source-ready | PostgreSQL execution pending |
| Rollback-to-publish activation anchor | **Source-ready in this overlay** | PostgreSQL execution pending |
| Multi-locale physical-loss recovery after rollback activation | **Source-ready in this overlay** | PostgreSQL execution pending |
| Canonical rollback anchor request-hash revalidation | **Source-ready in this overlay** | PostgreSQL negative execution pending |
| Unexplained post-rollback version drift | **Rejected by source contract** | PostgreSQL negative execution pending |
| Repair-aware rollback reconstruction | Source-ready | PostgreSQL execution pending |
| Historical rollback target provenance fallback | Deliberately absent | Not allowed |
| Automatic rollback -> rebuild -> activation chaining | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

No additional source-level parity gap is promoted by this slice. The canonical next cursor returns to accepted execution evidence:

1. execute the rollback-activated recovery PostgreSQL packet and retain both success and negative evidence;
2. execute the direct single-/multi-locale activation recovery and multi-locale repair-to-rollback packets;
3. execute the prior provenance, rebuild, audit, failure/atomicity/cache and repair transport packets;
4. retain metadata conflict/isolation, cache, artifact/HTTP/browser and tenant Wave evidence;
5. promote no FFA/FBA state until accepted evidence exists.

A separate future design would be required for loss of an already rebuilt replacement artifact; this slice deliberately does not generalize repair lineage or permit repeated-locale recovery from a replacement artifact.

## Validation boundary

Source review and authoring only; execution remains pending. No tests, Node verifiers, Cargo commands, formatting, migrations, PostgreSQL scenarios, workflows, CI or `git diff --check` were executed.
