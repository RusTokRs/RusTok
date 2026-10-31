# Crate: `leptos-ui`

Stable Leptos component package boundary for design system primitives, rich-text editor frames, and composite UI widgets.

## 1. Public API Surface

### Composites & Local Components
- `LanguageToggle` (`ui_language_toggle`, `LanguageToggleOption`): Locale selector component with reactive option selection.
- `RichTextEditorFrame`: Isolated rich-text editor iframe host component.
- `RichTextHtml`: Safe HTML renderer for typed server-derived `RichTextView`.
- `RichTextFrameCopy`, `localized_richtext_frame_copy`: Localized UI strings and error messages for the rich-text editor frame.
- `SuccessMessage` (`ui_success_message`): Success status message display component.
- `TableOfContents` (`ui_table_of_contents`, `TocItem`, `extract_headings_from_html`, `slugify_heading`): Table of contents component with heading parsing.

### Re-exported UI Primitives
- Re-exports the complete set of components and types from `rustok-ui-leptos`: `Button`, `ButtonVariant`, `Input`, `Select`, `Dialog`, `Card`, `Badge`, `Alert`, `Tabs`, `Progress`, `Spinner`, `Skeleton`, `Switch`, `Checkbox`, `Separator`, etc., along with class generation utilities (`merge_classes`, `button_classes`, etc.).

## 2. Invariants & Business Rules
- **Boundary Separation**: Foundational design primitives and token resolvers live in `rustok-ui` and `rustok-ui-leptos`; Leptos-specific composites reside here.
- **Safe Rich Text Rendering**: `RichTextHtml` strictly requires typed `RichTextView` structures rather than raw arbitrary HTML strings to prevent XSS.
- **SSR & Hydration Parity**: All composite components render deterministic HTML markup during server-side rendering and client hydration.

## 3. Error Handling & Edge Cases
- Rich-text editor payload errors produce localized user-facing feedback without crashing parent views.
- Heading extraction gracefully handles malformed HTML, nested tags, and special characters.
- Empty table-of-contents or missing options render fallback states cleanly.

## 4. Security & Isolation Invariants
- `RichTextEditorFrame` embeds the rich text editor in a sandboxed iframe to prevent untrusted content from accessing the host origin.
- Heading slugification cleanses URI-incompatible characters to generate safe anchor links.

## 5. Contract Tests & Verification
- Unit and contract tests verifying heading slugification, heading extraction from HTML, localized copy defaults, and package metadata.
- `src/contract_tests.rs`: Direct verification of package metadata, re-exported primitives, heading extraction, and localized rich-text copy.
