# Explicit Immutable Artifact-Loss Activation Recovery

Date: 2026-08-07  
Status: production-source-ready / postgres-harness-source-ready / execution-unvalidated

## Scope

Pages keeps rebuild and activation as two explicit tenant-admin operations. This recovery extends only `PageService::replace_rebuilt_artifact_binding` so an already rebuilt immutable artifact can be activated after the canonical source artifact row was physically lost and its locale binding was necessarily removed first.

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
- page version advances exactly once;
- exactly one `NodeUpdated` and one `NodePublished` are written in the owner transaction;
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
5. `publish_operation.result_version == expected_version`, and the common page-version fence has already proven `expected_version == current page.version`.

The publish-result equality is the historical-current fence. An old rebuild cannot become current merely because its source artifact disappeared.

Only the retained body identity is consumed by the recovery decision. Mutable current draft content is not used as rebuild or activation authority.

## Successful recovery semantics

After those fences pass, activation reuses the existing owner mutation:

```text
PageBuilderArtifactService::bind_existing_body_in_tx
```

That call recreates only the missing locale binding against the already existing retained body and the exact rebuilt immutable artifact. The command does not recreate the missing canonical source artifact, modify retained provenance, modify the rebuild receipt, compile, sanitize or rebuild anything.

The activation receipt keeps `expected_current_artifact_id` as `previous_artifact_id`. In the recovery branch this is historical source identity, not a claim that a binding row existed immediately before activation.

## Forbidden shortcuts

The source must continue to reject:

- missing binding while the retained source artifact still exists;
- existing mismatched binding with any fallback into recovery;
- absent retained source body;
- absent or mismatched source publish operation;
- source publish `result_version` different from the current expected page version;
- timestamp-based selection of rebuild or publish history;
- mutable current draft content as repair authority;
- source-artifact recreation;
- combined rebuild + activation;
- inline cache mutation;
- automatic repair scheduling.

## PostgreSQL source packet

The companion source packet is:

```text
crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-activation-recovery-postgres.mjs
```

It retains three unexecuted scenarios:

1. physical source loss -> explicit rebuild -> explicit activation restores one binding, advances the page version once, emits exactly two lifecycle events, leaves the source artifact absent, preserves provenance/rebuild receipt and replays exactly;
2. missing binding while the source artifact still exists is rejected with no activation receipt or lifecycle events;
3. a retained source publish receipt whose `result_version` is stale relative to the locked current page version is rejected even when the request uses that current version.

## Validation boundary

This packet describes source authored in the accompanying PR. Execution evidence remains intentionally empty. No Rust tests, PostgreSQL/SQLite scenarios, Node verifiers, Cargo commands, formatting, workflows or CI were run by the authoring workflow.

The PostgreSQL packet and its static guard are source-ready; maintainer execution and accepted evidence retention remain the next evidence cursor. The negative cases are source-artifact-still-present and stale-source-publish-version.
