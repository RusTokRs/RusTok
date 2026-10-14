# Fly rich text, embeds, action runtime, and framework adapters (Phase 2)

Date: 2026-10-14
Status: accepted
Scope: `fly`, `fly-ui`, `fly-leptos`, `fly-dioxus`, `fly-browser`,
`rustok-page-builder-storefront`, `rustok-pages-storefront`, the
`rustok-page-builder` static publish policy, and the page-builder SSR action form.

## Context

The 2026-10-14 deep audit found rich text disconnected (P0-4), custom actions inert
on published pages (P0-5), no embed block (P2-14), and framework adapters that were
empty shells (P0-1). The rich-text capability boundary is already decided
(`DECISIONS/2026-07-22-richtext-capability-boundary.md`): `rustok-content::richtext`
is the platform's single production renderer and browser editors are preview only.

## Decisions

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

## Verification

In this audit sandbox there is no Rust toolchain (no `cargo`/`rustc`, and the
toolchain hosts are unreachable), so `cargo test`/`clippy` were not run for this
phase either. What was run: `rust_syntax_check.mjs` over the changed Rust files,
`rustfmt_check.mjs` (24/24 conform after formatting; the single tree-sitter error in
`style_rule.rs` is a grammar limitation — the file parses and matches under real
rustfmt), `node --check` for `fly-actions.js`, and `scripts/fly-check.sh gates`
(20 passed, 0 failed). Behavioural `cargo test` runs for the new Rust suites (rich-text,
embed, layers, action materialization) are maintainer/CI work and must not be reported
as green until they have actually run.
