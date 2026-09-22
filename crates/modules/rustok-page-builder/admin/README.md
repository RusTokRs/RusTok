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

The current canvas supports rendering, geometry, hover, selection, overlays and persistence. It
also provides the block palette, drag-and-drop insertion, resize handles, keyboard editing,
traits/styles editors and asset management. Storefront real-DOM editing remains outside this
authoring package.

## Verification

```bash
node scripts/verify/verify-fly-admin-browser-runtime.mjs
node scripts/verify/verify-pages-ui-boundary.mjs
cargo check -p rustok-page-builder-admin --no-default-features --features hydrate --target wasm32-unknown-unknown
cargo check -p rustok-page-builder-admin --no-default-features --features ssr
```

Rust unit and browser interaction tests must also be run in a checkout with the repository Rust
and WASM toolchains installed.
