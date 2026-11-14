# RusTok Page Builder Admin

This package is the optional full-authoring surface for the Page Builder module.

## Current responsibilities

It owns:

- the full Fly presentation shell;
- an isolated iframe canvas with an instrumented, CSP-restricted document renderer;
- source/origin/protocol/instance/sequence validation for iframe messages;
- viewport, component geometry, pointer, hover and selection synchronization with `FlyUiStateMachine`;
- selected and hovered overlays in parent canvas coordinates;
- the `AdminCanvasController` that applies `fly-ui` intents to `FlyEditor`;
- the admin FFA facade contract over canonical Page Builder capability envelopes;
- editor-side diagnostics, dirty state, undo/redo, save lifecycle and revision-conflict handling.

## Consumer contract

The package does not own consumer document persistence, tenant/auth loading, GraphQL selection,
native server-function selection, domain widget data, or authoritative backend sanitization.

A consumer provides `PageBuilderAdminHostContext` with an `AdminCanvasController` and an
`Arc<dyn PageBuilderAdminFacade>`. The facade receives canonical capability requests and returns
canonical responses. The editor owns save-start, save-failure and acknowledgement state, including
acknowledgement against the exact project hash that was dispatched.

`frames[0].component` data into canonical `pages[].component`, keeps a synchronized frame snapshot
for the existing JSON fallback, performs optimistic `updated_at` revision checks, and persists
through the existing Pages transport facade.

## Browser boundary

The iframe is sandboxed with `allow-scripts` and without `allow-same-origin`. A `srcdoc` iframe has
an opaque `null` origin, so the parent validates both that origin and the exact `contentWindow`
identity before decoding the feature protocol. Browser listener, observer, pointer-capture and
postMessage lifecycle primitives live in `fly-leptos` and are compiled only for `wasm32`.
The admin package's direct DOM bindings are owned by its `browser-js` feature;
`csr` and `hydrate` enable that feature through `wasm-client`, while `ssr`
keeps the native rendering path independent from browser APIs.

The same feature decides whether the server-rendered editor emits the `fly-browser` adapter.
A consumer sets `default-features = false`, so a consumer feature that enables `ssr` must also
enable `browser-js`; without it the editor renders `<span data-fly-browser-adapter="disabled">`
and no control works. Under a host Content Security Policy with a nonce, the host provides
`rustok_ui_core::UiCspNonce` as render context: a server host from the request, a browser host
from the `nonce` property of a script of its own document. The editor writes it on the adapter
`<script>`, on the `<style>` and `<script>` of the canvas `srcdoc` document, and on the
stylesheet of the server preview document (`fly::document_html_with_style_nonce`). Both
documents inherit the host policy. The adapter workspace reaches the same nonce from the same
context: `AdapterWorkspacePanel` passes it to `fly_leptos::FlyEditorWorkspace`, whose canvas
renders the project stylesheet as an inline `<style>` that the policy would otherwise drop.

A consumer that restores SSR draft sessions provides `SsrDraftRenderBinding`, including the
exact token and generation that produced the rendered controller (or an explicit saved-page
render with no session). Pages supplies this binding. The browser uses the rendered generation,
not a potentially newer sessionStorage value. If a saved-page render finds a cached draft, or the
route and render disagree, editing stops and a same-origin recovery link preserves the token.
Out-of-order responses cannot rewrite the route, and a history-write failure stops editing and
offers recovery instead of reloading the wrong document. For this binding, the host checks the
pathname, `page_id`, draft token and connected root before sending an intent or applying its route
response; an old canvas cannot navigate the newly selected page. This does not make navigation
transactional: stopping can abort a request already committed by the server before its token is
received, and concurrent adapters for the same page can still overwrite the single per-page
sessionStorage pointer. Durable discovery of all in-flight draft sessions remains open. Hosts
without this binding retain their existing browser contract. The Node DOM-boundary test and Chromium host-bootstrap fixture pass;
they use stubbed endpoints and do not replace a running Pages-server integration test.

The current canvas supports rendering, geometry, hover, selection, overlays and persistence. It
also provides the block palette, drag-and-drop insertion, resize handles, keyboard editing,
traits/styles editors and asset management. Storefront real-DOM editing remains outside this
authoring package.

The canvas is a preview surface, not a navigable document: the runtime cancels authored link
activation and form submission inside the iframe, so an authored `<a href>` or form cannot replace
the `srcdoc` document with a page the bridge no longer instruments. The server preview panel is
where a rendered page — including its links and forms — is inspected.

## Verification

```bash
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node scripts/verify/verify-pages-ui-boundary.mjs
cargo check -p rustok-page-builder-admin --no-default-features --features hydrate --target wasm32-unknown-unknown
cargo check -p rustok-page-builder-admin --no-default-features --features ssr
```

Rust unit and browser interaction tests must also be run in a checkout with the repository Rust
and WASM toolchains installed.

## Known Limitations / Pending Implementation

Server preview (`src/editor/server_preview.rs`):

- **Active-page preview error handling is source-only.** Both the browser intent and the Pages
  preview route now call `AdminCanvasController::active_page_project`, which returns a Fly error
  instead of silently producing an empty project when the active page is absent or encoding
  fails. The change still requires crate compilation and regression coverage for both paths.
- **The preview cannot be requested in the default runtime.** "Refresh preview" acts through
  hydration. No intent or SSR form requests a preview, and the draft session holds none.
- **The preview is shown under the policy of the admin.** A `srcdoc` document inherits the
  policy of the page that embeds it, so a `style` attribute, a provider frame of an embed block
  and a font or media file from another origin are refused there although the published page
  shows them. The canvas document has the same limits.
- **Only the Fly document envelope receives the nonce.** A consumer whose preview port returns
  other markup gets it embedded as it arrived, and its inline styles are dropped under a nonce
  policy.

Pages of a document in the default runtime (`src/editor/ssr_inspector.rs`):

- **The page the editor is on is changed through the "Open page" form of the SSR inspector.**
  The page tabs of the workspace act through hydration and do nothing without it.
- **A caller without the edit capability cannot change the page.** The open page is kept in the
  draft session, which the host keeps for a caller that may edit, so the form is not rendered for
  anyone else. Such a caller sees the page the document opens on.
- **Undoing a created page does not return to the page the author came from.** The page is
  removed and the editor shows the last page that is left.

SSR draft session storage (`src/draft_session.rs`):

- **`prune` has one host caller, and it is the standalone admin.** Expired sessions are also
  dropped when they are read (the file store deletes the file under the token lock, the key-value
  backend deletes the key only while it still holds the generation that was read), and the
  per-owner cap is enforced when a session is created. Both are lazy: a session nobody reads again
  would never be collected, and the global cap (`maximum_entries`) would never apply. The
  standalone admin now runs `apps/admin/src/app/draft_retention.rs`
  (`DRAFT_SESSION_PRUNE_INTERVAL`, five minutes) over the store the intent endpoint resolves, so
  a store installed through `set_pages_browser_draft_store` is pruned too. A host that embeds the
  editor but not that binary still owes the same call.
- **Drafts of the pre-partition file layout are not migrated.** Files written directly into the
  storage root by earlier builds are no longer read; `prune` deletes them once they are older than
  one TTL.
- **A damaged record fails its session for up to one TTL.** Records carry a format version
  (`PersistentDraftSessionRecord::format`, now `1`) and both stores read them through
  `PersistentDraftSessionRecord::decode`, so the two causes are distinguishable:
  - A record of a **newer** format than this build understands parses, and is answered with
    `UnsupportedFormat` — a deployment that has to be finished (or rolled back), not damage. The
    record is not aged out and not deleted: it belongs to a build that is still writing the store.
  - A record that does not parse at all is damage: the file store reports `Serialization` for that
    token until the file has not been written for a whole TTL, then treats it as expired and
    deletes it; the key-value backend reports `Serialization` and leaves the record to the TTL the
    driver stored it with.
  Records written before the field existed have format `0` and are read as-is; the next commit
  rewrites them with the current format. An editor that holds the token of a damaged session
  still gets an error for every action until the TTL passes or it opens the page in a new tab.
- **Ownership and mode checks are Unix-only.** On other platforms the storage root is not checked
  for a foreign owner and files get no private mode; access control is whatever the operator set on
  the parent directory.
- **The file lock is advisory and breaks stale locks by age.** A crashed holder blocks its token for
  up to 30 seconds, and two waiters breaking the same stale lock can briefly both hold it (see
  `with_token_lock`). A stale lock that cannot be deleted fails the operation with an I/O error.
- **`DurableSsrDraftSessionStore` extends a session's lifetime on commit only.** Loading a draft
  through the durable store does not slide its expiry; a draft that is only viewed expires one TTL
  after its last edit.
- **An unusable storage root is reported with its path.** The error raised when the file store
  cannot be created names the configured directory, and the host returns that text to the editor
  that hit it.
- **A lookup that misses scans every tenant partition** of the file store before answering.
- **The per-owner cap groups sessions by `tenant_id:user_id`.** Two owners whose identifiers
  concatenate to the same string share one cap.

For the formal Rust API contract and guarantees, see [`CRATE_API.md`](CRATE_API.md).
