# Pages / Page Builder Selected Immutable Artifact Packet

Date: 2026-08-05
Status: source-ready / execution-pending
Scope: Pages owner persistence and public immutable-artifact selection after a persisted draft document mutation

## Problem closed at source level

Reviewed publication already stores an immutable Page Builder artifact and a locale binding. The public reader resolves that binding and verifies the referenced artifact, but the retained route packets did not isolate one important authoring invariant:

> A persisted current Fly body can advance after publication without becoming public render authority. Storefront remains bound to the selected immutable published artifact until a later reviewed publish or rollback changes the binding.

This packet adds a focused owner-level SQLite regression without changing production code.

## Retained sequence

```text
PageService::create
  → current Fly body contains "Published immutable artifact"
PageService::publish_reviewed
  → verified immutable artifact
  → page_published_landing_artifacts binding
exact-locale public read (en)
  → selected immutable artifact A
fallback public read (fr → en)
  → selected immutable artifact A
persist page_bodies.content with "Draft-only mutation"
  → current document revision/content changes
  → published binding still points to artifact A
exact-locale public read (en)
  → artifact A hash and document_html unchanged
fallback public read (fr → en)
  → artifact A hash and document_html unchanged
  → draft marker is absent from both public results
```

## Owner boundary

`PageBuilderArtifactService::load_public_bound_artifact_with_fallback` remains the public selection authority. In one transaction it:

1. requires the page to remain published;
2. applies channel visibility;
3. builds the exact/fallback locale candidate order;
4. resolves the current locale body identity;
5. resolves `page_published_landing_artifacts` by body identity;
6. loads the referenced immutable artifact by binding artifact ID;
7. reconstructs and verifies the stored Page Builder artifact/materialization envelope;
8. returns only the verified immutable payload.

The current `page_bodies.content` value is not rendered by this public path. It is used only to identify the locale body whose separate published binding owns the immutable selection.

## Source evidence

- `crates/rustok-pages/tests/selected_immutable_published_artifact_sqlite.rs`
- `crates/rustok-pages/contracts/evidence/pages-selected-immutable-artifact-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-selected-immutable-artifact.mjs`
- `crates/rustok-pages/src/services/page_builder_artifact.rs`
- `docs/modules/pages-page-builder-parity-continuation-plan.md`
- `crates/rustok-pages/docs/implementation-plan.md`

## Boundaries

This slice does not:

- change Pages or Page Builder production code;
- change publication, rollback, binding or lifecycle behavior;
- change event delivery or cache invalidation;
- touch optional Iggy infrastructure;
- change DTOs, migrations, routes, cache keys, namespaces or TTL;
- claim execution, browser, workflow, CI or rollout evidence;
- promote FFA or FBA status.

## Maintainer validation

Commands intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-selected-immutable-artifact.mjs
node crates/rustok-pages/scripts/verify/verify-pages-native-storefront-reviewed-artifact.mjs

cargo test -p rustok-pages \
  --test selected_immutable_published_artifact_sqlite -- --nocapture

cargo check -p rustok-pages --all-targets
```

Execution evidence remains pending.
