# Explicit Immutable Artifact-Loss Activation Recovery

Date: 2026-08-07  
Status: production-source-ready / single-and-multi-locale-postgres-harness-source-ready / execution-unvalidated

## Scope

Pages keeps rebuild and activation as two explicit tenant-admin operations. This recovery extends only `PageService::replace_rebuilt_artifact_binding` so already rebuilt immutable artifacts can be activated after canonical source artifact rows were physically lost and their locale bindings were necessarily removed first.

The original single-locale path remains intact. This revision additionally permits sequential recovery of multiple lost locales from the **same retained reviewed publish** without treating the page-version increments produced by earlier activation commands as unrelated drift.

No automatic audit-to-rebuild or rebuild-to-activation behavior is introduced.

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
4. that operation is also the rebuild receipt's `source_publish_operation_id`;
5. the common page-version fence has already proven `expected_version == current page.version`;
6. either `publish_operation.result_version == expected_version`, or every intervening page version is explained by the bounded sequential activation chain below.

The retained publish remains the historical-current authority. A page-version gap is never accepted merely because a source artifact disappeared.

Only retained body identity is consumed by the recovery decision. Mutable current draft content is not used as rebuild or activation authority.

## Sequential multi-locale version chain

When `publish_operation.result_version < expected_version`, the whole gap must be explained by prior `page_artifact_binding_replacement_operations` rows.

The recovery owner requires all of the following:

- the gap is positive and bounded to at most 256 prior activation steps;
- the number of prior activation receipts in `(publish.result_version, expected_version]` equals the exact version gap;
- ordered receipts form a contiguous chain where each `expected_version` equals the previous cursor and each `result_version == expected_version + 1`;
- every prior locale is unique and different from the locale currently being recovered;
- every prior activation request hash is recomputed from the canonical activation request identity;
- every prior activation's rebuild receipt and retained provenance are revalidated;
- every prior rebuild/provenance pair belongs to the exact same `source_publish_operation_id` as the locale currently being recovered;
- activation body, locale, source artifact id, replacement artifact id and replacement hashes exactly match that prior rebuild/provenance pair;
- each prior repaired locale binding still exists and still points at that exact rebuilt artifact;
- each prior rebuilt artifact still has the receipt-bound instance key, artifact hash and materialization hash.

Any unexplained lifecycle/version increment, foreign publish, repeated locale, target-locale activation, changed binding, missing prior artifact or corrupt receipt keeps recovery fail-closed.

This is intentionally a sequential command contract. Each activation still changes only one locale and advances the page version once.

## Successful recovery semantics

After those fences pass, activation reuses the existing owner mutation:

```text
PageBuilderArtifactService::bind_existing_body_in_tx
```

That call recreates only the missing locale binding against the already existing retained body and the exact rebuilt immutable artifact. The command does not recreate the missing canonical source artifact, modify retained provenance, modify rebuild receipts, compile, sanitize or rebuild anything.

The activation receipt keeps `expected_current_artifact_id` as `previous_artifact_id`. In the recovery branch this is historical source identity, not a claim that a binding row existed immediately before activation.

## Forbidden shortcuts

The source must continue to reject:

- missing binding while the retained source artifact still exists;
- existing mismatched binding with any fallback into recovery;
- absent retained source body;
- absent or mismatched source publish operation;
- source publish version drift not completely explained by contiguous prior activations from the same publish;
- prior activation for the locale currently being recovered;
- duplicate prior recovery locales;
- prior repaired bindings or rebuilt artifacts that no longer match their receipts;
- timestamp-based selection of rebuild or publish history;
- mutable current draft content as repair authority;
- source-artifact recreation;
- combined rebuild + activation;
- inline cache mutation;
- automatic repair scheduling.

## PostgreSQL source packets

The original single-locale packet remains:

```text
crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
```

It retains single-locale success plus source-artifact-still-present and unexplained stale-version rejection.

The sequential multi-locale packet is:

```text
crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-multilocale-activation-recovery-postgres.mjs
```

It retains two additional unexecuted scenarios:

1. two locales from one reviewed publish lose binding + manifest row + source artifact, both are explicitly rebuilt, the first activation starts at the publish version, and the second activation succeeds at the first activation's result version; both rebuilt bindings remain active and exactly four lifecycle events are emitted across the two activation commands;
2. after the first locale activation, an unexplained direct page-version increment is inserted by the fixture; the second locale activation is rejected because the full version gap is not explained by same-publish activation receipts.

## Validation boundary

This packet describes source authored in the accompanying PR. Execution evidence remains intentionally empty. No Rust tests, PostgreSQL/SQLite scenarios, Node verifiers, Cargo commands, formatting, workflows or CI were run by the authoring workflow.

The PostgreSQL packets and static guards are source-ready; maintainer execution and accepted evidence retention remain the next evidence cursor.
