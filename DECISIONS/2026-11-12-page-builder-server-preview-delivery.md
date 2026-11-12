# Page Builder server preview delivery

- Date: 2026-11-12
- Decision status: Proposed
- Implementation status: In progress
- Owners: `rustok-page-builder` (admin), `rustok-pages` (admin), `fly-leptos`, `apps/admin`, `apps/server`
- Extends: [Fly SSR control contract and route-carried view state](./2026-11-11-fly-ssr-control-contract-and-route-view-state.md), [Fly SSR-first browser runtime](./2026-07-16-fly-ssr-first-browser-runtime.md)
- Supersedes: None
- Superseded by: None

## Context

The SSR-first decision makes `ssr + browser-js` the default Page Builder admin runtime, and the
control-contract ADR makes every working control a pair: a Rust view emits a protocol attribute and
`fly-browser.js` (or the host bootstrap) binds it. Three defects are open against that model, and
they are one question asked three times — *how does a rendered draft reach the browser?*

1. **The server preview cannot be requested in the default runtime.** `ServerPreviewPanel` renders
   a button whose `on:click` calls `AdminEditorRuntime::request_server_preview`. A server-rendered
   document has no Leptos handlers, so nothing calls it. No browser intent and no SSR form produces
   a `PageBuilderCapabilityRequest::Preview` either: `AdminCanvasController`'s only request effect
   is `Persist`, which builds a `Publish`. A server-rendered editor therefore renders the panel's
   empty state forever, and the button is an inert control that the contract check does not see
   because it is not a `form[data-fly-intent-form]`.

2. **The preview is rendered under the admin policy, not the storefront's.** The panel embeds the
   document through `srcdoc`, and a `srcdoc` document inherits the policy of the document that
   embeds it. Under `apps/admin/src/app/security.rs` the policy is
   `style-src-attr 'none'` and `default-src 'self'`, so a `style` attribute is dropped and a
   cross-origin embed frame, font or media file is refused. `fly` writes
   `style="aspect-ratio:…"` on an embed block, so the preview of a page that publishes correctly
   is visibly not the published page.

3. **The standalone SSR admin does not render the editor at all.** `PagesAdmin` loads the page list
   and the open page through `LocalResource`, which never resolves under `leptos/ssr`, and the
   standalone shell deliberately renders no `HydrationScripts`. The `ssr + browser-js` editor is
   therefore rendered by crate render tests and Playwright fixtures and by no host.

The published-page artifact already solves the transport half of (2): `/api/pages/{id}/artifact` is
its own response, with its own `content-security-policy` (`style-src 'sha256-<css hash>'`,
`frame-ancestors 'self'`), `nosniff`, and cache headers. The preview needs the same shape, not a
second policy vocabulary.

## Decision

1. **A preview is a request, not a click.** The server preview is reachable through the same two
   channels every other control uses, in this order of preference:
   - the host bootstrap posts `form[data-fly-intent-form="render_server_preview"]` to the browser
     intent endpoint, and the intent resolves to a `Preview` capability request;
   - a host that renders without the bootstrap submits the same form natively.

   The panel renders that form instead of the `on:click` button, so the control is part of the
   two-sided contract and the contract verifier derives it like any other form. The `on:click`
   path stays for the hydrated profile and is not the SSR path.

2. **A preview document is delivered as its own response when the host can serve one.** A preview
   that is meant to be faithful to the published page is a document with its own policy, because
   the inherited policy of an embedding admin document cannot be the storefront's policy. The
   delivery order is:
   - **Own document (target).** A preview endpoint returns the rendered draft with the artifact
     policy of the page it previews, and the panel embeds it by `src`, with
     `frame-ancestors` naming the admin origin. This is the only shape that can render an embed
     frame, a remote font or a remote media file exactly as publication would.
   - **`srcdoc` (interim, and only when the policy admits it).** A host that has no preview route
     embeds the document through `srcdoc`, and the rendered draft carries no `style` attribute and
     no cross-origin resource, so what the panel shows is a subset the policy permits rather than
     a preview that silently loses styling.

   A host must not claim the second shape is `faithful to the published page`; the panel states
   which shape it is rendering.

3. **The preview never widens the admin policy.** The preview document policy is the artifact
   policy — `default-src 'none'`, a `style-src` of the document's own hash, `frame-ancestors`
   naming only the host that embeds it. `style-src-attr 'unsafe-inline'`, a script source, and a
   relaxed `default-src` are not added to the admin document to make `srcdoc` work.

4. **The host owns which runtime it renders, and says so.** A host that renders the editor on the
   server resolves the page before it renders (`Resource` under a route mode that needs no script),
   or it ships its hydration bundle. A host that does neither does not render an editor that
   cannot work: it renders the reason. `LocalResource` under `leptos/ssr` is not a supported way to
   load editor state, and a shell that omits `HydrationScripts` must not mount a view whose
   controls depend on a browser script.

## Sources of truth and ownership

- Canonical owner of the preview render: `rustok-page-builder`
  (`PageBuilderRenderer::render_runtime_document_html`), reached through the
  `PageBuilderPreviewRenderingPort` the consumer implements.
- Canonical owner of the draft: the Page Builder draft session store. The Pages consumer owns the
  session identity (`PagesBuilderSaveSnapshot`, `DraftSessionOwner`) and the browser-intent
  endpoint (`dispatch_pages_browser_intent_with_capabilities`).
- Canonical owner of the preview *policy*: the Pages artifact policy for the page being previewed.
  The admin policy stays what it is.
- Canonical owner of the rendered document envelope: `fly`
  (`compose_document_html`, `document_html_with_style_nonce`).
- Derived: the `srcdoc` string and the nonce substitution are presentation, never authority.

## Invariants

- Every SSR-rendered control that performs an action names a `BrowserIntentKind` some handler
  resolves; the preview control is one of them.
- A rendered preview is never produced from a body the caller may not read: the preview request
  goes through the same `pages:read` / editor-capability preflight as every other intent.
- The admin document's policy does not gain a script source, `style-src-attr`, or a relaxed
  `default-src` for the sake of a preview.
- A preview response carries the policy of the page it previews, derived from its own rendered
  CSS, exactly as the artifact response does.
- A host that mounts the SSR editor either makes its state resolvable on the server or ships the
  hydration bundle; it never renders an editor whose controls cannot act.

### Allowed states

- A hydrated admin: `on:click` requests the preview and the frame renders.
- An SSR admin with a preview route: the form posts, the route answers with a policy-carrying
  document, the frame loads it by `src`.
- An SSR admin without a preview route: the form posts, the intent answers with the HTML, the
  panel renders the policy-admitted subset and labels it as such.

### Forbidden states

- A rendered preview control with no intent behind it.
- A preview document rendered under the admin's nonce policy and presented as faithful.
- Widening the admin policy to accommodate an embedded preview.
- Mounting `LocalResource`-backed editor state in an SSR render that emits no hydration script.

## Non-goals

- This decision does not make `LocalResource` resolve under SSR, and does not convert the 68 files
  that use it to another resource type.
- It does not add a preview route for any module other than Pages; the port and the policy shape are
  the reusable parts.
- It does not decide whether the standalone admin ships hydration; it requires the host to decide
  and to act consistently.

## Data, transaction, and concurrency boundary

The preview does not persist anything. It renders the draft session the caller already holds and
does not advance a revision, a generation, or a publish receipt. A preview request is bounded by
`PAGE_BUILDER_PORT_DEADLINE` like every other capability request, and the rendered result is
refused when the project hash or runtime context changed while it was rendering — the existing
`preview_request` comparison, which the SSR path must keep rather than bypass.

## Context dimensions

Tenant and principal participate exactly as they do for the browser-intent endpoint: the intent is
preflighted against the caller's narrowest editor capabilities, and the renderer verifies that the
context tenant and actor are its own. Locale participates through the runtime context
(`set_runtime_locale`), not through the preview request. Channel and timezone do not participate.

## Events and projections

No events. The preview is not a publish, emits no outbox row, invalidates no cache generation, and
does not touch the immutable artifact set.

## Failure semantics

- A preview requested without the read capability fails the preflight, before any render.
- A render failure is reported to the panel with its message; it is not retried automatically and
  it does not mark the draft dirty or failed.
- A preview whose document cannot be embedded under the host policy renders the documented subset
  and says so, rather than silently displaying an adulterated page as faithful.
- A host that cannot resolve editor state renders the reason it cannot, not an empty editor.

## Migration and cutover

The `on:click` button is replaced by the intent form in the SSR view; the hydrated profile keeps the
handler. No persisted state changes, no route is removed, and an admin that has not implemented the
preview route keeps working through the interim shape. The `render_server_preview` intent name is
added to `BrowserIntentKind` and to the capability policy in the same change that renders the form,
so no build ships a form the endpoint does not know.

## Alternatives considered

- **Keep `on:click` and render the button as disabled under SSR.** Rejected: it hides the defect
  rather than closing it, and the contract check would still not see the control.
- **Widen the admin policy for `srcdoc` (`style-src-attr 'unsafe-inline'`, `img-src https:`).**
  Rejected: it weakens the policy of every admin page to style one panel, and it still cannot render
  a cross-origin embed frame.
- **Render the preview client-side from `project_data`.** Rejected: it duplicates the render
  pipeline in a second language and stops being evidence of what the server publishes.
- **Make the preview a browser-only feature and state so.** Rejected: the panel exists to show what
  the server pipeline will publish; a host that cannot show it is exactly the host that needs it.

## Verification

- The SSR control contract verifier derives the preview form and follows it to the intent the
  endpoint resolves, in both directions.
- The admin render tests assert the preview form is rendered and enabled exactly when the endpoint
  admits its intent.
- A render test asserts the panel embeds through `src` when a preview route is configured and
  through `srcdoc` otherwise, and that the `srcdoc` document carries the host nonce on its
  stylesheet.
- A policy test asserts the admin policy is byte-identical before and after this change, and that
  the preview policy carries the hash of the previewed document's own CSS.
- A host test asserts the standalone admin resolves the page on the server or ships hydration; the
  current state (neither) fails it.

## Consequences

The preview becomes part of the contract that is checked mechanically, so it cannot silently rot
again. The interim `srcdoc` shape keeps hosts without a route working while making their fidelity
limit explicit. The standing cost is that the preview route is a second surface that renders
authored content and therefore needs the same policy tests as the artifact route; the decision
keeps that surface owned by the module that already owns the artifact policy, rather than adding a
second policy vocabulary to the admin.
