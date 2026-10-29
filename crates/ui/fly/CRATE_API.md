# fly / CRATE_API

## Public surface

### Primary Engine Types
- `FlyEditor`: command processor and state coordinator managing document revisions, transaction batches, and undo/redo history.
- `ProjectDocument`: root document tree containing components, style rules, symbols, and asset registries.
- `ComponentNode`: individual component node representing HTML elements, text nodes, or custom blocks.
- `GrapesJsCodec`: lossless bidirectional serializer and deserializer for GrapesJS project schemas.
- `History`: undo/redo history manager with configurable memory budget (`DEFAULT_HISTORY_MEMORY_BUDGET_BYTES`).
- `EditorCommand`: atomic document manipulation commands (`InsertComponent`, `RemoveComponent`, `UpdateAttributes`, `PatchStyleRule`, etc.).
- `ProjectBundle`: exportable, versioned project archive (`encode_project_bundle`, `decode_project_bundle`).
- `AuditReport`, `AuditDiagnostic`: preflight page auditing models for accessibility, broken links, and SEO metadata.

### Runtime, Binding, and Expression System
- `RuntimeBinding`, `BindingCatalog`, `BindingTarget`: dynamic data binding descriptors.
- `RuntimeContextContract`, `RuntimeContextPreflight`: type-checked context contract validation and dependency analysis.
- `LocaleResolver`: pluggable trait for BCP-47 locale identity resolution (`platform-i18n` with CLDR support or standalone).
- `SsrRenderer`: server-side HTML and CSS renderer generating static markup from document models.

## Invariants

- **Zero UI Framework Dependencies**: The core engine contains no imports of Leptos, Dioxus, React, or browser-specific globals.
- **Safe Rust**: The entire crate forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Lossless Round-Trip**: The GrapesJS codec guarantees lossless preservation of unrecognized attributes, custom traits, and raw CSS rules.
- **Undo/Redo Integrity**: All applied `EditorCommand` operations produce invertible history entries that strictly restore prior state without document drift.
