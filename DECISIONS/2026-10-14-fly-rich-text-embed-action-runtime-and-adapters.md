# Fly rich text, embeds, action runtime, and framework adapters (Phase 2)

- Date: 2026-10-14
- Decision status: Accepted
- Implementation status: Implemented
- Owners: `fly`, `fly-ui`, `fly-leptos`, `fly-dioxus`, `fly-browser`, `rustok-page-builder`
- Extends: [Fly style states, class rules, unique ids, and head metadata](./2026-10-14-fly-style-states-class-rules-and-head-metadata.md)
- Supersedes: None
- Superseded by: None

## Context

The 2026-10-14 deep audit found rich text disconnected (P0-4), custom actions inert
on published pages (P0-5), no embed block (P2-14), and framework adapters that were
empty shells (P0-1). The rich-text capability boundary is already decided
(`DECISIONS/2026-07-22-richtext-capability-boundary.md`): `rustok-content::richtext`
is the platform's single production renderer and browser editors are preview only.

## Decision

1. **Fly stores rich text; the host renders it.** A component may carry a `flyRichText`
   extension with the existing `RichTextPayload` (`capability: "richtext"` plus an
   opaque ProseMirror/Tiptap-shaped document). `fly::rich_text` owns the storage
   contract, structural validation (root `type: "doc"`, depth/node/byte limits), plain
   text projection, and a conservative escaped fallback projection built only from
   allow-listed block types and marks with URLs filtered through `safe_url`. Semantic
   HTML comes from the host through the `RichTextHtmlRenderer` port
   (`render_page_with_rich_text`). `fly` does not depend on `rustok-api` or
   `rustok-content`; in RusToK the binding lives in the consumer:
   `rustok-pages-storefront` binds `PagesRichTextRenderer` (Article profile) under its
   `ssr` feature, and `rustok-page-builder-storefront` accepts an optional shared
   renderer. An invalid payload renders nothing; validation reports
   `rich_text_payload_invalid` before publish.
2. **Embeds are a closed provider registry, never arbitrary iframes.** `fly::embed`
   ships YouTube (plus youtube-nocookie), Vimeo, and Google Maps providers with exact
   host matches and path prefixes over HTTPS only, a required title, and an `W / H`
   aspect ratio. Valid specs render a sandboxed iframe (`allow-scripts
   allow-same-origin allow-presentation`, lazy, `noopener`-equivalent referrer
   policy); invalid specs degrade to a safe HTTPS link or escaped text, and
   validation reports `embed_invalid`. The component type is `fly_embed`: the static
   publish policy refuses it explicitly, because the sandboxed iframe is outside the
   static tag allow-list — embeds publish through dynamic routes, never by leaking an
   unsandboxed iframe into static HTML. There is still no custom-code block.
3. **One action model, one small client runtime.** The action model gains
   `ToggleVisibility` (toggle/show/hide against a target component, with
   `aria-expanded`/`aria-hidden` synchronization) and `ScrollTo` (smooth scroll and
   focus). Materialization stays server-owned (`data-fly-action`,
   `data-fly-action-kind`, `data-fly-action-target`); `fly-browser` ships
   `fly-actions.js` (`FLY_ACTIONS_JS`), a dependency-free runtime that dispatches
   `fly:action` CustomEvents for `emit_event`/`provider_action` and implements the
   two new behaviours natively. It is idempotent, stoppable, and never intercepts
   native navigate/submit actions. Animation/transition actions remain unimplemented
   and are recorded as such in the `fly` README.
4. **Adapters are built in parallel on shared contracts.** `fly-ui` owns the pure
   editor view-models (layer tree flattening, palette listing). `fly-leptos` and
   `fly-dioxus` each ship a real `FlyEditorWorkspace` (palette, layers, canvas
   preview, undo/redo) bound to a `FlyEditor` + the shared state machine, rendering
   server-side and forwarding every accepted document change to the host. The
   shell components (`FlyFullEditor` with children, etc.) stay source-compatible.
   Migrating the page-builder admin's SSR/WASM panels onto these workspaces is the
   remaining P0-2 work, not a silent omission.

## Sources of truth and ownership

- Rich text storage contract and payload validation: `crates/ui/fly`.
- Production rich-text semantic HTML rendering: `rustok-content::richtext` via host consumer port `RichTextHtmlRenderer`.
- Embed provider registry and sandboxing: `crates/ui/fly`.
- Action execution runtime: `crates/ui/fly/browser/fly-actions.js`.
- View-model flattening and editor workspaces: `crates/ui/fly/ui`, `crates/ui/fly/leptos`, `crates/ui/fly/dioxus`.

## Invariants

- Rich text payloads outside the document schema limits (max nodes, max depth, max bytes) fail validation.
- Embed iframes are strictly limited to registered HTTPS providers (YouTube, Vimeo, Google Maps) with restrictive sandbox flags.
- Action runtime is idempotent, listener-safe, and never intercepts form submissions or native navigational links.
- Static HTML publisher strictly rejects un-sandboxed iframes and arbitrary script injection.

## Non-goals

- In-browser WYSIWYG editor implementation inside `fly` (ProseMirror/Tiptap client is preview-only).
- Arbitrary raw HTML / custom-code injection blocks.
- Client-side document mutation bypassing the `FlyEditor` command boundary.

## Data, transaction, and concurrency boundary

- Document updates containing rich text and embeds are committed through `FlyEditor` commands inside transactional revision envelopes.
- Action definitions attach to components as structured metadata and serialize deterministically.

## Context dimensions

- Rich-text rendering supports tenant- and locale-aware markup resolution via the host port.
- Embed URLs and aspect ratios are responsive across standard viewport breakpoints.

## Events and projections

- Plain-text projection of rich-text content is generated for search indexes and previews.
- Client runtime dispatches synthetic DOM CustomEvents (`fly:action`) for event-bound components.

## Failure semantics

- Invalid rich-text payloads fail closed, emitting empty content rather than malformed HTML.
- Unrecognized or malformed embed URLs fail validation with `embed_invalid` diagnostics and render fallback links.

## Migration and cutover

- Existing documents deserialize without required rich-text or embed extensions.
- Components lacking action attributes function identically to prior static components.

## Alternatives considered

- **Embedding arbitrary iframes / HTML snippets**: Rejected due to high XSS risk and breaking static sandbox guarantees.
- **Direct dependency on `rustok-content` inside `fly`**: Rejected to keep `fly` UI crate free of domain/core coupling.
- **Client-only action handling with external frameworks**: Rejected in favor of a zero-dependency, server-materialized runtime script.

## Verification

- Crate-scoped tests in `fly` (rich-text validation, embed provider allow-list, action data attribute emission).
- `node --check` and lint verification for `fly-actions.js`.
- Static publish policy invariants verified against embed component rejection.

## Consequences

- Modules using `fly` can safely render rich-text and embeds without exposing XSS vectors.
- UI framework adapters (Leptos, Dioxus) share identical view-model structures and state transitions.
