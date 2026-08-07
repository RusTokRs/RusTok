# Page Builder / Pages Parity Actualization

Date: 2026-08-07
Status: current-source-overlay / rebuild-provenance-source-ready / explicit-artifact-rebuild-source-ready / explicit-artifact-binding-replacement-source-ready / explicit-artifact-repair-transport-source-ready / repair-request-contract-harness-source-ready / execution-and-rollout-open

This overlay reconciles the Page Builder programme with current `main`. It supersedes stale open-checkbox wording in older broad plans where that wording conflicts with merged source. It does not convert source-ready work into executed evidence.

## Corrected source state

### Consumer properties

The typed metadata contribution is source-complete for the Pages reference consumer.

- `rustok.pages.metadata` has one registered six-field schema.
- Draft Pages workspaces mount the canonical `ConsumerPropertiesPanel` inside Fly.
- Published pages mount the same registered panel in a Pages-owned standalone surface.
- Published Fly authoring remains unmounted.
- `PageMetadataEditor` and its direct metadata transport write are removed.
- Metadata persistence remains Pages-owned and independently versioned from the Fly document.

Any older Phase 5 checkbox saying that consumer metadata still needs to move into typed property contributions is stale at source level. Executed conflict, dirty-Fly isolation and browser evidence remain open.

### Immutable rollback

Immutable rollback is source-complete.

- Pages has a separate idempotent rollback command and receipt.
- Rollback selects a prior exact publish manifest and verifies immutable artifacts.
- It replaces locale bindings, advances the page version and writes `NodeUpdated` plus `NodePublished` in the owner transaction.
- It never invokes current-document sanitization, runtime materialization or compilation.
- GraphQL, HTTP, OpenAPI and the typed Pages admin prepare/confirm control are connected.

Any older Phase 6 checkbox saying rollback still needs implementation is stale at source level. Database execution and accepted rollback evidence remain open.

### Cache and public readers

The event-driven cache boundary is source-connected.

- Pages owns route/page/artifact scopes, namespace generations and key shape.
- Publish, rollback and explicit repaired-binding activation emit durable lifecycle events instead of calling cache infrastructure inline.
- The handler validates event/correlation-bound generation receipts.
- Storefront and artifact readers authorize before lookup, use current generations, load verified owner data before fill and fail open on cache errors.

Recent source packets add progressively stronger evidence:

- PR #2955: event/correlation and generation miss/refill contract;
- PR #2971: PostgreSQL publish/rollback outbox/cache harness;
- PR #2974: durable relay failure and restart harness;
- PR #2979: public artifact HTTP cache harness;
- native storefront cache source contract.

The native storefront cache source contract is ready. It retains composite route/page/artifact generation keys, hit short-circuit, generation rotation, old-value reachability rules and cache-failure fallback through the same public Pages cache runtime.

### Authenticated inline authoring and anonymous exclusion

Authenticated real-DOM authoring, dedicated authoring assets, same-origin admin launch, deterministic release composition and the artifact/HTTP/browser/rollout evidence harness chain are source-ready.

Anonymous default, CSR, hydrate and SSR profiles do not enable inline editing, authoring assets or the admin launch. Public Pages HTML remains SSR-only and excludes the dedicated authoring bootstrap, JS and WASM source paths.

Older Phase 3 and Phase 8 checkboxes that still say the real-DOM adapter, authenticated storefront editing, selected immutable artifact rendering or anonymous authoring-code exclusion are unimplemented are stale at source level. Artifact, HTTP, browser and tenant execution remain open.

### Reviewed publish resource limits

Marker:

```text
static-publish-resource-limits-source-ready
```

The reviewed static publish path already has provider-owned fail-closed HTML, CSS, URL, attribute, metadata and public-resource checks. The remaining global budget gap is now source-closed.

The provider now rejects a prepared project above any of these limits:

- 16 MiB serialized project bytes;
- 128 pages;
- 50,000 current `pages[].component` nodes;
- component depth 128;
- 4,096 assets;
- 20,000 style rules.

The resource policy has deterministic SHA-256 identity and typed observations, but the established sanitization identity remains exactly `page_builder_static_publish_sanitization_v2`. Its existing hash payload and the Pages locale-ordered `sanitized_set_hash` contract are unchanged.

Resource validation runs before sanitized-project hashing and is repeated during transient sanitization integrity verification before runtime materialization. Existing per-content, attribute, URL, CSS and media-query limits remain authoritative.

No persisted DTO, migration or historical immutable artifact rewrite is introduced by this slice.

The broad Phase 4 wording should now be read as follows:

- HTML/CSS/URL/attribute policy source: ready;
- global project/page/component/depth/asset/style budgets: source-ready;
- accepted parser, real-project, runtime and tenant evidence: pending.

### Immutable artifact integrity audit

Markers:

```text
immutable-artifact-integrity-audit-source-ready
immutable-artifact-integrity-audit-transport-source-ready
```

Pages owns a bounded read-only immutable artifact integrity audit for one exact tenant and page.

The owner command requires `pages:manage` with `PermissionScope::All`. Under the current access-token permission bridge effective Pages Manage resolves to `All` when present and `None` when absent; there is no current Pages Manage `Own` request state. The explicit owner `All` check remains defense in depth for direct/internal callers and future authorization-model changes. The audit reads through one transaction and scans at most 512 records ordered by creation time and artifact id. It requests one extra row and sets `truncated=true` instead of claiming a complete audit when more retained records exist.

Each record is reconstructed through the Page Builder static artifact and materialization contracts. The audit checks owner identity, canonical or operation-bound storage instance identity, static artifact hashes, build/renderer metadata, output byte limits, complete current materialization evidence and exact legacy all-`NULL` compatibility. Partial evidence fails closed.

The result contains only artifact id, fixed SHA-256 locale and record-identity hashes, one public finding code, hashed internal diagnostics, counts, truncation flags and a deterministic audit hash. The internal record identity also binds `instance_key`. It does not return raw locale, storage instance key, stored build/artifact/content/materialization hashes, HTML, CSS, runtime snapshots, materialization identity JSON or internal error text.

The GraphQL and HTTP audit transport is source-ready:

- GraphQL mutation `auditPageArtifacts` is mounted into `PagesMutation`;
- HTTP registers `POST /api/admin/pages/{id}/artifacts/audit`;
- OpenAPI includes the route and bounded owner DTOs;
- both adapters require effective `pages:manage`, fence the current tenant and delegate to the owner service;
- the owner service still performs the authoritative `PermissionScope::All` check;
- adapters return static public error codes and never copy internal `PagesError` text;
- neither adapter queries artifact tables, writes records or emits events.

The audit remains read-only and does not invoke rebuild or binding replacement automatically. No admin repair UI is added.

The broad Phase 6 wording should now be read as follows:

- bounded read-only integrity-audit command: source-ready;
- GraphQL, HTTP and OpenAPI audit transport: source-ready;
- explicit append-only rebuild service command: source-ready;
- explicit binding replacement service command: source-ready;
- bounded tenant-admin repair transports: source-ready;
- generated transport and request-contract harnesses: source-ready;
- accepted database and transport evidence: pending.

### Reviewed publish rebuild provenance

Marker:

```text
publish-rebuild-provenance-source-ready
```

New reviewed publish receipts retain one locale-specific immutable source row in `page_publish_rebuild_sources` in the same owner transaction as the publish operation and immutable artifact manifest.

The source row records the exact selected page-body identity, format and revision, the canonical sanitized Page Builder project and sanitized hash, the reviewed runtime hash, artifact source/artifact/materialization hashes, materialization identity, runtime snapshots and a deterministic provenance hash.

The existing publish-receipt hook re-reads the exact locale binding and body, re-sanitizes through the canonical Page Builder policy, verifies the sanitization envelope, requires complete reviewed materialization evidence and recomputes both locale-ordered `sanitized_set_hash` and `artifact_set_hash`. Any mismatch aborts the surrounding publication transaction.

Existing publish operations are not backfilled. Existing artifact rows and bindings are not changed. The provenance row deliberately survives loss of its referenced artifact row and remains usable as investigation input. The complete runtime context is not duplicated; rebuild requires an explicitly reviewed context and proves its review, scenario and context hashes against retained evidence.

### Explicit immutable artifact rebuild

Marker:

```text
explicit-artifact-rebuild-source-ready
```

Pages has an explicit tenant-admin service command for one exact retained provenance row.

The request binds tenant, page, source id, expected provenance hash, idempotency key and a fresh `ReviewedPagePublishRuntimeInput`. Tenant-wide `pages:manage` is required. Under the current request model Manage present resolves to `PermissionScope::All` and Manage absent to `None`; the owner service still checks `All` before input validation or writes. The command never reads the mutable current draft.

Before compilation it recomputes the provenance hash, re-sanitizes the retained sanitized source, verifies the sanitizer envelope and requires exact review hash, runtime scenario and runtime-context hash parity. The rebuilt source, static artifact, materialization hash, materialization identity and runtime snapshots must reproduce the retained reviewed publication exactly.

`page_static_landing_artifacts` separates deterministic content identity from storage instance identity:

```text
canonical
rebuild:<rebuild-operation-uuid>
```

Ordinary reviewed publish remains on `canonical`. Rebuild inserts a new operation-bound immutable row and a replayable `page_artifact_rebuild_operations` receipt. The source or damaged artifact is never updated or deleted.

The command does not change the published binding, page version, lifecycle state, route/page/artifact generations or event stream. It remains a distinct owner command and is not automatic repair.

### Explicit artifact binding replacement

Marker:

```text
explicit-artifact-binding-replacement-source-ready
```

Pages owns the separate explicit activation command for one exact rebuild receipt.

The request binds tenant, page, rebuild operation id, expected page version, expected current artifact id and a distinct idempotency key. Tenant-wide `pages:manage` is required; current request permissions again resolve Pages Manage to `All` when present and `None` when absent. The page must remain published.

The selected rebuild receipt and retained provenance must be valid and mutually consistent. Its source artifact must equal the caller's expected current artifact, and the locked locale binding must still point to that exact source id and retained body. The replacement row must match the receipt's tenant, page, locale, operation-bound instance key, artifact hash and materialization hash. The existing Page Builder artifact verifier runs before the binding update.

The owner transaction updates one locale binding only, advances the page version once, retains published state, writes `NodeUpdated` plus `NodePublished`, stores `page_artifact_binding_replacement_operations`, and then commits. Cache generation effects remain event-driven after commit; the command never calls cache infrastructure inline.

An exact replay returns the retained result without repeating the binding, version or events. A rebuild receipt may receive only one activation receipt. The damaged source artifact remains unchanged and retained.

Audit does not schedule rebuild or activation automatically.

### Bounded tenant-admin repair transports

Markers:

```text
explicit-artifact-repair-transport-source-ready
explicit-artifact-repair-pages-manage-all-none-actualized
explicit-artifact-repair-request-contract-harness-source-ready
```

Pages exposes the two existing repair owner commands through separate bounded adapters.

GraphQL mounts:

```text
rebuildPageArtifact
activateRebuiltPageArtifact
```

HTTP mounts:

```text
POST /api/admin/pages/{id}/artifacts/rebuild
POST /api/admin/pages/{id}/artifacts/activate
```

OpenAPI registers both routes, their explicit owner inputs and bounded public result schemas.

GraphQL performs the tenant module-enabled check. Both transport families fence the authenticated actor to the current tenant and require an effective `pages:manage` grant before owner delegation. The owner commands independently enforce `PermissionScope::All` before writes.

Adapters delegate once to `PageService`. They do not import entities, query artifact/provenance/binding tables, mutate owner records directly, emit lifecycle events or call cache infrastructure. Error mappers use only static public codes and static messages; raw `PagesError` text is never returned.

The bounded rebuild result exposes operation/page/locale identities, source and rebuilt artifact ids, verified artifact/materialization hashes, replay state and timestamp. The bounded activation result exposes operation/page/version/locale identities, rebuild operation id, previous/replacement artifact ids, verified replacement hashes, replay state and timestamp.

Both result shapes omit provenance source id, source publish operation id, storage instance key, idempotency keys, runtime context, materialization identity JSON and runtime snapshots. No discovery/list endpoint or combined repair endpoint is introduced.

A generated contract harness builds the real Pages GraphQL schema and serializes the real Pages OpenAPI document. A separate request-level harness is source-ready to dispatch real GraphQL and Axum requests for both rebuild and activation, covering current-tenant mismatch, missing Manage, Manage-present owner validation and the static public error bodies. Neither harness is counted as execution evidence until the maintainer runs it.

Current repair/rebuild matrix:

| Capability | Source state | Execution state |
| --- | --- | --- |
| Immutable rebuild provenance | Source-ready | Migration/publish evidence pending |
| Explicit append-only repair/rebuild command | Source-ready | SQLite/PostgreSQL and authorization evidence pending |
| Rebuild idempotent receipt | Source-ready | Replay/conflict evidence pending |
| Canonical/rebuild storage instance identity | Source-ready | Migration and duplicate-identity evidence pending |
| Explicit binding replacement | Source-ready | SQLite/PostgreSQL, fences, lifecycle and cache evidence pending |
| Bounded tenant-admin repair transports | Source-ready | GraphQL/HTTP/OpenAPI execution pending |
| Generated GraphQL/OpenAPI transport contract harness | Source-ready | Maintainer execution pending |
| Request-level tenant/Manage/static-error harness | Source-ready | Maintainer execution pending |
| Pages Manage `All`/`None` semantics | Source actualized | Maintainer execution pending |
| Static public repair errors and bounded results | Source-ready | Response-shape execution pending |
| Automatic audit-to-rebuild action | Deliberately absent | Not allowed |

### Status boundary

Source parity has advanced, but execution and rollout remain open.

- No new test, source verifier, Cargo, formatting, migration, database, GraphQL, HTTP, OpenAPI, browser, workflow or CI execution is claimed here.
- No audit, provenance, rebuild, binding replacement, repair transport, request-contract, lifecycle/cache observation or tenant rollout scenario was executed.
- No FFA/FBA promotion is made.

## Current next cursor

1. Run the generated repair transport and request-contract harnesses plus the immutable artifact audit, provenance, explicit-rebuild, binding-replacement and repair transport/request source guards.
2. Retain SQLite/PostgreSQL audit evidence for valid canonical/rebuilt records, corruption, partial evidence, Manage present/absent authorization and truncation.
3. Retain provenance migration/publish evidence for exact locale capture, aggregate-hash mismatch rollback, artifact-row loss and legacy no-backfill behavior.
4. Retain explicit rebuild evidence for Manage present/absent with `All`/`None` semantics, exact replay, idempotency conflict, provenance corruption, runtime mismatch and byte-for-byte reproduction.
5. Prove rebuild appends a distinct artifact row while the active binding, page version, lifecycle events and cache generations remain unchanged.
6. Retain explicit binding replacement evidence for stale page version, stale current artifact, invalid replacement, one-locale mutation, exact replay, one activation per rebuild and unchanged source row.
7. Observe committed `NodeUpdated`/`NodePublished` processing and route/page/artifact generation changes only after activation commit.
8. Retain repair transport execution evidence for generated GraphQL/OpenAPI contracts, current-tenant fences, Manage absent/present behavior, static errors and bounded result fields.
9. Run the static publish resource-limit source guard and accepted real-project policy evidence.
10. Execute existing metadata conflict/isolation, cache continuity, artifact/HTTP/browser and tenant Wave packets before promotion.
