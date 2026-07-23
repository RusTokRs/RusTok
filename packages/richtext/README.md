# `@rustok/richtext`

## Purpose

`@rustok/richtext` is the framework-neutral browser authoring runtime for the
RusToK `RichTextDocument` contract. It packages one vanilla Tiptap editor for
Next and Leptos hosts without creating a persistence or backend module.

## Responsibilities

- Build the editor schema and toolbar from server-exported profiles.
- Run authoring code in an opaque-origin sandboxed frame.
- Expose one bounded, sequenced `MessageChannel` protocol.
- Supply thin browser and React lifecycle adapters.
- Produce immutable, self-contained frame assets.

The package does not select locale, call application APIs, persist drafts,
render production read HTML, or own module data. Visible labels are supplied by
the host. Server validation and rendering remain authoritative.

## Entry points

- `@rustok/richtext` — documents, profiles, messages, validation, and commands.
- `@rustok/richtext/frame` — framework-neutral frame controller.
- `@rustok/richtext/react` — thin React frame component.
- `@rustok/richtext` `mountLeptosRichTextFrame` — the Leptos wasm lifecycle
  binding, called from `on_mount` and disposed from `on_cleanup`.
- `dist/asset-manifest.json` — immutable frame artifact lookup.

## Interactions

The generated contract mirrors `rustok-api::RichTextDocument` and
`rustok-content::richtext` profiles. Next and Leptos hosts serve the same files
from `dist/` with the headers documented in the central
[richtext implementation plan](../../docs/modules/rich-text-implementation-plan.md).

See [`docs/README.md`](./docs/README.md) for the runtime contract.
