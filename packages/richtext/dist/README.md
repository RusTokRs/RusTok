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
- Supply thin browser and React lifecycle adapters; disposed controllers ignore
  queued handshakes and port events rather than invoking owner callbacks or
  recreating a channel.
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

## Dependency installation

All `@tiptap/*` dependencies use the same exact release (`3.30.5`), including
locked transitive peers. Use `npm ci` and `npm run verify`; no forced/legacy peer
resolution is required. The Next hosts' npm locks carry the same local-package
contract. Dependency-contract tests check the cohort, lock metadata and matching
host React/render/type versions. Host tooling currently uses ESLint 9.39.4 and
TypeScript 6.0.3 to satisfy the installed Next/TypeScript-ESLint peer contracts.
