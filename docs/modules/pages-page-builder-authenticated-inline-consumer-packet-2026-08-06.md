# Pages / Page Builder Authenticated Inline Consumer Packet

Date: 2026-08-06  
Status: source-ready / execution-pending  
Route mount: open

## Purpose

PR #3039 supplied the reusable Fly/Page Builder real-DOM adapter and canonical patch session. This slice adds the Pages-owned consumer boundary: signed short-lived grants, authenticated bootstrap/commit server functions, document-only persistence through the existing Pages owner, and an opt-in Leptos surface.

The source does not mount that surface into the anonymous Pages route or any authenticated application route. A route owner must still choose where the component is rendered and how its client artifact is delivered.

## Authorization and ownership

Every bootstrap and commit requires a direct authenticated user session. Delegated OAuth principals and service principals fail closed.

The Pages service then reuses its existing authority:

- tenant module `pages.builder.inline_edit.enabled` must be explicitly `true`; missing settings default to disabled;
- the actor must pass `pages:update` owner-aware authorization;
- published page documents remain immutable;
- the requested locale must own both a translation and an existing body;
- the body must be the current Fly/GrapesJS format;
- the current body revision remains the optimistic persistence fence.

The storefront transport never writes `page_bodies` directly. Its only persistence call is the existing `PageService::save_document` owner, which retains row locking, revision conflict detection, transaction ordering and `NodeUpdated` publication.

## Signed grant

`PageInlineEditKeyring` issues versioned HMAC-SHA256 proofs. Claims bind:

- tenant id;
- user id;
- direct authenticated session id;
- a separate fresh edit-session UUID;
- Pages page id;
- stable Fly page id;
- exact locale;
- current Pages body revision;
- exact Fly project hash;
- channel id and normalized channel slug when present;
- issue and expiry timestamps.

Authenticated-session identity and edit-session identity are deliberately separate. Multiple editable surfaces for one login do not share DOM root identity, and every replacement grant receives a fresh edit-session UUID.

The default TTL is 60 seconds and the maximum accepted TTL is five minutes. The keyring contract supports at most eight keyed secrets for bounded rotation. Secret material, signatures and authorization proofs are redacted from `Debug`.

Signature verification uses the shared HMAC-SHA256 helper and fixed-work 32-byte digest comparison. The proof is verified on receipt and again immediately before the canonical Fly mutation.

## Host composition

The Pages module reads only explicit host environment configuration:

```text
RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY
RUSTOK_PAGES_INLINE_EDIT_HMAC_KEY_ID
RUSTOK_PAGES_INLINE_EDIT_GRANT_TTL_MS
```

There is no development or production fallback secret. Missing key configuration leaves the typed keyring absent, so the server functions return a bounded unavailable response. Invalid explicit configuration fails module runtime registration.

The typed keyring is registered in `ModuleRuntimeExtensions` and transferred to `HostRuntimeContext`; the UI package never reads environment variables or secret values.

## Bootstrap boundary

The bootstrap server function:

1. extracts trusted `AuthContext`, `AuthPrincipalContext`, `TenantContext` and optional `RequestContext`;
2. requires one direct non-nil authenticated session;
3. verifies tenant/user request identity;
4. resolves the typed keyring and transactional event bus from `HostRuntimeContext`;
5. enforces the tenant inline-edit feature and Pages update ownership;
6. loads one exact mutable localized Fly body;
7. decodes it with the canonical Fly codec;
8. requires exactly one Fly page, a stable page id and stable ids on every component before hashing;
9. issues a short-lived grant with a fresh edit-session UUID.

Stable ids are required before hashing so `FlyEditor::new` cannot silently change the document and invalidate the grant.

## Commit boundary

The commit server function orders work as follows:

```text
authenticated request context
→ initial signed-grant verification
→ exact claim/request/session/channel match
→ current authorized localized document load
→ exact body revision and Fly hash recheck
→ signed-grant verification at current authorization time
→ canonical AuthenticatedInlineEditSession
→ consumer authorization port
→ one Fly EditorCommand::Patch
→ tenant capability recheck
→ PageService::save_document(expected_revision)
→ committed body revision
→ fresh replacement grant
```

A stale, replayed, expired, tampered, cross-session, cross-channel, cross-page, cross-locale or cross-revision request fails closed. The existing optimistic revision owner is the final replay fence even if two requests reach persistence concurrently.

The client receives stable Pages error codes plus user-safe messages. Internal error text and secret material are not serialized into the response.

## Opt-in feature graph

The source adds non-default profiles only:

```text
rustok-pages-storefront/inline-edit
rustok-storefront/pages-inline-edit
rustok-storefront/pages-inline-edit-hydrate
rustok-server/pages-inline-edit
```

The retained Pages `default`, `hydrate`, `ssr` profiles and host `csr`, `hydrate`, `ssr` profiles do not enable inline editing. The six anonymous dependency graphs must therefore continue to exclude `fly-leptos` and all authoring packages.

`pages-inline-edit-hydrate` is a separate client-build profile. It is not emitted by the current SSR-only public host.

## Source evidence

- `crates/rustok-pages/src/services/page/inline_edit.rs`;
- `crates/rustok-pages/src/services/page/inline_edit_feature.rs`;
- `crates/rustok-pages/src/services/page/inline_edit_runtime.rs`;
- `crates/rustok-pages/storefront/src/inline_edit.rs`;
- `crates/rustok-pages/contracts/evidence/pages-authenticated-inline-consumer-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs`;
- updated anonymous-storefront and adapter guards;
- opt-in storefront/server feature declarations.

## Deliberate limits

This slice does not:

- mount `PagesAuthenticatedInlineEditSurface` into an authenticated route;
- mount editing in the anonymous Pages storefront;
- add public executable scripts to the current SSR-only host;
- accept delegated or service principals;
- add a new database table or migration;
- add GraphQL or REST mutations;
- bypass `PageService::save_document`;
- edit published immutable documents;
- change publish, rollback, artifact, cache or event schemas;
- claim tests, verifiers, formatting, Cargo, WASM, browser, workflow, CI or rollout execution;
- promote FFA or FBA.

## Next cursor

1. Select and implement an authenticated authoring route owner that renders `PagesAuthenticatedInlineEditSurface` without altering the anonymous Pages route.
2. Deliver the separate `pages-inline-edit-hydrate` client artifact only on that authenticated surface.
3. Define CSP/nonce and navigation behavior for the authenticated editor shell.
4. Run the static verifier, Pages/storefront unit tests, SSR and WASM checks, anonymous graph verifier and a real browser save/reload/conflict scenario.
5. Record observed tenant rollout evidence before promotion.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

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
