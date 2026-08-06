# Pages / Page Builder Authenticated Authoring Route Packet

Date: 2026-08-06  
Status: source-ready / execution-pending

## Purpose

This slice mounts the Pages authenticated inline consumer from PR #3049 on a dedicated opt-in storefront authoring surface. It does not change the anonymous Pages route, public artifact selection, Pages document persistence, publication, rollback, route history, cache identity or event delivery.

The source route is:

```text
/modules/pages-authoring?page_id=<pages UUID>&lang=<locale>
/{locale}/modules/pages-authoring?page_id=<pages UUID>
```

The existing storefront module route and registry remain the host owner. The registered page keeps `module_slug = "pages"`, so normal tenant Pages-module admission still applies before the authoring component is selected.

## Admission boundary

The server auth middleware identifies only the two bounded authoring HTML forms and the two existing inline server-function endpoints.

Before route rendering or server-function execution it requires:

1. a resolved authenticated principal;
2. `SecurityActorKind::User`;
3. a direct authenticated user principal rather than delegated OAuth or service authority;
4. a non-nil authenticated session id;
5. effective `pages:update` permission.

This is a coarse host admission gate. Exact tenant, page ownership, locale, unpublished lifecycle, body format, revision, channel and signed-grant checks remain in the Pages-owned bootstrap/commit path. The host does not recreate those policies.

Anonymous, service and delegated principals fail closed before the authoring HTML or grant endpoints are reached.

## Response policy

Every response on the authoring HTML and bootstrap/commit endpoints receives:

```text
Cache-Control: private, no-store
```

The HTML authoring surface additionally receives:

```text
X-Robots-Tag: noindex, nofollow, noarchive
```

The existing outer security middleware remains the CSP owner. The HTML route therefore receives the normal per-response nonce-backed UI CSP. The server-function endpoints remain API surfaces under the existing deny-by-default API CSP.

The authoring document uses only an external same-origin module script. No authorization proof, signing key or grant payload is written into HTML or DOM attributes.

## Authoring shell

`apps/storefront/src/modules/core.rs` registers `pages-authoring` only when `rustok-storefront/pages-inline-edit` is enabled.

The source shell:

- reads `page_id` and locale from the existing `UiRouteContext`;
- rejects a missing or blank page id before mounting the consumer;
- provides a return link to `/admin/pages/{page_id}`;
- mounts the existing `PagesAuthenticatedInlineEditSurface`;
- emits `/assets/pages-inline-edit-bootstrap.js` only on this registered authoring page.

The bootstrap script first requires `#pages-inline-edit-client-root` with `data-pages-authoring-route="true"`. Only then does it import the dedicated client module and WASM. Anonymous storefront HTML does not reference this script and therefore does not fetch the authoring graph.

## Client artifact contract

The host crate now supports a `cdylib` output for the already opt-in `pages-inline-edit-hydrate` profile and exports:

```text
start_pages_inline_edit_client
```

The export is compiled only for `pages-inline-edit-hydrate` on `wasm32`. It reads bounded page/locale identity from the authoring root, removes the SSR shell, and mounts one client authoring surface. The signed authorization proof remains inside the existing server-function transport and is not copied into DOM.

Fixed deployment paths are declared in `apps/storefront/Cargo.toml`:

```text
/assets/pages-inline-edit-bootstrap.js
/assets/pages-inline-edit/rustok_storefront.js
/assets/pages-inline-edit/rustok_storefront_bg.wasm
```

`apps/storefront/scripts/build-pages-inline-edit-client.mjs` is the retained source builder. It is designed to:

1. build `rustok-storefront` for `wasm32-unknown-unknown` with only `pages-inline-edit-hydrate`;
2. run `wasm-bindgen --target web`;
3. place JS/WASM under the fixed asset directory;
4. copy the small bootstrap module to the asset root.

The builder was not run. No generated client file is committed and no asset server or deployment was observed.

## Unchanged ownership

This slice does not add:

- a second editor document;
- direct DOM-to-database persistence;
- a new Pages body write path;
- an unsigned or fallback grant;
- an anonymous authoring mount;
- a public Pages route change;
- a database migration;
- a GraphQL or REST mutation;
- a publish, rollback, route, cache or event schema change.

`AuthenticatedInlineEditSession` remains the sole Fly mutation adapter and `PageService::save_document(expected_revision)` remains the sole Pages document persistence owner.

## Source evidence

- `apps/storefront/src/modules/core.rs`
- `apps/storefront/public/assets/pages-inline-edit-bootstrap.js`
- `apps/storefront/scripts/build-pages-inline-edit-client.mjs`
- `apps/storefront/Cargo.toml`
- `apps/storefront/build.rs`
- `apps/server/src/middleware/auth_context.rs`
- `crates/rustok-pages/contracts/evidence/pages-authenticated-authoring-route-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs`

## Validation status

The source status is explicit: artifact build and browser execution remain pending. Tests, verifiers, Cargo, formatting, SSR/WASM checks, dependency graphs, HTTP hosts, asset delivery, browsers, workflows and CI were not run by the implementation agent.

Suggested commands, intentionally not run:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs
cargo test -p rustok-server auth_context -- --nocapture
cargo test -p rustok-storefront --features pages-inline-edit,ssr --all-targets -- --nocapture
cargo check -p rustok-server --features pages-inline-edit
cargo check -p rustok-storefront --no-default-features \
  --features pages-inline-edit-hydrate --target wasm32-unknown-unknown
node apps/storefront/scripts/build-pages-inline-edit-client.mjs
node crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs
```

Acceptance still requires retained command output, generated artifact inspection, same-origin asset delivery, authenticated/anonymous HTTP evidence and observed browser editing with stale/replay failure cases.
