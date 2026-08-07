# Pages / Page Builder Explicit Repair Failure Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-failure-harness-source-ready / execution-pending  
Scope: negative owner-command evidence for explicit immutable rebuild and rebuilt-artifact activation

## Rechecked source state

Current `main` already contains:

- retained reviewed publish rebuild provenance;
- the explicit append-only immutable artifact rebuild owner command;
- the separate explicit rebuilt-artifact activation owner command;
- bounded GraphQL/HTTP/OpenAPI repair transports;
- generated transport and request-level contract harnesses;
- a PostgreSQL real-migration repair atomicity harness.

The next source gap was the failure matrix that must reject before repair side effects. The existing positive SQLite and PostgreSQL regressions do not retain all five rejection paths in one focused packet.

## Failure harness

Marker:

```text
explicit-artifact-repair-failure-harness-source-ready
```

`crates/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs` adds five isolated SQLite regressions. Each test creates its own in-memory database, applies the real Pages/Channel migrations plus the canonical `sys_events` migration, creates and reviewed-publishes a Page Builder page using the current canonical body revision shape, and snapshots durable repair state before the rejected command.

The snapshot includes:

- rebuild receipt count;
- activation receipt count;
- immutable artifact count;
- current locale binding artifact id;
- page version;
- page lifecycle status;
- durable outbox row count.

Every negative scenario requires the complete snapshot to remain unchanged across the rejected repair request.

## Rebuild provenance corruption

The retained `page_publish_rebuild_sources.provenance_hash` is deliberately replaced with another syntactically valid SHA-256 value. The request supplies that stored value so validation reaches the retained-source integrity check rather than failing input syntax.

The owner must reject with the `PAGE_ARTIFACT_REBUILD_SOURCE_INVALID` semantic carried by `PagesError::PublishOperationIntegrity`.

The failed rebuild must not:

- insert a rebuild receipt;
- append an artifact;
- change the active binding;
- advance the page version;
- change lifecycle state;
- append a durable event;
- repair or rewrite the corrupted provenance row.

## Reviewed runtime mismatch

A second valid `PageBuilderReviewedPublishRuntime` is created with a different reviewed scenario/context. The selected provenance row itself remains valid.

The rebuild owner must reject the request as `PagesError::PublishRuntimeReviewInvalid` before an artifact or receipt is appended.

The same complete durable state snapshot must remain unchanged.

## Activation stale version

A normal append-only rebuild fixture is created first. The fixture itself asserts the rebuild adds exactly one receipt and one artifact while leaving the binding, page version/status and event count unchanged.

Activation is then called with an expected page version different from the locked current version. The owner must return `PagesError::VersionConflict` with the exact expected and actual values.

No activation receipt, binding mutation, page mutation or event write may occur.

## Activation invalid replacement

A normal rebuilt artifact is deliberately corrupted after rebuild by changing its stored artifact hash while leaving the rebuild receipt intact.

Activation must reject the candidate through `PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID` before `bind_existing_body_in_tx` can switch the locale binding.

The test-owned corruption remains present, but the activation request itself must add no receipt and change no binding, page or outbox state.

## Activation unpublished page

A normal rebuilt fixture is explicitly unpublished through the Pages owner lifecycle before activation. The post-unpublish state is then snapshotted.

Activation supplies the current unpublished page version, so it reaches the published-state fence rather than a stale-version fence. The owner must reject through `PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT`.

The rejected request must leave the post-unpublish binding, page version/status, receipt counts and outbox count unchanged.

## Preserved boundaries

This packet does not:

- change rebuild or activation production code;
- change migrations or database schema;
- change GraphQL, HTTP or OpenAPI adapters;
- execute cache invalidation handling;
- add automatic audit-to-rebuild behavior;
- add automatic rebuild-to-activation behavior;
- promote FFA or FBA.

## Evidence

Machine source evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-failures-source.json
```

Status remains:

```text
pages_explicit_artifact_repair_failures_source_unvalidated
```

Execution is empty and every validation flag remains false until maintainer execution.

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-failures.mjs
```

The guard is intentionally not run in this slice.

## Updated parity matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Rebuild provenance corruption rejection | Harness-ready | Execution pending |
| Rebuild reviewed-runtime mismatch rejection | Harness-ready | Execution pending |
| Rebuild failure zero-side-effect snapshot | Harness-ready | Execution pending |
| Activation stale-version rejection | Harness-ready | Execution pending |
| Activation invalid-replacement rejection | Harness-ready | Execution pending |
| Activation unpublished-page rejection | Harness-ready | Execution pending |
| Activation failure zero-side-effect snapshots | Harness-ready | Execution pending |
| Positive PostgreSQL receipt/transaction atomicity | Harness-ready | Execution pending |
| Committed activation cache-generation rotation | Source-owned | Observation pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Run the transport schema, request-contract, PostgreSQL repair and negative SQLite repair harnesses and retain accepted execution evidence.
2. Run their source guards plus the existing provenance/rebuild/activation guards.
3. Retain byte-for-byte successful rebuild reproduction evidence alongside the new failure matrix.
4. Observe a committed successful activation through the real Pages cache invalidation handler and prove route/page/artifact generations rotate only after commit.
5. Retain browser/tenant rollout evidence and keep automatic audit-to-repair absent before any FFA/FBA promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-failures.mjs
cargo test -p rustok-pages --test explicit_artifact_repair_failures_sqlite -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, SQLite/PostgreSQL scenarios, lifecycle/cache observation, GraphQL/HTTP requests, workflows and CI were intentionally not run.
