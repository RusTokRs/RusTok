# Explicit Immutable Artifact-Loss Activation Recovery

Date: 2026-08-07  
Status: production-source-ready / direct-rollback-activated-multi-locale-and-repeated-loss-postgres-harness-source-ready / execution-unvalidated

## Scope

Pages keeps rebuild and activation as two explicit tenant-admin operations. Recovery extends only `PageService::replace_rebuilt_artifact_binding` so an already rebuilt immutable artifact can be activated after the canonical source artifact row was physically lost and its locale binding was removed.

The recovery base remains one exact retained reviewed publish. That publish can be current in either of two traceable ways:

1. it is still current from its original reviewed publish result version; or
2. an exact later `page_rollback_operation` reactivated that same immutable publish artifact set.

A rollback receipt is only a **version activation anchor**. It never replaces retained publish provenance as immutable rebuild authority.

Repeated recovery is also explicit. If a rebuilt replacement is later physically lost, the tenant administrator may append another rebuild from the same retained publish provenance and activate it only after the prior rebuilt instance has disappeared. Durable rebuild and activation receipts remain lineage evidence; they are not content authority.

No automatic audit-to-rebuild, rollback-to-rebuild or rebuild-to-activation behavior is introduced.

## Common activation fences

The existing activation contract remains authoritative:

- tenant-wide `pages:manage`;
- exact tenant and page;
- positive incrementable `expected_version` equal to the locked current page version;
- page status remains `published`;
- exact rebuild operation id;
- valid retained provenance and rebuild receipt;
- rebuild source artifact equals `expected_current_artifact_id`;
- one activation receipt at most per rebuild;
- exact replacement owner, locale, operation-bound instance identity, artifact hash and materialization hash;
- complete replacement artifact integrity before binding mutation;
- page version advances exactly once per activation;
- exactly one `NodeUpdated` and one `NodePublished` are written in each owner transaction;
- cache effects remain event-driven after commit;
- exact replay returns the retained activation receipt without another mutation.

`expected_current_artifact_id` remains the historical source artifact identity from retained publish provenance. On a missing-binding recovery this is a provenance/version fence, not a claim that the source artifact is still bound immediately before activation.

## Existing-binding path remains strict

If the locale binding exists, activation still requires:

- `binding.page_body_id == source.page_body_id`;
- `binding.artifact_id == expected_current_artifact_id`;
- the rebuild artifact is not already the bound artifact.

Any existing binding mismatch fails immediately. It never falls through into physical-loss recovery. Repeated recovery therefore cannot be used to replace a live rebuilt binding through the ordinary path.

## Missing-binding recovery admission

A missing locale binding is accepted only when all of these additional facts hold inside the same owner transaction:

1. the retained source artifact identified by the rebuild receipt is absent for the exact tenant, page and locale;
2. the retained source page-body row still exists by `source.page_body_id` for that exact tenant, page and locale;
3. the retained source publish operation still exists by `source.operation_id` for the exact tenant and page;
4. that publish is also the rebuild receipt's `source_publish_operation_id`;
5. the common page-version fence has already proven `expected_version == current page.version`;
6. recovery selects an exact activation anchor for that publish;
7. every page version after the selected anchor is explained by the bounded sequential same-publish activation lineage below.

Only retained body identity is consumed by the recovery decision. Mutable current draft content is not used as rebuild or activation authority.

## Publish-or-rollback activation anchor

### Direct publish anchor

When `publish_operation.result_version == expected_version`, the original reviewed publish is the activation anchor. This preserves the original single-locale recovery path unchanged.

### Exact rollback activation anchor

When the source publish is older than `expected_version`, recovery looks for the latest rollback receipt that proves that exact publish set was reactivated later. The candidate must match:

- exact tenant and page;
- `target_publish_operation_id == publish_operation.id`;
- `target_artifact_set_hash == publish_operation.artifact_set_hash`;
- `result_version <= expected_version`.

The rollback receipt itself is revalidated before its `result_version` is admitted as an anchor:

- non-nil receipt identity and non-empty idempotency key;
- valid SHA-256 request/source/target artifact-set identities;
- source and target artifact-set hashes differ;
- target artifact-set hash still equals the retained publish artifact-set hash;
- rollback result version is newer than the original publish and no newer than the current expected page version;
- original rollback expected version is recovered as `rollback.result_version - 1`;
- the canonical rollback request hash is recomputed from:

```text
page_rollback_operation_v1
+ tenant_id
+ page_id
+ rollback_expected_version
+ target_publish_operation_id
```

A SHA-shaped but noncanonical rollback receipt is rejected.

If no matching rollback anchor exists, recovery falls back to the original publish anchor. The remaining version gap must then satisfy the same activation-lineage proof; arbitrary stale versions do not become admissible.

## Sequential multi-locale and repeated-loss version chain

When the selected activation anchor is older than `expected_version`, the whole post-anchor gap must be explained by prior `page_artifact_binding_replacement_operations` rows.

The recovery owner requires all of the following:

- the gap is positive and bounded to at most 256 prior activation steps;
- the database query is physically capped at 257 rows so corrupted duplicate evidence cannot create an unbounded receipt scan;
- the number of prior activation receipts in `(anchor_version, expected_version]` equals the exact version gap;
- ordered receipts form a contiguous chain where each `expected_version` equals the previous cursor and each `result_version == expected_version + 1`;
- every prior activation request hash is recomputed from the canonical activation request identity;
- every prior activation's rebuild receipt and retained provenance are revalidated;
- every prior rebuild/provenance pair belongs to the exact same source publish as the locale currently being recovered;
- activation body, locale, historical source artifact id, replacement artifact id and replacement hashes exactly match that prior rebuild/provenance pair;
- the chain tracks the **latest repair state per locale** rather than assuming each locale appears once;
- when a locale appears again, its previously activated rebuilt artifact must already be physically absent before the later activation receipt may supersede it in the lineage;
- for repeated recovery, the prior rebuilt instance is physically absent before a later receipt can supersede it;
- at the current version, the locale being recovered must have no binding and its latest prior rebuilt instance, if it appeared in the chain, must be physically absent;
- every other locale's latest repaired binding must remain active and must point to its latest rebuild receipt;
- every other latest rebuilt artifact must still have the receipt-bound instance key, artifact hash and materialization hash.

Any unexplained lifecycle/version increment — including an unexplained increment after rollback — foreign publish, repeated locale while its previous rebuilt artifact still exists, unexpected target binding, changed latest non-target binding, missing latest non-target artifact or corrupt receipt keeps recovery fail-closed.

This remains a sequential command contract. Each activation changes only one locale and advances the page version once.

## Repair-aware rollback reconstruction

Rollback still tries the original immutable publish manifest first. The retained-provenance fallback applies only to the currently active repaired publish cursor.

Its physical-loss activation prefix now mirrors the same latest-state-per-locale rules:

- the prefix begins at the exact direct-publish or rollback activation anchor;
- receipts remain contiguous, canonical and same-publish only;
- a repeated locale is accepted only after the earlier rebuilt instance is physically absent;
- a required missing-manifest locale is not considered proven merely because the locale appeared once: the prefix must reach the activation whose replacement artifact id equals the artifact that is **currently** bound for that locale;
- once every required current repaired locale is proven, each locale's latest rebuild represented in the prefix must match the current repaired artifact set and the latest rebuilt row must still match its receipt.

The verifier then stops at that minimal completion point. Later valid page-version changes remain outside the repair proof. Historical rollback targets never use this fallback and still require original manifests plus live immutable artifacts.

## Successful recovery semantics

After those fences pass, activation reuses the existing owner mutation:

```text
PageBuilderArtifactService::bind_existing_body_in_tx
```

That call recreates only the missing locale binding against the retained body and the exact newest rebuilt immutable artifact. The command does not recreate the missing canonical source artifact, modify retained provenance, modify historical rollback/rebuild/activation receipts, compile, sanitize or rebuild anything.

## Forbidden shortcuts

The source continues to reject:

- missing binding while the retained source artifact still exists;
- existing mismatched binding with any fallback into recovery;
- absent retained source body;
- absent or mismatched source publish operation;
- rollback anchor that targets another publish or artifact set;
- rollback anchor with noncanonical request identity;
- any post-anchor version gap not completely explained by contiguous prior activations from the same publish;
- repeated locale while the prior rebuilt instance still exists;
- repeated recovery while the target locale binding is present;
- changed latest non-target repaired binding;
- missing or drifted latest non-target rebuilt artifact;
- timestamp-only selection of rebuild or publish history;
- mutable current draft content as repair authority;
- using rollback receipt payload as rebuild provenance;
- source-artifact recreation;
- combined rebuild + activation;
- inline cache mutation;
- automatic repair scheduling.

## PostgreSQL source packets

Direct single-locale:

```text
crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
```

Direct sequential multi-locale:

```text
crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
```

Rollback-activated recovery:

```text
crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs
crates/rustok-pages/contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json
```

Repeated artifact loss:

```text
crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-repeated-artifact-loss-recovery.mjs
crates/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-source.json
```

The repeated-loss packet retains four source scenarios:

1. the same locale recovers from source loss to `R1`, then from physical loss of `R1` to `R2`, with exact activation replay remaining idempotent;
2. deleting only the binding while leaving `R1` alive rejects repeated recovery;
3. another locale can recover after the first locale has been recovered twice, proving latest-state-per-locale chain validation;
4. rollback to an older publish succeeds after the current locale was recovered twice, proving rollback reconstruction reaches the current replacement rather than stopping at the first locale occurrence.

## Validation boundary

This packet describes source authored in the accompanying PR. Execution evidence remains intentionally empty. No Rust tests, PostgreSQL/SQLite scenarios, Node verifiers, Cargo commands, formatting, workflows or CI were run by the authoring workflow.

The PostgreSQL packets and static guards are source-ready; maintainer execution and accepted evidence retention remain the next evidence cursor.
