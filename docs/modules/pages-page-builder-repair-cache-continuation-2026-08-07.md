# Pages / Page Builder Explicit Repair Cache Continuation

Date: 2026-08-07  
Status: source-ready / explicit-artifact-repair-cache-harness-source-ready / execution-pending  
Scope: exact rebuild byte reproduction and committed activation lifecycle delivery through the Pages cache invalidation owner

## Rechecked state

Current `main` already contains:

- retained reviewed publish provenance;
- explicit append-only immutable artifact rebuild;
- separate rebuilt-artifact activation;
- bounded GraphQL/HTTP/OpenAPI repair transports;
- generated transport and request-contract harnesses;
- PostgreSQL repair receipt/transaction atomicity harness;
- negative SQLite repair failure matrix.

The remaining repair-specific cache gap is not whether the cache owner exists. Earlier publish/rollback packets already exercise `PageCacheInvalidationEventHandler`. The open repair-specific question is whether a successful rebuilt-artifact activation stays cache-neutral until its committed lifecycle envelopes are delivered, and whether those exact envelopes produce the expected generation deltas.

## Source marker

```text
explicit-artifact-repair-cache-harness-source-ready
```

New harness:

```text
crates/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs
```

Machine evidence:

```text
crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-cache-source.json
```

Fail-closed source guard:

```text
crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-cache.mjs
```

The harness is environment-gated by `RUSTOK_PAGES_TEST_DATABASE_URL` with `DATABASE_URL` fallback and accepts PostgreSQL URLs only. It creates an isolated schema, applies the real `OutboxModule` and `PagesModule` migrations, seeds only the Pages module enablement fixture, and drops the schema after the scenario.

## Reviewed publish revision contract

The repair-cache fixture follows the current production `reviewed_publish::body_revision_snapshot` contract exactly:

```text
body.updated_at.to_string()
```

The harness therefore sends the matching created-body DTO `updated_at` value directly. It does not append a content digest. The source guard reads the production reviewed-publish owner and fails closed if the stale `updated_at:sha256(format\0content)` fixture construction returns.

This correction changes test scaffolding only; production reviewed-publish behavior is unchanged.

## Exact rebuild reproduction packet

The harness reviewed-publishes one GrapesJS page through `PageService`, then retains the complete canonical `page_static_landing_artifacts` model in memory before deliberately damaging the persisted canonical HTML/body/CSS payload.

The explicit rebuild then runs from the retained reviewed provenance. The rebuilt row must match the pre-damage canonical model exactly after normalizing only the three storage-instance fields that are expected to differ:

```text
id
instance_key
created_at
```

All deterministic artifact/materialization/build/renderer/registry/head/body/CSS/content/section fields therefore remain bound to the exact pre-damage publication snapshot.

The rebuild still must:

- append a distinct rebuilt artifact id;
- keep the active locale binding on the damaged source artifact;
- keep the page version unchanged;
- add no lifecycle outbox rows;
- leave cache generations unchanged because no cache handler is invoked.

## Activation after-commit cache boundary

Before activation, the harness creates a recording `PageCacheInvalidationPort` with non-zero route/page/artifact generations and wraps it in the real:

```text
PageCacheInvalidationEventHandler
PagesCacheInvalidationRuntime
```

The valid activation runs through `PageService::replace_rebuilt_artifact_binding` and returns only after the owner transaction commits.

Immediately after that return, before any durable envelope is delivered to the handler, the recording cache generations must still equal the initial snapshot. This retains the source boundary that repair activation does not invalidate cache inline.

The harness then loads only the two new committed `sys_events` rows and validates their registered envelope schema. It requires exactly one Pages `NodeUpdated` and one Pages `NodePublished` envelope for the activated page.

The handler is then invoked explicitly with those committed envelopes:

1. `NodeUpdated` must rotate route and page generations only;
2. `NodePublished` must rotate route, page and artifact generations.

For one activation lifecycle pair, the total generation delta is therefore:

```text
route    +2
page     +2
artifact +1
```

Both invalidation requests and receipts must preserve the source event id and correlation id. The `NodeUpdated` receipt must not claim an artifact generation; the `NodePublished` receipt must carry the final route/page/artifact generations.

## Preserved boundaries

This slice does not change production behavior.

- No service, entity, adapter or cache production source is changed.
- No migration or database schema is changed.
- No GraphQL/HTTP/OpenAPI surface is changed.
- Rebuild still emits no lifecycle events.
- Activation still emits durable lifecycle events and never calls cache infrastructure inline.
- No automatic audit-to-rebuild or rebuild-to-activation flow is introduced.
- No FFA/FBA promotion is made.

## Evidence state

Status remains:

```text
pages_explicit_artifact_repair_cache_source_unvalidated
```

`execution` is empty and every validation flag remains false. The source guard and PostgreSQL harness are intentionally not run in this slice.

## Updated repair matrix

| Capability | Source state | Execution state |
| --- | --- | --- |
| Reviewed publish revision fixture | Owner-aligned | Execution pending |
| Explicit rebuild owner | Source-ready | Runtime evidence pending |
| Rebuild provenance/runtime negative matrix | Harness-ready | SQLite execution pending |
| PostgreSQL rebuild/activation receipt atomicity | Harness-ready | PostgreSQL execution pending |
| Exact rebuilt model reproduction after source-artifact damage | Harness-ready | PostgreSQL execution pending |
| Rebuild leaves binding/version/events/cache generations unchanged | Harness-ready | PostgreSQL execution pending |
| Explicit rebuilt-artifact activation | Source-ready | Runtime evidence pending |
| Activation stale-version/invalid-target/unpublished matrix | Harness-ready | SQLite execution pending |
| Activation durable lifecycle pair | Harness-ready | PostgreSQL execution pending |
| Activation cache-neutral immediately after owner commit | Harness-ready | PostgreSQL execution pending |
| Committed `NodeUpdated` cache rotation | Harness-ready | PostgreSQL execution pending |
| Committed `NodePublished` cache rotation | Harness-ready | PostgreSQL execution pending |
| Total activation generation delta `+2/+2/+1` | Harness-ready | PostgreSQL execution pending |
| Automatic repair | Deliberately absent | Not allowed |
| FFA/FBA promotion | Open | Not promoted |

## Next cursor

1. Execute the repair transport contract, request contract, PostgreSQL atomicity, negative SQLite and repair-cache harnesses and retain accepted evidence.
2. Execute their source guards plus the existing provenance/rebuild/binding/audit guards.
3. Retain successful transport execution for both repair commands with bounded result bodies and current-tenant/Manage fencing.
4. Retain accepted PostgreSQL exact-reproduction and activation cache-generation observations from this packet.
5. Retain the broader immutable-artifact audit and provenance migration/publish execution evidence still open in the canonical parity plan.
6. Execute existing artifact/HTTP/browser and tenant Wave packets before any FFA/FBA promotion.
7. Keep automatic audit-to-rebuild and rebuild-to-activation chaining absent until execution evidence is accepted.

## Maintainer validation

Suggested commands, intentionally not run here:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-cache.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_cache_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test explicit_artifact_repair_postgres -- --nocapture
node crates/rustok-pages/scripts/verify/verify-pages-explicit-artifact-repair-failures.mjs
cargo test -p rustok-pages --test explicit_artifact_repair_failures_sqlite -- --nocapture
cargo check -p rustok-pages --all-targets
```

Tests, source verifiers, Cargo commands, formatting, PostgreSQL/SQLite scenarios, cache-handler execution, GraphQL/HTTP requests, workflows and CI were intentionally not run.
