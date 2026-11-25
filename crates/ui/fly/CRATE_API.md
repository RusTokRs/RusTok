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
- `DesignToken`, `ThemeCommand`: Design token management (`Upsert`, `Remove`, `Rename` with exact CSS `var()` reference rewriting, `Replace`), `find_token_usages`, and reversible history tracking. Rename includes nested fallbacks but leaves quoted strings, comments and unrelated custom-property text untouched.

### Runtime, Binding, and Expression System
- `RuntimeBinding`, `BindingCatalog`, `BindingTarget`: Dynamic data binding descriptors and materialization.
- `RuntimeContextContract`, `RuntimeContextPreflight`: Type-checked context contract validation and dependency analysis.
- `LocaleResolver`: Pluggable trait for BCP-47 locale identity resolution (`PlatformLocaleResolver` with CLDR support or `BasicLocaleResolver`).
- `RenderPolicy`, `RenderedPage`, `render_page`, `render_page_with_embed_registry`, `render_page_with_rich_text_and_embed_registry`: Server-side HTML/CSS rendering with built-in or host-provided embed catalogs.
- `escape_css_attribute`: Allow-list escape of a value written into a double-quoted CSS attribute selector inside a `<style>` element. Used by the renderer and by framework adapters that style a component by its id.
- `DynamicCatalog`, `RuntimeCondition`, `RuntimeRepeater`: Dynamic condition and iteration rendering with safety bounds.
- `validate_project_with_embed_registry`, `validate_patch_incremental_with_embed_registry`: Canonical full validation against the host provider catalog. The patch-target entrypoint checks target existence and delegates to full validation; a prior report does not prove project-wide invariants.
- `render_page_with_runtime_context_with_embed_registry`, `render_page_with_runtime_context_with_providers_and_embed_registry`: Runtime rendering with the same host embed catalog, optionally alongside provider form endpoints.

## 2. Invariants & Business Rules
- **Zero UI Framework Dependencies**: The core engine contains no imports of Leptos, Dioxus, React, or browser-specific globals.
- **Safe Rust**: The entire crate forbids unsafe code (`#![forbid(unsafe_code)]`).
- **Lossless Round-Trip**: The GrapesJS codec guarantees lossless preservation of unrecognized attributes, custom traits, and raw CSS rules.
- **Undo/Redo Integrity**: History retains before/after snapshots for undo and redo, including when an inverse command exists. The budget is a conservative JSON-size proxy, not exact heap accounting; shared `Arc` snapshots can be counted more than once. Eviction retains one oversized entry and preserves a contiguous undo/redo sequence; deserialization rebuilds accounting from the stored entries. Undo/redo verify the current endpoint and validate the destination against current policy before moving either stack.
- **Snapshot diff integrity**: Comparisons fingerprint actual payloads, retain every occurrence of an identity, and report page, asset and style-rule position changes, including opaque entries and shifted siblings.
- **Inverse metadata**: Batch transactions and content patches that remove scalar text children have no local inverse; undo and redo restore exact document snapshots. Style inverses restore the full original slot (including absence).
- **Snapshot capture**: Verify payload round-trip and fingerprint the actual payload before committing the catalog sequence; exhausted/duplicate identities or zero capacity fail without mutation.
- **Move insertion slots**: Move indices refer to pre-removal slots. The implicit first-page root and its explicit ID resolve to the same parent, and inverse moves account for the shifted slot when moving left.
- **Root placement**: Omitting the insertion parent resolves the first page root and applies the same registered child-type policy as its explicit component ID. Opaque child storage is not an empty insertion list; missing move sources and page roots are not draggable.
- **Structural ancestry**: Placement and recursive-move checks use the indexed page and strict component path prefix, not a chain of parent IDs. Anonymous intermediate nodes cannot hide a descendant, and duplicate IDs retain first-occurrence diagnostic lookup without redirecting the structural ancestry walk.
- **Document cache coherence**: `ProjectDocument::project()` exposes a read-only project view. `project_mut()`, `component_mut()` and indexed mutable component access invalidate the cached project hash and component index before returning a mutable borrow. The raw `project` field is private, so no consumer can bypass cache invalidation. Compound theme/symbol mutations stage a candidate document so validation errors do not leak partial writes.
- **Identity remapping**: Component IDs, authored HTML IDs and provider form IDs have distinct ownership. Recognized action, HTML/ARIA, style and runtime-catalog references are remapped in their own namespaces; opaque provider data, assets and independent symbol definitions are not recursively rewritten. A form ID also names its provider endpoint and is not silently minted. Separate pages may reuse authored HTML IDs; duplicate component or form IDs, duplicate HTML IDs within a page, or a form ID claimed by another HTML node make cloning ambiguous and are rejected.
- **Action targets**: SSR reserves `data-fly-target-id`, `data-fly-style-id` and editor identity hooks against authored spoofing. It adds a plain HTML ID for an action target only if the ID is unoccupied and the target has no authored HTML ID.
- **Inline-only theme tokens**: With `RenderPolicy.emit_style_hooks=false`, project CSS and style identity hooks remain disabled, but validated theme custom properties are emitted on the rendered root element so inline descendant styles can resolve them. Embed component styles follow the same inline path.
- **Leptos SSR history controls**: In browser-JS mode, Undo and Redo expose `intent:undo` and `intent:redo` controls before WASM hydration; edit/preview mode controls remain WASM-only.
- **Project Resource Budgets**: JSON decoding matches serde_json's 128-level parser ceiling; validation defaults to 64 component levels and 10,000 components, pages, assets, and style rules. Rendering independently checks those collection ceilings and component depth/node budgets before recursive helpers.
- **Iteration Limits**: Repeater operations enforce strict bounds (`DEFAULT_REPEATER_LIMIT`, `MAX_REPEATER_LIMIT`, `MAX_TOTAL_REPEATED_NODES`). The global work budget charges each copied subtree node before recursive expansion, including anonymous and intermediate nodes; the public repeated-nodes counter still counts generated repeater instances.
- **Scoped bindings**: the full runtime pipeline captures alias-dependent bindings per surviving repeater instance, with nested lexical shadowing, and applies values after interpolation. Content, safe CSS property names and non-identity attributes are supported; identity/reference and provider contract changes are blocked. The standalone binding pass remains global-only.
- **Runtime binding ownership**: one binding per component slot; case-insensitive HTML/CSS names, exact extension fields. Duplicate IDs/slots block conflicting writers; number transforms preserve JSON integer precision, explicit null fallback round-trips, and fallback is missing-path-only.
- **Computed context**: structural dependency order is shared by validation and evaluation, including ancestor/descendant reads. Computed slots discard stale input/default values (failed indexed slots become null without shifting siblings). Fallbacks settle before dependent values; cyclic or overlapping targets are blocking, and the graph is bounded to 100,000 edges. Repeater aliases must be distinct single context keys within each scope; nested shadowing remains supported.
- **Runtime context paths**: at most 2048 bytes, 64 segments and index 9999; a single write may extend arrays by at most 10,000 total slots. Preflight rejects invalid shape or sparse growth before mutation. Duplicate target diagnostics compare parsed segments, not path spelling.

## 3. Error Handling & Edge Cases
- All errors are strongly typed via `FlyError` without panic or stringly-typed swallowing.
- `switch_slide` actions require exactly one navigation selector (`target_slide`, `direction`, or `index`); malformed direction/index form values and ambiguous selectors are rejected.
- `ThemeCommand::Replace` rejects duplicate token names rather than silently keeping one value.
- Computed format expressions validate every `{{path}}` placeholder; empty and unclosed placeholders are blocking validation errors even when earlier placeholders are valid.
- Validation failures produce structured diagnostics (`ValidationDiagnostic`, `AuditDiagnostic`) with explicit severity levels (`ValidationSeverity`, `AuditSeverity`).
- Invalid or colliding generated component IDs are reissued with bounded retries and a deterministic fallback. Ambiguous existing form/HTML/component identities are reported instead of silently choosing an owner.
- Empty projects, missing templates, and circular symbol references fail closed with descriptive domain errors.

## 4. Security & Isolation Invariants
- Host publish policies reuse the public URL-shape helpers, `validate_srcset_candidates` (with a host resource-admission callback), and `css_property_allowed`; hosts can narrow policy without duplicating renderer grammar.
- SSR and validation share head URL admission (including Twitter images) and bounded responsive image candidate validation. Absolute HTTP(S) URLs require a parseable host/port and reject userinfo credentials.
- Safe URL validation (`validate_component_public_urls`, `validate_embed_url`) forbids `javascript:`, executable data URIs, HTTP(S) userinfo credentials, embed non-default ports, and paths that escape registered provider surfaces.
- Theme-token rename and inline-only rendering semantics are recorded in [the Fly theme-token reference ADR](../../../DECISIONS/2026-11-25-fly-theme-token-reference-and-inline-rendering.md).
- HTML sanitization and safe SSR rendering prevent injection of executable script tags or malicious event handlers.
- Multi-tenant bundle export/import validates cryptographic SHA-256 digests (`bundle_hash`, `sha256_hex`, `constant_time_eq`) under strict policy enforcement.
- Strict iframe sandboxing for embedded providers (`EMBED_IFRAME_SANDBOX`).

## 5. Contract Tests & Verification
- Unit tests cover codecs, command inversion, snapshot/history budgets, identity collisions and remapping, validation, SSR generation, and symbol cycle detection. Run the actual workspace variants with `scripts/fly-check.sh test`; an isolated no-default-features build is not a substitute for platform-i18n or consumer checks.
- Property-based testing via `proptest` verifying lossless codec round-trip on arbitrary component trees.
- `src/contract_tests.rs`: Direct contract verification of public API traits, manifest metadata, depth guards, and zero-framework independence.
