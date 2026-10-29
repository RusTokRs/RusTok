# leptos-ui / CRATE_API

## Public surface

### Composites & Local Components
- `LanguageToggle`: locale selector component (`ui_language_toggle`, `LanguageToggleOption`).
- `RichTextEditorFrame`: isolated rich-text editor iframe host component.
- `RichTextHtml`: safe HTML renderer for typed server-derived `RichTextView`.
- `SuccessMessage`: success status message display component (`ui_success_message`).
- `TableOfContents`: table of contents component with heading parsing (`ui_table_of_contents`, `TocItem`, `extract_headings_from_html`).

### Re-exported UI Primitives
- Re-exports the complete set of components and types from `rustok-ui-leptos` (`Button`, `Input`, `Select`, `Dialog`, `Card`, `Badge`, `Alert`, `Tabs`, `Progress`, `Spinner`, etc.).

## Invariants

- **Boundary Separation**: Leptos-specific composites reside here, while foundational primitives and class resolvers remain owned by `rustok-ui` and `rustok-ui-leptos`.
- **Safe Rich Text**: `RichTextHtml` only renders validated, typed `RichTextView` representations; raw arbitrary HTML injection is rejected.
- **SSR Parity**: Composites render identically in server-side HTML generation and client hydration.
