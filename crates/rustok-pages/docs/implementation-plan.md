# Implementation Plan for `rustok-pages`

Date: 2026-08-06  
Status: `in_progress / authenticated-inline-consumer-source-ready / route-mount-open / execution-pending`

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

Fly remains the only visual document and command authority. Pages owns page identity, localization, document revisions, lifecycle, route history, immutable published bindings, caches and authenticated inline grants/save transport.

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
- Pages/Navigation/SEO composition binds exact owner payloads and rendered HTML into a private revalidation ETag.
- Memory and OutboxLocal event delivery profile parity remains source-ready.
- Production generation invalidation, native route refill and PostgreSQL retry packets remain source-ready.

### Anonymous storefront boundary

- The anonymous storefront dependency graph verifier retains six feature-resolved profiles.
- Compiled SSR/CSR/hydrate bundle artifact evidence remains open.
- The current public Pages host remains SSR-only.
- Authenticated inline profiles are opt-in and are not enabled by the retained anonymous default/CSR/hydrate/SSR profiles.

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

Source:

- `src/services/page/inline_edit.rs`;
- `src/services/page/inline_edit_feature.rs`;
- `src/services/page/inline_edit_runtime.rs`;
- `storefront/src/inline_edit.rs`;
- `contracts/evidence/pages-authenticated-inline-consumer-source.json`;
- `scripts/verify/verify-pages-authenticated-inline-consumer.mjs`.

### Feature profiles

Non-default source profiles:

```text
rustok-pages-storefront/inline-edit
rustok-storefront/pages-inline-edit
rustok-storefront/pages-inline-edit-hydrate
rustok-server/pages-inline-edit
```

The authenticated inline profiles are opt-in. No current anonymous profile enables them.

### Remaining open boundary

The authenticated route mount remains open. No anonymous route mounts `PagesAuthenticatedInlineEditSurface`, and no current public SSR document emits the authenticated hydrate artifact.

Next source work must:

1. choose an authenticated authoring route/shell owner;
2. render `PagesAuthenticatedInlineEditSurface` only after authenticated route admission;
3. deliver `pages-inline-edit-hydrate` only on that surface;
4. define CSP/nonce and navigation behavior;
5. retain the anonymous SSR-only boundary.

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
- authenticated inline profiles are opt-in.

## Execution status

No tests, static verifiers, formatting, Cargo checks, SQLite/PostgreSQL scenarios, SSR/WASM builds, browsers, dependency graphs, workflows or CI were executed by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs
cargo test -p rustok-pages --all-targets -- --nocapture
cargo test -p rustok-pages-storefront --features inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-storefront --no-default-features \
  --features pages-inline-edit-hydrate --target wasm32-unknown-unknown
cargo check -p rustok-server --features pages-inline-edit
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
node crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs
```

Execution evidence remains pending.
