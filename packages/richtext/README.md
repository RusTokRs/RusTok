# `@rustok/richtext`

## Purpose

`@rustok/richtext` is the framework-neutral browser authoring runtime for the
RusToK `RichTextDocument` contract. It packages one vanilla Tiptap editor for
Next and Leptos hosts without creating a persistence or backend module.

## Responsibilities

- Build the editor schema and toolbar from server-exported profiles.
- Run authoring code in an opaque-origin sandboxed frame.
- Expose one unversioned, bounded, sequenced `MessageChannel` protocol.
- Apply host-selected content locale, derived writing direction, spellcheck and
  dynamic editable/read-only state inside the isolated frame.
- Supply thin browser and React lifecycle adapters.
- Expose a separate editor-free React `RichTextHtml` boundary for rendering a
  typed server-derived `RichTextView`.
- Produce immutable, self-contained frame assets.

The package does not select an owner content locale, call application APIs,
persist drafts, derive or sanitize production read HTML, or own module data. It
canonicalizes the locale supplied by the owner, uses neutral `und` while that
input is unavailable or invalid, and derives only browser authoring behavior.
Visible labels are supplied by the host. Server validation and rendering remain
authoritative.

## Entry points

- `@rustok/richtext` — documents, profiles, messages, validation, and commands.
- `@rustok/richtext/frame` — framework-neutral frame controller.
- `@rustok/richtext/react` — thin React frame component.
- `@rustok/richtext/view` — editor-free React renderer that accepts only a
  typed server-derived `RichTextView`, content locale, and presentation class.
- `@rustok/richtext/next` — server-only Next route handlers for the canonical
  frame document and allowlisted immutable assets with shared security headers.
- `dist/leptos-adapter.mjs` — the browser bridge mounted by a Leptos hydration
  `Effect`; its handle supports controlled document, authoring-context and
  editable updates plus disposal from `on_cleanup`.
- `dist/asset-manifest.json` — immutable frame artifact lookup.

## Interactions

The generated contract mirrors `rustok-api::RichTextDocument` and
`rustok-content::richtext` profiles. Both Next hosts reuse
`@rustok/richtext/next`; Leptos hosts serve the same files from `dist/` with the
headers documented in the central
[richtext implementation plan](../../docs/modules/rich-text-implementation-plan.md).

See [`docs/README.md`](./docs/README.md) for the runtime contract.
