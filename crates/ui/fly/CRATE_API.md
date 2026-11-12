# Crate: `fly`

Fly Visual Editor Engine: framework-neutral visual editor engine, AST models, design system specifications, and lossless GrapesJS compatibility core.

## 1. Public API Surface

### Primary Engine & Document Types
- `FlyEditor`: Command processor and state coordinator managing document revisions, transaction batches, and undo/redo history.
- `ProjectDocument`: Root document tree containing components, style rules, symbols, and asset registries.
- `ComponentNode`: Individual component node representing HTML elements, text nodes, or custom blocks with typed and opaque attribute storage.
- `GrapesJsCodec`: Lossless bidirectional serializer and deserializer for GrapesJS project schemas.
- `History`, `HistoryEntry`: Undo/redo history manager with configurable memory budget (`DEFAULT_HISTORY_MEMORY_BUDGET_BYTES`).
- `EditorCommand`: Atomic document manipulation commands (`InsertComponent`, `RemoveComponent`, `UpdateAttributes`, `PatchStyleRule`, etc.).
- `ProjectBundle`, `BundleInspection`: Exportable, versioned project archive (`encode_project_bundle`, `decode_project_bundle`, `inspect_project_bundle`).
- `AuditReport`, `AuditDiagnostic`: Preflight page auditing models for accessibility, broken links, and SEO metadata.
- `DesignToken`, `ThemeCommand`: Design token management (`Upsert`, `Remove`, `Rename` with automatic usage rewriting, `Replace`), `find_token_usages`, and reversible history tracking.

### Runtime, Binding, and Expression System
- `RuntimeBinding`, `BindingCatalog`, `BindingTarget`: Dynamic data binding descriptors and materialization.
- `RuntimeContextContract`, `RuntimeContextPreflight`: Type-checked context contract validation and dependency analysis.
- `LocaleResolver`: Pluggable trait for BCP-47 locale identity resolution (`PlatformLocaleResolver` with CLDR support or `BasicLocaleResolver`).
- `RenderPolicy`, `RenderedPage`, `render_page`: Server-side HTML and CSS renderer generating static markup from document models.
- `escape_css_attribute`: Allow-list escape of a value written into a double-quoted CSS attribute selector inside a `<style>` element. Used by the renderer and by framework adapters that style a component by its id.
- `DynamicCatalog`, `RuntimeCondition`, `RuntimeRepeater`: Dynamic condition and iteration rendering with safety bounds.

## 2. Invariants & Business Rules
- **Zero UI Framework Dependencies**: The core engine contains no imports of Leptos, Dioxus, React, or browser-specific globals.
- **Safe Rust**: The entire crate forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Lossless Round-Trip**: The GrapesJS codec guarantees lossless preservation of unrecognized attributes, custom traits, and raw CSS rules.
- **Undo/Redo Integrity**: All applied `EditorCommand` operations produce invertible history entries that strictly restore prior state without document drift.
- **Node & Depth Budgets**: Deeply nested trees exceeding depth limits (`MAXIMUM_DECODE_DEPTH`, `DEFAULT_MAXIMUM_DEPTH`) or pathological payloads are rejected fail-closed without recursion stack overflow.
- **Iteration Limits**: Repeater operations enforce strict bounds (`DEFAULT_REPEATER_LIMIT`, `MAX_REPEATER_LIMIT`, `MAX_TOTAL_REPEATED_NODES`) to prevent denial-of-service via unbounded expansion.

## 3. Error Handling & Edge Cases
- All errors are strongly typed via `FlyError` without panic or stringly-typed swallowing.
- Validation failures produce structured diagnostics (`ValidationDiagnostic`, `AuditDiagnostic`) with explicit severity levels (`ValidationSeverity`, `AuditSeverity`).
- Idempotent ID collision resolution: conflicting or hostile IDs are sanitized and remapped via `UniqueIdGenerator` rather than causing corrupt state.
- Empty projects, missing templates, and circular symbol references fail closed with descriptive domain errors.

## 4. Security & Isolation Invariants
- Safe URL validation (`validate_component_public_urls`, `validate_embed_url`) forbids `javascript:`, data URIs with executable MIME types, and hostile network paths.
- HTML sanitization and safe SSR rendering prevent injection of executable script tags or malicious event handlers.
- Multi-tenant bundle export/import validates cryptographic SHA-256 digests (`bundle_hash`, `sha256_hex`, `constant_time_eq`) under strict policy enforcement.
- Strict iframe sandboxing for embedded providers (`EMBED_IFRAME_SANDBOX`).

## 5. Contract Tests & Verification
- Unit test suite: 301 comprehensive tests covering codecs, command inversion, history budgets, validation algorithms, SSR generation, and symbol cycle detection.
- Property-based testing via `proptest` verifying lossless codec round-trip on arbitrary component trees.
- `src/contract_tests.rs`: Direct contract verification of public API traits, manifest metadata, depth guards, and zero-framework independence.
