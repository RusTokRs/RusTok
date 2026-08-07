# Pages / Page Builder Artifact-Loss Rebuild Continuation

Date: 2026-08-07  
Status: source-ready / artifact-loss-rebuild-postgres-harness-source-ready / execution-pending  
Scope: explicit immutable rebuild from retained reviewed provenance after the referenced source artifact row is physically absent

## Rechecked source state

The retained reviewed-publish provenance packet now proves at source level that `page_publish_rebuild_sources` deliberately survives loss of its referenced immutable artifact row. The explicit rebuild owner separately recompiles from retained provenance and appends a new operation-bound immutable artifact.

The missing combined evidence boundary was whether those two source contracts actually compose: after the canonical source artifact row is gone, can the explicit rebuild command still reproduce the reviewed publication without silently depending on that missing row?

This packet retains that combined PostgreSQL harness without changing production behavior.

Marker:

```text
artifact-loss-rebuild-postgres-harness-source-ready
```

Harness:

```text
crates/rustok-pages/tests/artifact_loss_rebuild_postgres.rs
```

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-artifact-loss-rebuild-postgres-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-rebuild-postgres.mjs
```

## PostgreSQL fixture

The harness is gated by `RUSTOK_PAGES_TEST_DATABASE_URL` with `DATABASE_URL` fallback and accepts PostgreSQL URLs only. It creates one unique schema, applies the real `OutboxModule` and `PagesModule` migrations, enables Pages for one tenant and drops the schema after the scenario.

The page is created and reviewed-published through the real `PageService`. Its reviewed body revision uses the current owner contract:

```text
body.updated_at
```

The test retains both:

- the exact `page_publish_rebuild_sources` model;
- the exact canonical `page_static_landing_artifacts` model.

## Physical source-artifact loss

To produce a constraint-valid missing-artifact state without disabling PostgreSQL integrity, the test first removes the two independent references that point to the selected canonical artifact:

1. the active published binding;
2. the publish-operation artifact-manifest row.

It then deletes the canonical artifact row itself.

After that deletion:

- the source artifact id is no longer present in `page_static_landing_artifacts`;
- the retained provenance row remains byte-for-byte unchanged and still records the missing artifact id;
- the page version and lifecycle status remain unchanged;
- no lifecycle event is added;
- the published binding remains absent.

This is deliberately a test-owned loss fixture. No production delete path or automatic corruption workflow is introduced.

## Explicit rebuild from retained provenance

The harness then invokes the real:

```text
PageService::rebuild_immutable_artifact
```

with the retained source id, exact provenance hash, a distinct idempotency key and the reviewed runtime context used by the original publication.

The production rebuild owner loads only `page_publish_rebuild_sources`. It verifies the retained provenance, re-sanitizes the retained project, re-materializes it with the explicitly reviewed context and appends a new immutable artifact through `PageBuilderArtifactService::append_rebuilt_in_tx`.

The source artifact row is not loaded as a prerequisite.

The resulting rebuild must retain the missing source artifact id in its receipt while producing a distinct rebuilt artifact id:

```text
source_artifact_id = <missing canonical artifact id>
rebuilt_artifact_id != source_artifact_id
instance_key = rebuild:<operation-id>
```

## Exact pre-loss reproduction

Before physical deletion, the harness retains the complete canonical artifact model in memory.

After explicit rebuild it loads the rebuilt artifact and normalizes only the expected storage-instance differences:

```text
id
instance_key
created_at
```

Every other model field must equal the exact pre-loss canonical model, including source/build/artifact/materialization identities, renderer/build/registry evidence, runtime snapshots, materialization identity, page/head data, document/body HTML, CSS, content hash and landing sections.

This is stronger than hash-only recovery evidence: it binds the rebuilt row to the exact canonical model that existed before physical artifact loss.

## Receipt and side-effect boundaries

One successful rebuild must add exactly:

- one rebuilt artifact row;
- one `page_artifact_rebuild_operations` receipt.

The receipt retains `source_artifact_id` as historical evidence even though that row is absent.

The rebuild must not:

- recreate the published binding;
- advance the page version;
- change page lifecycle status;
- modify the retained provenance row;
- emit `NodeUpdated`, `NodePublished` or any other lifecycle event;
- activate the rebuilt artifact automatically.

An exact replay with the same request must return the same operation and rebuilt artifact and add no second artifact, receipt or event.

## Migration ownership boundary

Migration `m20260806_000013_create_page_publish_rebuild_sources` has no artifact foreign key. That keeps retained provenance available after artifact loss.

Migration `m20260806_000014_add_explicit_artifact_rebuild` stores `source_artifact_id` on the rebuild receipt as evidence, but its receipt table has only two foreign keys:

```text
page_id   -> pages.id
source_id -> page_publish_rebuild_sources.id
```

There is intentionally no foreign key from `source_artifact_id` to `page_static_landing_artifacts`.

Therefore a successful rebuild receipt can truthfully retain the identity of a missing source artifact without requiring that row to be recreated first.

## Preserved boundaries

- No production service/entity/artifact code is changed.
- No migration or database schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- No audit-to-rebuild automation is introduced.
- No rebuild-to-activation automation is introduced.
- No binding recreation is performed by the rebuild command.
- No FFA/FBA promotion is made.

## Evidence state

Status remains:

```text
pages_artifact_loss_rebuild_postgres_source_unvalidated
```

`execution` is empty and every validation flag remains false. The PostgreSQL harness and source guard are intentionally not run in this slice.

## Updated repair matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Provenance survives source artifact row loss | Harness-ready | PostgreSQL execution pending |
| Explicit rebuild does not require source artifact row | Harness-ready | PostgreSQL execution pending |
| Exact pre-loss canonical model reproduction | Harness-ready | PostgreSQL execution pending |
| Missing source artifact id retained in rebuild receipt | Harness-ready | PostgreSQL execution pending |
| Rebuild leaves binding absent | Harness-ready | PostgreSQL execution pending |
| Rebuild preserves page version/status/events | Harness-ready | PostgreSQL execution pending |
| Exact replay after source artifact loss | Harness-ready | PostgreSQL execution pending |
| Automatic activation | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Execute the provenance and artifact-loss rebuild source guards and PostgreSQL harnesses and retain accepted evidence.
2. Execute the already source-ready SQLite/PostgreSQL immutable-artifact audit packets.
3. Execute the repair transport/request, PostgreSQL atomicity, negative SQLite and after-commit cache packets.
4. Retain successful bounded audit/repair transport evidence with current-tenant and Pages Manage fencing.
5. Execute the broader artifact/HTTP/browser and tenant Wave packets before FFA/FBA promotion.
6. Keep audit-to-rebuild and rebuild-to-activation chaining absent until accepted execution evidence supports any policy change.

## Maintainer validation

Suggested commands, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-artifact-loss-rebuild-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test artifact_loss_rebuild_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-publish-rebuild-provenance-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test publish_rebuild_provenance_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, PostgreSQL scenarios, GraphQL/HTTP requests, workflows and CI were intentionally not run.
