# Pages / Page Builder Artifact-Loss Activation Recovery Actualization

Date: 2026-08-07  
Status: source-gap-confirmed / missing-binding-activation-recovery-open / implementation-next / execution-pending  
Scope: actualize the Pages repair plan after the retained-provenance and source-artifact-loss rebuild packets merged

## Why this actualization exists

The canonical Page Builder / Pages parity overlay predates the newest repair packets and now understates source readiness in two places:

- reviewed-publish rebuild provenance already has a dedicated PostgreSQL harness/guard packet;
- explicit immutable rebuild after physical loss of the referenced source artifact already has a dedicated PostgreSQL harness/guard packet.

Those packets are still source-only and unvalidated, but they close the previously open scaffolding gaps.

The combined source recheck also exposes one remaining functional recovery gap that is more important than adding another execution scaffold: after physical loss of the source artifact row, explicit rebuild can reproduce the immutable artifact, but the current explicit activation owner cannot activate it because the current locale binding is absent.

This document supersedes the stale repair/rebuild cursor wording in `docs/modules/page-builder-parity-actualization-2026-08-05.md` where that wording conflicts with the source state below. It does not convert any source-ready packet into executed evidence.

## Source state already merged

### Reviewed-publish provenance

Merged packet:

```text
crates/rustok-pages/tests/publish_rebuild_provenance_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance-postgres.mjs
crates/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-postgres-source.json
```

Status:

```text
pages_publish_rebuild_provenance_postgres_source_unvalidated
```

The packet is source-ready for:

- exact two-locale retained provenance capture;
- body `updated_at` revision binding;
- retained body/artifact/materialization/review identities;
- `artifact_set_hash` mismatch rollback;
- `sanitized_set_hash` mismatch rollback;
- zero receipt/manifest/provenance side effects on aggregate mismatch;
- provenance survival after referenced artifact-row loss;
- migration 000013 no-artifact-FK and no-legacy-backfill boundaries.

### Explicit rebuild after source artifact loss

Merged packet:

```text
crates/rustok-pages/tests/artifact_loss_rebuild_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-rebuild-postgres.mjs
crates/rustok-pages/contracts/evidence/pages-artifact-loss-rebuild-postgres-source.json
```

Status:

```text
pages_artifact_loss_rebuild_postgres_source_unvalidated
```

The packet is source-ready for:

- constraint-valid removal of the active binding and publish-manifest reference;
- physical deletion of the canonical source artifact row;
- unchanged retained provenance after that loss;
- explicit rebuild without loading the missing source artifact row;
- exact reproduction of the pre-loss canonical artifact model except storage instance identity and creation timestamp;
- a rebuild receipt that retains the missing historical `source_artifact_id`;
- no binding recreation, page-version change or lifecycle events during rebuild;
- exact rebuild replay after source-artifact loss.

## Rechecked activation owner

The existing explicit activation owner remains:

```text
PageService::replace_rebuilt_artifact_binding
```

It correctly protects the normal replacement case with all of these fences:

- tenant-wide `pages:manage`;
- exact page version;
- page must still be published;
- exact rebuild receipt and retained provenance;
- `rebuild.source_artifact_id == expected_current_artifact_id`;
- one activation receipt per rebuild;
- current locale binding must exist;
- current binding body must equal retained provenance body;
- current binding artifact must equal the expected/source artifact;
- replacement artifact must exactly match the rebuild receipt;
- one-locale binding update, page version +1 and durable `NodeUpdated` + `NodePublished` in one owner transaction.

The normal path must not be weakened.

## Confirmed missing-binding recovery gap

`load_binding_for_update_in_tx` currently returns an error when the locale binding is absent:

```text
PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT
current locale binding is unavailable
```

That is correct for ordinary replacement.

However, the physical source-artifact-loss fixture must remove the binding before deleting the source artifact because the binding independently references that artifact. After the source row is gone:

```text
retained provenance: present
source artifact row: absent
current locale binding: absent
rebuilt artifact row: present after explicit rebuild
page: still published at the same version
```

The rebuilt artifact is therefore intentionally not activated by `rebuild_immutable_artifact`, but the existing explicit activation command has no bounded recovery branch that can restore the missing binding.

This is now the primary remaining source-level repair gap.

## Required recovery contract

The next implementation should reuse the existing explicit activation intent and receipt surface rather than introduce automatic repair. A missing binding may be admitted only as a narrow recovery case with all of the following conditions.

### Common fences retained

The existing activation command must continue to require:

- tenant-wide `pages:manage`;
- exact tenant and page;
- positive incrementable `expected_version`;
- current page version equals `expected_version`;
- page status is `published`;
- exact rebuild operation id;
- valid retained provenance and rebuild receipt;
- `expected_current_artifact_id == rebuild.source_artifact_id`;
- no prior activation receipt for the rebuild;
- exact replacement artifact identity and materialization evidence.

### Existing-binding path

When the locale binding exists, behavior remains exactly as today:

- binding body id must equal `source.page_body_id`;
- binding artifact id must equal `expected_current_artifact_id`;
- rebuilt artifact must not already be bound;
- any mismatch fails closed.

No fallback from an existing mismatched binding into recovery is allowed.

### Missing-binding recovery path

When the locale binding is absent, activation may proceed only if all of these additional facts are true:

1. the retained source artifact row identified by `rebuild.source_artifact_id` is actually absent;
2. the retained source page body still exists under the exact tenant/page/locale and its id equals `source.page_body_id`;
3. the retained source publish operation still exists and equals `source.operation_id` / `rebuild.source_publish_operation_id`;
4. that publish operation belongs to the same tenant and page;
5. `publish_operation.result_version == expected_version == current page.version`.

The version equality is the critical historical-current fence. Without an existing binding, the command must not infer that an arbitrary old rebuild is current merely because its source artifact disappeared. The retained publish receipt must identify the exact currently published page version.

If the binding is missing while the source artifact row still exists, activation must fail closed. That state is not the physical-artifact-loss recovery case and must not be silently treated as equivalent.

If the retained publish operation is older than the current page version, activation must fail closed even when the source artifact row is absent.

## Successful recovery semantics

Once the missing-binding recovery fences pass, the command may reuse the existing owner mutation:

```text
PageBuilderArtifactService::bind_existing_body_in_tx
```

The successful transaction must preserve existing activation semantics:

- create the one locale binding to the rebuilt artifact;
- advance page version exactly once;
- keep page status `published`;
- emit exactly one `NodeUpdated` and one `NodePublished`;
- store one `page_artifact_binding_replacement_operations` receipt;
- keep retained provenance and the rebuild receipt unchanged;
- never recreate the missing source artifact row;
- never call cache infrastructure inline;
- allow the existing event-driven cache handler to rotate generations only after commit;
- exact replay returns the retained activation receipt without another binding/version/event mutation.

The existing result field `previous_artifact_id` remains the retained historical source artifact identity supplied as `expected_current_artifact_id`; in the missing-binding recovery branch it is not a claim that a binding row was observed immediately before mutation.

## Explicitly forbidden shortcuts

The implementation must not:

- accept a missing binding while the source artifact row still exists;
- accept a source publish receipt whose `result_version` differs from the current page version;
- fall back from an existing mismatched binding into the missing-binding branch;
- select the newest rebuild or publish receipt by timestamp;
- infer currentness from locale alone;
- recreate the canonical source artifact row;
- modify retained provenance;
- combine rebuild and activation into one command;
- make audit schedule rebuild or activation;
- introduce automatic retry/repair policy;
- bypass the replacement-artifact integrity verifier;
- rotate cache generations before the committed lifecycle events are processed.

## Required source packet with implementation

The implementation PR should add a dedicated PostgreSQL packet for the complete physical-loss recovery chain:

```text
reviewed publish
-> remove binding/manifest references
-> delete source artifact
-> retained provenance survives
-> explicit rebuild
-> explicit activation restores missing binding
```

The packet should require:

- exact source publish `result_version` fence;
- successful missing-binding recovery;
- one restored locale binding;
- page version +1 only at activation;
- exact `NodeUpdated` + `NodePublished` pair;
- source artifact remains absent;
- retained provenance unchanged;
- rebuild receipt unchanged;
- exact activation replay;
- failure when binding is missing but source artifact still exists;
- failure when retained source publish version is stale relative to current page version.

A fail-closed source guard should bind those scenarios to the production owner and ensure the normal existing-binding path remains present.

Evidence must remain source-unvalidated until maintainer execution.

## Actualized repair matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed-publish rebuild provenance owner | Source-ready | Runtime evidence pending |
| Provenance PostgreSQL exact-capture/rollback/loss packet | Harness + guard ready | PostgreSQL execution pending |
| Immutable artifact audit SQLite/PostgreSQL packets | Harness + guard ready | Execution pending |
| Explicit append-only rebuild owner | Source-ready | Runtime evidence pending |
| Repair PostgreSQL atomicity / negative SQLite / cache packets | Harness + guard ready | Execution pending |
| Repair publish-revision fixtures | Owner-aligned | Execution pending |
| Explicit rebuild after physical source-artifact loss | Harness + guard ready | PostgreSQL execution pending |
| Existing-binding activation | Source-ready | Execution pending |
| Missing-binding activation after physical source-artifact loss | **Source gap confirmed** | Not implemented |
| Repair GraphQL/HTTP/OpenAPI transport contracts | Source-ready | Execution pending |
| Automatic audit-to-rebuild | Deliberately absent | Not allowed |
| Automatic rebuild-to-activation | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Actualized next cursor

1. Implement the bounded missing-binding recovery branch in explicit rebuilt-artifact activation with the exact source-artifact-absent, retained-body and source-publish-version fences above.
2. Add the dedicated PostgreSQL source packet and fail-closed guard for physical loss -> rebuild -> explicit activation, including stale-publish-version and source-artifact-still-present rejection cases.
3. Then run, outside this source-authoring workflow, the provenance, artifact-loss rebuild, audit, repair atomicity/failure/cache and activation-recovery packets and retain accepted evidence.
4. Retain generated/request-level GraphQL/HTTP/OpenAPI repair execution evidence with current-tenant and Pages Manage fencing.
5. Run static publish resource-limit accepted policy evidence plus metadata conflict/isolation, cache continuity, artifact/HTTP/browser and tenant Wave packets before any FFA/FBA promotion.
6. Keep audit-to-rebuild and rebuild-to-activation automation absent unless a later accepted policy explicitly introduces it.

## Validation boundary

This actualization is source review and planning only.

No production Rust, DTO, migration, schema, GraphQL, HTTP or OpenAPI source is changed by this document. No tests, source guards, Cargo commands, formatting, PostgreSQL/SQLite scenarios, request dispatch, lifecycle/cache handling, workflows or CI are executed. No FFA/FBA promotion is made.
