# Explicit Immutable Artifact-Loss Activation Recovery

Date: 2026-08-07  
Status: production-source-ready / direct-and-rollback-activated-single-and-multi-locale-postgres-harness-source-ready / execution-unvalidated

## Scope

Pages keeps rebuild and activation as two explicit tenant-admin operations. Recovery extends only `PageService::replace_rebuilt_artifact_binding` so an already rebuilt immutable artifact can be activated after the canonical source artifact row was physically lost and its locale binding was removed.

The recovery base remains one exact retained reviewed publish. That publish can be current in either of two traceable ways:

1. it is still current from its original reviewed publish result version; or
2. an exact later `page_rollback_operation` reactivated that same immutable publish artifact set.

A rollback receipt is only a **version activation anchor**. It never replaces retained publish provenance as immutable rebuild authority.

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

## Existing-binding path remains strict

If the locale binding exists, activation still requires:

- `binding.page_body_id == source.page_body_id`;
- `binding.artifact_id == expected_current_artifact_id`;
- the rebuild artifact is not already the bound artifact.

Any existing binding mismatch fails immediately. It never falls through into physical-loss recovery.

## Missing-binding recovery admission

A missing locale binding is accepted only when all of these additional facts hold inside the same owner transaction:

1. the retained source artifact identified by the rebuild receipt is absent for the exact tenant, page and locale;
2. the retained source page-body row still exists by `source.page_body_id` for that exact tenant, page and locale;
3. the retained source publish operation still exists by `source.operation_id` for the exact tenant and page;
4. that publish is also the rebuild receipt's `source_publish_operation_id`;
5. the common page-version fence has already proven `expected_version == current page.version`;
6. recovery selects an exact activation anchor for that publish;
7. every page version after the selected anchor is explained by the bounded sequential same-publish activation chain below.

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

If no matching rollback anchor exists, recovery falls back to the original publish anchor. The remaining version gap must then satisfy the same activation-chain proof; arbitrary stale versions do not become admissible.

## Sequential multi-locale version chain

When the selected activation anchor is older than `expected_version`, the whole post-anchor gap must be explained by prior `page_artifact_binding_replacement_operations` rows.

The recovery owner requires all of the following:

- the gap is positive and bounded to at most 256 prior activation steps;
- the database query is physically capped at 257 rows so corrupted duplicate evidence cannot create an unbounded receipt scan;
- the number of prior activation receipts in `(anchor_version, expected_version]` equals the exact version gap;
- ordered receipts form a contiguous chain where each `expected_version` equals the previous cursor and each `result_version == expected_version + 1`;
- every prior locale is unique and different from the locale currently being recovered;
- every prior activation request hash is recomputed from the canonical activation request identity;
- every prior activation's rebuild receipt and retained provenance are revalidated;
- every prior rebuild/provenance pair belongs to the exact same source publish as the locale currently being recovered;
- activation body, locale, source artifact id, replacement artifact id and replacement hashes exactly match that prior rebuild/provenance pair;
- each prior repaired locale binding still exists and still points at that exact rebuilt artifact;
- each prior rebuilt artifact still has the receipt-bound instance key, artifact hash and materialization hash.

Any unexplained lifecycle/version increment — including an unexplained increment after rollback — foreign publish, repeated locale, target-locale activation, changed binding, missing prior artifact or corrupt receipt keeps recovery fail-closed.

This remains a sequential command contract. Each activation changes only one locale and advances the page version once.

## Successful recovery semantics

After those fences pass, activation reuses the existing owner mutation:

```text
PageBuilderArtifactService::bind_existing_body_in_tx
```

That call recreates only the missing locale binding against the already existing retained body and the exact rebuilt immutable artifact. The command does not recreate the missing canonical source artifact, modify retained provenance, modify rollback/rebuild receipts, compile, sanitize or rebuild anything.

The activation receipt keeps `expected_current_artifact_id` as `previous_artifact_id`. In the recovery branch this is historical source identity, not a claim that a binding row existed immediately before activation.

## Forbidden shortcuts

The source continues to reject:

- missing binding while the retained source artifact still exists;
- existing mismatched binding with any fallback into recovery;
- absent retained source body;
- absent or mismatched source publish operation;
- rollback anchor that targets another publish or artifact set;
- rollback anchor with noncanonical request identity;
- any post-anchor version gap not completely explained by contiguous prior activations from the same publish;
- prior activation for the locale currently being recovered;
- duplicate prior recovery locales;
- prior repaired bindings or rebuilt artifacts that no longer match their receipts;
- timestamp-only selection of rebuild or publish history;
- mutable current draft content as repair authority;
- using rollback receipt payload as rebuild provenance;
- source-artifact recreation;
- combined rebuild + activation;
- inline cache mutation;
- automatic repair scheduling.

## PostgreSQL source packets

The direct single-locale packet remains:

```text
crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
```

It retains direct-publish single-locale success plus source-artifact-still-present and unexplained stale-version rejection.

The direct sequential multi-locale packet remains:

```text
crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
```

It retains two source scenarios:

1. two locales from one reviewed publish lose binding + manifest row + source artifact, both are explicitly rebuilt, the first activation starts at the publish version, and the second activation succeeds at the first activation's result version;
2. after the first locale activation, an unexplained direct page-version increment is inserted by the fixture and the second locale activation is rejected.

The rollback-activated packet is:

```text
crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-rollback-activated-artifact-loss-recovery.mjs
crates/rustok-pages/contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json
```

It retains three additional source scenarios:

1. publish A with `en` + `fr` -> publish B -> rollback to A -> physical loss of both A artifacts -> explicit rebuild -> `en` activation at rollback result version -> `fr` activation through one same-publish sequential receipt -> both rebuilt bindings active and exact replay remains idempotent;
2. a SHA-shaped but noncanonical rollback request hash rejects the first recovery without activation mutation;
3. an unexplained page-version increment after rollback rejects recovery because the post-anchor gap has no corresponding activation receipt.

## Validation boundary

This packet describes source authored in the accompanying PR. Execution evidence remains intentionally empty. No Rust tests, PostgreSQL/SQLite scenarios, Node verifiers, Cargo commands, formatting, workflows or CI were run by the authoring workflow.

The PostgreSQL packets and static guards are source-ready; maintainer execution and accepted evidence retention remain the next evidence cursor.
