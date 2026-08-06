# Page Builder / Pages Parity Actualization

Date: 2026-08-06
Status: current-source-overlay / rebuild-provenance-source-ready / execution-and-rollout-open

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
- Publish and rollback emit durable lifecycle events instead of calling cache infrastructure inline.
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

Pages now owns a bounded read-only immutable artifact integrity audit for one exact tenant and page.

The command accepts only tenant-wide `pages:manage` (`PermissionScope::All`); owner-scoped Manage is rejected. It reads through one transaction and scans at most 512 records ordered by creation time and artifact id. It requests one extra row and sets `truncated=true` instead of claiming a complete audit when more retained records exist.

Each record is reconstructed through the Page Builder static artifact and materialization contracts. The audit checks owner identity, static artifact hashes, build/renderer metadata, output byte limits, complete current materialization evidence and exact legacy all-`NULL` compatibility. Partial evidence fails closed.

The result contains only artifact id, fixed SHA-256 locale and record-identity hashes, one public finding code, hashed internal diagnostics, counts, truncation flags and a deterministic audit hash. It does not return raw locale, stored build/artifact/content/materialization hashes, HTML, CSS, runtime snapshots, materialization identity JSON or internal error text.

The GraphQL and HTTP audit transport is now source-ready:

- GraphQL mutation `auditPageArtifacts` is mounted into `PagesMutation`;
- HTTP registers `POST /api/admin/pages/{id}/artifacts/audit`;
- OpenAPI includes the route and bounded owner DTOs;
- both adapters require effective `pages:manage`, fence the current tenant and delegate to the owner service;
- the owner service still performs the authoritative `PermissionScope::All` check;
- adapters return static public error codes and never copy internal `PagesError` text;
- neither adapter queries artifact tables, writes records or emits events.

This command and its transports do not publish, rollback, repair or rebuild. No admin UI is added. Automatic repair/rebuild remains open as a separate source cursor.

The broad Phase 6 wording should now be read as follows:

- bounded read-only integrity-audit command: source-ready;
- GraphQL, HTTP and OpenAPI transport: source-ready;
- accepted database and transport evidence: pending;
- repair/rebuild remains open.

Any local Pages heading claiming that all remaining work is execution evidence only is still stale because repair/rebuild remains an open source task.

### Reviewed publish rebuild provenance

Marker:

```text
publish-rebuild-provenance-source-ready
```

New reviewed publish receipts now retain one locale-specific immutable source row in `page_publish_rebuild_sources` in the same owner transaction as the publish operation and immutable artifact manifest.

The source row records the exact selected page-body identity, format and revision, the canonical sanitized Page Builder project and sanitized hash, the reviewed runtime hash, artifact source/artifact/materialization hashes, materialization identity, runtime snapshots and a deterministic provenance hash.

The existing publish-receipt hook re-reads the exact locale binding and body, re-sanitizes through the canonical Page Builder policy, verifies the sanitization envelope, requires complete reviewed materialization evidence and recomputes both locale-ordered `sanitized_set_hash` and `artifact_set_hash`. Any mismatch aborts the surrounding publication transaction.

Existing publish operations are not backfilled. Existing artifact rows and bindings are not changed. The provenance row deliberately survives loss of its referenced artifact row and therefore remains usable as investigation input. The complete runtime context is not duplicated; a future command must obtain an explicitly reviewed context and prove its review/context hashes against retained evidence.

This closes the immutable source provenance prerequisite only. The repair/rebuild command remains open, as do authorization, idempotency, append-only replacement, explicit binding switch, lifecycle/cache effects and transports. No automatic repair is introduced.

### Status boundary

Source parity has advanced, but execution and rollout remain open.

- No new test, verifier, Cargo, formatting, migration, database, GraphQL, HTTP, browser, workflow or CI execution is claimed here.
- No audit database or transport scenario, provenance migration/publish scenario, publish/materialization scenario or repair was executed.
- No FFA/FBA promotion is made.

## Current next cursor

1. Run the immutable artifact audit command and transport source guards plus focused Pages tests.
2. Retain SQLite/PostgreSQL audit evidence for valid legacy/current records, corruption, partial evidence, tenant-wide versus owner-scoped authorization and 513-row truncation.
3. Retain GraphQL/HTTP/OpenAPI evidence for current-tenant fencing, static public errors and bounded result parity.
4. Run the reviewed publish rebuild-provenance source guard and retain SQLite/PostgreSQL evidence for exact locale capture, aggregate-hash mismatch rollback, artifact-row loss and legacy no-backfill behavior.
5. Design an explicit tenant-wide repair/rebuild command that selects one provenance row, reauthorizes the exact runtime context, appends a new immutable artifact and never updates the damaged artifact in place.
6. Keep any binding switch separately authorized and idempotent, with lifecycle/cache effects only after the explicit switch.
7. Run the static publish resource-limit source guard and retain accepted real-project policy evidence.
8. Execute the existing metadata conflict/isolation, cache continuity, artifact/HTTP/browser and tenant Wave packets before promotion.
