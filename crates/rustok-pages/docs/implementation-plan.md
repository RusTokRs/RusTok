# Implementation Plan for `rustok-pages`

Date: 2026-08-06  
Status: `in_progress / authenticated-authoring-route-source-ready / inline-edit-asset-delivery-source-ready / release-integration-admin-launch-browser-pending`

## Policy: current code only

Pages is under active development. It keeps **no legacy** compatibility editor, component mirror, block table, shadow document authority or migration shim.

Forbidden:

- a JSON/CRUD editor beside Fly;
- the deleted Next/GrapesJS page-builder route;
- `frames[0].component` as a component-tree mirror;
- `PageBlock`, `BlockService`, `page_blocks` or block mutations;
- storefront block fallback rendering;
- UI access to raw transport adapters;
- host-owned Pages persistence, route-claim policy, cache-key policy or document policy;
- direct DOM-to-database persistence;
- fallback signing secrets or unsigned inline-edit claims.

Fly remains the only visual document and command authority. Pages owns page identity, localization, document revisions, lifecycle, route history, immutable published bindings, caches, authenticated inline grants/save transport and the module-owned inline asset HTTP contract. The host owns authenticated route admission, CSP and response composition, but not Pages document policy.

## Current source state

### Metadata and document ownership

- Registered Pages metadata uses the shared consumer-property contribution.
- The bespoke metadata editor and direct workspace metadata write remain removed.
- `PageService::save_document` remains the only document-only persistence owner.
- Published Fly documents remain immutable without an explicit draft lifecycle.
- Optimistic body revisions, row locks, transaction ordering and `NodeUpdated` remain owned by Pages.

### Reviewed publication and rollback

- Reviewed Page Builder runtime materialization remains the required builder publish path.
- Immutable published artifacts and locale bindings remain public render authority.
- Rollback selects a prior immutable publish manifest without compiling the current draft.
- The selected immutable published artifact regression covers persisted draft body mutation; current body content is not public render authority.

### Public locale, routing and SEO

- Public detail and list use tenant locale fallback with requested locale → tenant default → platform fallback.
- Published slug changes create immutable redirects for published slug renames.
- Old published slug claims cannot be reused.
- Localized canonical routes, hreflang alternates and host canonical/redirect/gone responses remain Pages-owned.
- Historical compatibility marker: `Delete tombstones and historical backfill remain open` described the earlier route-alias slice; delete tombstones and explicit history import are now source-ready.
- The historical route import owner uses bounded batches and immutable provenance receipts.
- Automatic inference remains open by design and deliberately unsupported.

### Cache and event delivery

- The native storefront cache, routed-channel admission and selected immutable artifact authority remain source-ready.
- Navigation-owned menus and SEO-owned context are composed into a deterministic private revalidation ETag with exact rendered HTML.
- Memory and OutboxLocal factory profile parity remains source-ready.
- The production generation gate, native route refill and PostgreSQL restart retry packets remain source-ready.

### Anonymous storefront boundary

- The anonymous storefront dependency graph verifier retains six feature-resolved profiles.
- Compiled SSR/CSR/hydrate bundle artifact evidence remains open.
- The public Pages route remains SSR-only and unchanged.
- Authenticated inline and asset profiles are opt-in and are not enabled by retained anonymous default/CSR/hydrate/SSR profiles.
- The authoring bootstrap is referenced only by the registered authenticated authoring page.

## Authenticated inline editing

### Reusable adapter: source-ready

`fly-leptos` and `rustok-page-builder-storefront` own the feature-gated real-DOM buffer and canonical Fly patch session.

- only noninteractive static leaf text outside runtime-owned subtrees is eligible;
- proof material is not written into DOM;
- focusout is the commit boundary;
- unchanged values do not consume a grant;
- changed values become one canonical `EditorCommand::Patch` after consumer authorization.

Historical adapter marker retained for verifier compatibility: `Pages consumer grant issuance and document-only save mount remain open` was correct for PR #3039 and is superseded by the current consumer source.

### Pages authenticated inline grant issuer: source-ready

`PageInlineEditKeyring` owns versioned HMAC-SHA256 grants. Claims bind tenant, user, direct authenticated session, a separate fresh edit-session UUID, channel, Pages page, stable Fly page, locale, body revision, project hash and expiry.

- default TTL: 60 seconds;
- maximum TTL: five minutes;
- maximum keyring size: eight;
- secret/proof/signature debug output is redacted;
- missing `RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY` leaves inline editing unavailable;
- invalid explicit key configuration fails module runtime registration;
- there is no fallback secret.

The tenant capability is `pages.builder.inline_edit.enabled` and defaults to disabled.

### Document-only save transport: source-ready

The feature-gated bootstrap and commit server functions:

1. require a direct authenticated user session;
2. verify tenant/user request identity;
3. require the typed host keyring and transactional event bus;
4. enforce the tenant inline feature and `pages:update` ownership;
5. load one unpublished exact-locale GrapesJS body;
6. require stable Fly page/component ids before hashing;
7. verify signed claims on receipt and again immediately before mutation;
8. reuse `AuthenticatedInlineEditSession` for the sole Fly patch;
9. recheck tenant capability;
10. call only `PageService::save_document(expected_revision)`;
11. issue a fresh replacement grant from the committed revision.

Stale revision, cross-session, cross-channel, cross-page, cross-locale, tampered, expired and replayed requests fail closed. The existing optimistic document revision remains the final persistence fence.

### Authenticated authoring route and shell: source-ready

The existing storefront module route owner registers the opt-in `pages-authoring` page only under `rustok-storefront/pages-inline-edit`:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<locale>
/{locale}/modules/pages-authoring?page_id=<pages UUID>
```

The host performs coarse admission before HTML or inline server-function execution:

- direct user principal only;
- non-nil authenticated session;
- effective `pages:update`;
- Pages module enablement through the existing module-page registry.

Exact page ownership and document eligibility remain in the Pages owner. The route reuses `PagesAuthenticatedInlineEditSurface`; it does not introduce a second persistence path.

All authoring HTML and bootstrap/commit responses use `private, no-store`. HTML also uses `X-Robots-Tag: noindex, nofollow, noarchive`. The existing global nonce-backed UI CSP remains authoritative. The route emits only a same-origin external bootstrap module and writes no proof into DOM.

Historical consumer marker retained for verifier compatibility: `authenticated route mount remains open` was correct for PR #3049 and is superseded by the authenticated authoring route source above.

### Client artifact export: source-ready

Non-default source profiles:

```text
rustok-pages-storefront/inline-edit
rustok-storefront/pages-inline-edit
rustok-storefront/pages-inline-edit-hydrate
rustok-server/pages-inline-edit
rustok-pages/inline-edit-assets
rustok-server/pages-inline-edit-assets
```

The storefront crate exposes a feature/target-gated `start_pages_inline_edit_client` WASM export. The authoring bootstrap imports only fixed same-origin paths after it finds the authoring root. Before mounting the client surface, the export removes the SSR shell to avoid duplicate editor instances.

Fixed paths:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

### Inline edit asset delivery: source-ready

The Pages module HTTP owner now conditionally merges an exact asset router under `rustok-pages/inline-edit-assets`. All three generated files are embedded into the server binary. Missing files fail asset-profile compilation, so an incomplete authoring deployment cannot silently fall back to a runtime filesystem.

The fixed stable paths use explicit JavaScript/WASM MIME types, `Cache-Control: public, max-age=0, must-revalidate`, full SHA-256 ETags, exact/weak `If-None-Match` handling and `Cross-Origin-Resource-Policy: same-origin`.

`apps/storefront/scripts/build-pages-inline-edit-client.mjs` resolves the exact `wasm-bindgen` version from `Cargo.lock`, requires a matching CLI, uses Cargo `--locked`, respects `CARGO_TARGET_DIR`, validates generated files and atomically publishes the JS/WASM pair.

`scripts/build/build-pages-inline-edit-server.sh` installs or verifies the exact CLI, builds all three assets first and then compiles `rustok-server --features pages-inline-edit-assets`. Binary-only packaging remains compatible because no runtime asset directory is required.

The source boundary is complete, but release workflow integration remains pending. Production Docker builder integration remains pending. The admin-owned launch link remains pending. No generated artifact, embedded binary, HTTP response or browser result is claimed.

Source:

- `src/services/page/inline_edit.rs`;
- `src/services/page/inline_edit_feature.rs`;
- `src/services/page/inline_edit_runtime.rs`;
- `src/http.rs`;
- `src/http/inline_edit_assets.rs`;
- `storefront/src/inline_edit.rs`;
- `apps/storefront/src/modules/core.rs`;
- `apps/storefront/public/assets/pages-inline-edit-bootstrap.js`;
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`;
- `apps/server/src/middleware/auth_context.rs`;
- `scripts/build/build-pages-inline-edit-server.sh`;
- `contracts/evidence/pages-authenticated-inline-consumer-source.json`;
- `contracts/evidence/pages-authenticated-authoring-route-source.json`;
- `contracts/evidence/pages-inline-edit-asset-delivery-source.json`;
- `scripts/verify/verify-pages-authenticated-inline-consumer.mjs`;
- `scripts/verify/verify-pages-authenticated-authoring-route.mjs`;
- `scripts/verify/verify-pages-inline-edit-asset-delivery.mjs`.

## Retained source markers

These phrases remain for static guard compatibility:

- public list tenant locale fallback;
- immutable redirects for published slug renames;
- Old published slug claims cannot be reused;
- host route response adapter;
- selected immutable published artifact regression;
- persisted draft body mutation;
- current body content is not public render authority;
- historical route import owner;
- provenance receipts;
- Automatic inference remains open by design;
- anonymous storefront dependency graph verifier;
- Compiled SSR/CSR/hydrate bundle artifact evidence remains open;
- Memory and OutboxLocal factory profile parity;
- production generation gate;
- PostgreSQL restart retry;
- Navigation-owned menus and SEO-owned context;
- deterministic private revalidation ETag;
- authenticated inline grant issuer;
- document-only save transport;
- authenticated route mount remains open;
- authenticated inline profiles are opt-in;
- authenticated authoring route and shell: source-ready;
- client artifact build and browser execution remain pending;
- inline edit asset delivery: source-ready;
- release workflow integration remains pending;
- admin-owned launch link remains pending.

## Next implementation order

### P0 — deployment composition

- [ ] Integrate `build-pages-inline-edit-server.sh` into both deterministic release build and reproducibility jobs.
- [ ] Integrate the same exact-lock build path into the production Docker builder.
- [ ] Update protected release-infrastructure guards and approvals without weakening pinned actions or reproducibility.
- [ ] Add an admin-owned launch link under a matching opt-in admin feature.

### P1 — maintainer execution evidence

- [ ] Run the asset-delivery, authenticated-route and consumer static guards.
- [ ] Run focused Pages/router/auth tests and server Cargo checks.
- [ ] Run the exact client/server orchestrator and retain generated file hashes/sizes plus binary digest.
- [ ] Prove same-origin `200` and `304` delivery of all three fixed assets with exact MIME, ETag and CORP headers.
- [ ] Prove anonymous/public Pages HTML does not reference or fetch the authoring bootstrap.
- [ ] Execute direct-user allowed and anonymous/service/delegated/permission-denied HTTP cases.
- [ ] Observe browser edit, save, replacement grant, stale revision, replay and expiry behavior.
- [ ] Re-run anonymous dependency graph and built artifact inspection.

### P2 — deployment hardening

- [ ] Decide whether later deployment should fingerprint the generated JS/WASM while retaining a stable bootstrap contract.
- [ ] Record CSP violation reports for the authoring route under production policy.
- [ ] Complete observed tenant rollout evidence before promotion.

## Execution status

No tests, static verifiers, formatting, Cargo checks, SQLite/PostgreSQL scenarios, SSR/WASM builds, client/server artifact builders, HTTP hosts, asset delivery, browsers, dependency graphs, workflows or CI were executed by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs
node apps/storefront/scripts/build-pages-inline-edit-client.mjs --print-wasm-bindgen-version
bash scripts/build/build-pages-inline-edit-server.sh
cargo test -p rustok-pages --features inline-edit-assets --all-targets -- --nocapture
cargo test -p rustok-pages-storefront --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-storefront --no-default-features \
  --features pages-inline-edit-hydrate --target wasm32-unknown-unknown
cargo check -p rustok-server --features pages-inline-edit-assets
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-ssr-delivery.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
```

Execution evidence remains pending.
