# Crate: `fly`

Fly Visual Editor Engine: framework-neutral visual editor engine, AST models, design system specifications, and lossless GrapesJS compatibility core.

## 1. Public API Surface

### Primary Engine & Document Types
- `FlyEditor`: Command processor and state coordinator managing document revisions, transaction batches, undo/redo history, and host embed-provider validation (`with_embed_registry`). The embed registry is host runtime policy and is not serialized with the project.
- `ProjectDocument`: Root document tree containing components, style rules, symbols, and asset registries.
- `ComponentNode`: Individual component node representing HTML elements, text nodes, or custom blocks with typed and opaque attribute storage.
- `GrapesJsCodec`: Lossless bidirectional serializer and deserializer for GrapesJS project schemas.
- `History`, `HistoryEntry`: Undo/redo history manager with configurable serialized-size budget (`DEFAULT_HISTORY_MEMORY_BUDGET_BYTES`), accounting for both snapshot stacks and commands.
- `ProjectDocument::clone_as_page(&mut IdGenerator) -> FlyResult<ProjectDocument>` and `ProjectFragment::remap_ids(&mut IdGenerator) -> FlyResult<BTreeMap<String, String>>`: reissue owned identities and references; ambiguous source identities fail before consuming the generator.
- `EditorCommand`: Atomic document manipulation commands (`InsertComponent`, `RemoveComponent`, `UpdateAttributes`, `PatchStyleRule`, etc.).
- `PageCommand`, `PageLocator`, `PagePatch`: Page add/remove/move/patch operations; page identity is optional for legacy documents, but an explicit id on add or patch must be a valid identifier and is cleared only through `remove_fields`.
- `ProjectBundle`, `BundleInspection`: Exportable, versioned project archive (`encode_project_bundle`, `decode_project_bundle`, `inspect_project_bundle`).
- `AuditReport`, `AuditDiagnostic`: Preflight page auditing models for accessibility, broken links, and SEO metadata.
- `DesignToken`, `ThemeCommand`: Design token management (`Upsert`, `Remove`, `Rename` with automatic usage rewriting, `Replace`), `find_token_usages`, and reversible history tracking.

### Runtime, Binding, and Expression System
- `RuntimeBinding`, `BindingCatalog`, `BindingTarget`: Dynamic data binding descriptors and materialization.
- `RuntimeContextContract`, `RuntimeContextPreflight`: Type-checked context contract validation and dependency analysis.
- `LocaleResolver`: Pluggable trait for BCP-47 locale identity resolution (`PlatformLocaleResolver` with CLDR support or `BasicLocaleResolver`).
- `RenderPolicy`, `RenderedPage`, `render_page`, `render_page_with_embed_registry`, `render_page_with_rich_text_and_embed_registry`: Server-side HTML/CSS rendering with built-in or host-provided embed catalogs.
- `escape_css_attribute`: Allow-list escape of a value written into a double-quoted CSS attribute selector inside a `<style>` element. Used by the renderer and by framework adapters that style a component by its id.
- `DynamicCatalog`, `RuntimeCondition`, `RuntimeRepeater`: Dynamic condition and iteration rendering with safety bounds.
- `validate_project_with_embed_registry`, `validate_patch_incremental_with_embed_registry`: Full and incremental validation against the same host provider catalog.
- `render_page_with_runtime_context_with_embed_registry`, `render_page_with_runtime_context_with_providers_and_embed_registry`: Runtime rendering with the same host embed catalog, optionally alongside provider form endpoints.

## 2. Invariants & Business Rules
- **Zero UI Framework Dependencies**: The core engine contains no imports of Leptos, Dioxus, React, or browser-specific globals.
- **Safe Rust**: The entire crate forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Lossless Round-Trip**: The GrapesJS codec guarantees lossless preservation of unrecognized attributes, custom traits, and raw CSS rules.
- **Undo/Redo Integrity**: History retains before/after snapshots for undo and redo, including when an inverse command exists. The budget is a conservative JSON-size proxy, not exact heap accounting; shared `Arc` snapshots can be counted more than once. Eviction retains one oversized entry and preserves a contiguous undo/redo sequence; deserialization rebuilds accounting from the stored entries.
- **Move insertion slots**: Move indices refer to pre-removal slots. The implicit first-page root and its explicit ID resolve to the same parent, and inverse moves account for the shifted slot when moving left.
- **Structural ancestry**: Placement and recursive-move checks use the indexed page and strict component path prefix, not a chain of parent IDs. Anonymous intermediate nodes cannot hide a descendant, and duplicate IDs retain first-occurrence diagnostic lookup without redirecting the structural ancestry walk.
- **Document cache coherence**: Public mutation helpers invalidate the cached project hash and component index after successful writes; `project_mut`, mutable page access and indexed component mutation invalidate before returning a borrow. The public raw `project` field still allows bypassing this contract; use `project_mut()` for raw writes until the repository-wide field-privacy cutover. Compound theme/symbol mutations stage a candidate document so validation errors do not leak partial writes.
- **Identity remapping**: Component IDs, authored HTML IDs and provider form IDs have distinct ownership. Recognized action, HTML/ARIA, style and runtime-catalog references are remapped in their own namespaces; opaque provider data, assets and independent symbol definitions are not recursively rewritten. A form ID also names its provider endpoint and is not silently minted. Separate pages may reuse authored HTML IDs; duplicate component or form IDs, duplicate HTML IDs within a page, or a form ID claimed by another HTML node make cloning ambiguous and are rejected.
- **Action targets**: SSR reserves `data-fly-target-id`, `data-fly-style-id` and editor identity hooks against authored spoofing. It adds a plain HTML ID for an action target only if the ID is unoccupied and the target has no authored HTML ID.
- **Project Resource Budgets**: JSON decoding matches serde_json's 128-level parser ceiling; validation defaults to 64 component levels and 10,000 components, pages, assets, and style rules. Rendering independently checks those collection ceilings and component depth/node budgets before recursive helpers.
- **Iteration Limits**: Repeater operations enforce strict bounds (`DEFAULT_REPEATER_LIMIT`, `MAX_REPEATER_LIMIT`, `MAX_TOTAL_REPEATED_NODES`) to prevent denial-of-service via unbounded expansion.

## 3. Error Handling & Edge Cases
- All errors are strongly typed via `FlyError` without panic or stringly-typed swallowing.
- `switch_slide` actions require exactly one navigation selector (`target_slide`, `direction`, or `index`); malformed direction/index form values and ambiguous selectors are rejected.
- `ThemeCommand::Replace` rejects duplicate token names rather than silently keeping one value.
- Validation failures produce structured diagnostics (`ValidationDiagnostic`, `AuditDiagnostic`) with explicit severity levels (`ValidationSeverity`, `AuditSeverity`).
- Invalid or colliding generated component IDs are reissued with bounded retries and a deterministic fallback. Ambiguous existing form/HTML/component identities are reported instead of silently choosing an owner.
- Empty projects, missing templates, and circular symbol references fail closed with descriptive domain errors.

## 4. Security & Isolation Invariants
- Safe URL validation (`validate_component_public_urls`, `validate_embed_url`) forbids `javascript:`, executable data URIs, embed credentials/non-default ports, and paths that escape registered provider surfaces.
- HTML sanitization and safe SSR rendering prevent injection of executable script tags or malicious event handlers.
- Multi-tenant bundle export/import validates cryptographic SHA-256 digests (`bundle_hash`, `sha256_hex`, `constant_time_eq`) under strict policy enforcement.
- Strict iframe sandboxing for embedded providers (`EMBED_IFRAME_SANDBOX`).

## 5. Contract Tests & Verification
- Unit tests cover codecs, command inversion, snapshot/history budgets, identity collisions and remapping, validation, SSR generation, and symbol cycle detection. Run the actual workspace variants with `scripts/fly-check.sh test`; an isolated no-default-features build is not a substitute for platform-i18n or consumer checks.
- Property-based testing via `proptest` verifying lossless codec round-trip on arbitrary component trees.
- `src/contract_tests.rs`: Direct contract verification of public API traits, manifest metadata, depth guards, and zero-framework independence.
