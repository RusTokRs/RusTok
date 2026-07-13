# RusTok Page Builder Admin

This package is the optional full-authoring surface for the Page Builder module.

It owns:

- the full Fly presentation shell;
- an isolated iframe canvas;
- the `AdminCanvasController` that applies `fly-ui` intents to `FlyEditor`;
- the admin FFA facade contract over canonical Page Builder capability envelopes;
- editor-side diagnostics, dirty state, undo/redo and publish request emission.

It does not own consumer document persistence, tenant/auth loading, GraphQL selection, native server-function selection, domain widget data, or backend sanitization. A host supplies an implementation of `PageBuilderAdminFacade` and forwards emitted canonical requests through its selected transport.

The iframe is sandboxed without `allow-same-origin`. Browser listener, observer, pointer-capture and postMessage lifecycle primitives live in `fly-leptos` and are compiled only for `wasm32`.
