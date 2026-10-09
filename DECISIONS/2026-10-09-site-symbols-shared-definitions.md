# Site symbols: shared component definitions with instance re-issue

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-page-builder` (semantics and editor surface), `fly` (component model and resolution), `rustok-pages` (site store, load/publish merge)
- Extends: [Multilingual content contract](./2026-03-28-multilingual-content-contract.md), [Page body working copies: draft revisions and body revision journal](./2026-10-09-page-body-draft-and-revision-history.md)
- Supersedes: None
- Superseded by: None

## Context

The functional audit (2026-10-09, gap G-5) found no global symbols or symbol
components: `ProjectFragment` only moves clipboard copies, and there are no
instances updatable from a single source ("change everywhere"). Headers and
CTA blocks are physically copied per page; a rebrand means hand-editing every
page. Webflow Symbols, WordPress reusable blocks, and Tilda global blocks all
solve this with reference components.

Constraints that shape the design:

- Each Pages page body is a standalone Fly document per `(page, locale)`;
  `publish_reviewed` requires the body to contain exactly one Fly page.
- The published artifact is immutable and served policy-free; its sanitized
  source is hash-verified (`publish_sanitization::verify_integrity`), and the
  build identity covers the sanitized payload (`landing_contract` build hash).
- The editor stays transport-free; consumer modules bind ports and carry auth
  context (established by the media `AssetProviderPort` slice).
- The FFA capability envelope (`PageBuilderCapabilityRequest/Response`) is
  pinned by gates and must not grow. Editor commands are the established
  extension point (`AssetCommand`, `StyleRuleCommand`, `PageCommand`, ...).

## Decision

**Reference components ("symbols") whose definitions are edited in one place
and whose instances re-issue on publish.**

1. **Instance shape.** A component may carry `symbol_id` (JSON `symbolId`) and
   an empty child list at rest. The instance is a thin reference; its own node
   shell (id, tag, attributes, style) is preserved as a wrapper at resolution.
   Palette identity type is `symbol` (renders as `div`).
2. **Definition shape.** `SymbolDescriptor { id, name, components }` where
   `components: Vec<ComponentNode>` is the definition body. Definitions live in
   the document under `project.extensions["flySymbols"]`
   (`FLY_SYMBOLS_FIELD`), following the `flyLocales`/`flyPageMeta` extension
   precedent.
3. **Canonical site store.** `rustok-pages` persists definitions in a
   `site_symbols` table keyed by `(tenant_id, locale, symbol_id)`. The body
   document is the editing surface; body saves synchronize the store from the
   document's `flySymbols` block with an optimistic `flySymbolsRevision` token.
   Create without definitions and body-revision restore leave the store alone.
   On editor load and before publish the store is
   merged back (store wins), so a document's embedded copy is a working cache
   and the store is the single source of truth across pages.
4. **Resolution.** `fly::resolve_symbol_instances(document)` produces a copy
   of the document with every instance expanded: the instance shell keeps its
   identity and receives a deep clone of the definition components (ids
   remapped to stay unique). Unknown `symbolId` and reference cycles fail
   closed. Pages publish resolves before `sanitize_static_landing_project`;
   the admin canvas and previews resolve before rendering.
5. **Re-issue semantics.** Definitions resolve freshly at every publish.
   Editing a definition and then reloading, saving, reviewing and publishing
   each dependent page re-issues its occurrences; already published artifacts
   stay immutable until that explicit publish. Reviewed publish compares the
   stored body's definition snapshot with the current site catalog and fails
   closed on drift (including when a scheduled publish executes after a symbol
   edit); this prevents unreviewed global content from slipping into artifacts.
   A `siteSymbolUsage` report lists pages whose current bodies reference a
   symbol so operators know what to re-issue. Bulk automatic re-publish is a
   non-goal for this iteration.
6. **Editor commands.** `EditorCommand::Symbol { command: SymbolCommand }`
   with `SymbolCommand::Upsert { symbol: Value }` and
   `SymbolCommand::Remove { symbol_id }`, applied like `AssetCommand`.
   `Remove` fails closed while any instance references the definition.
   Inserting an instance is the existing `EditorCommand::Insert` of a
   `symbol`-type node. Create-from-selection serializes the selected subtree
   as a `SymbolDescriptor` (the `ProjectFragment` shape already collects the
   component tree).

## Sources of truth and ownership

- `fly` owns instance/reference semantics: `ComponentObject.symbol_id`,
  `SymbolDescriptor`, `SymbolCommand`, validation rules, and
  `resolve_symbol_instances` (pure document in, document out).
- `rustok-pages` owns the canonical cross-page store (`site_symbols`), the
  save-time upsert, the load/publish merge, and the `siteSymbolUsage` report.
- The page body document remains the editing surface and keeps a working copy
  under `flySymbols`; it is a projection of the store after every load.
- `rustok-page-builder` admin owns the Symbols panel (document-local commands
  only; no transport).

## Invariants

- An instance never carries children at rest; children visible on an instance
  are definition content introduced by resolution.
- `resolve_symbol_instances` is deterministic: same document plus same store
  yields the same resolved tree and the same sanitized hashes.
- Resolution fails closed: unknown `symbolId`, non-identifier ids, and cycles
  are errors; publish never renders an unresolved instance.
- `SymbolCommand::Remove` cannot orphan live instances.
- Definition content passes the same static policy as any authored content;
  symbols introduce no new attribute or element surface. Fly caps the site
  catalog at 128 definitions and each definition at 512 component nodes and
  256 KiB of encoded content before it can enter the store.
- The FFA capability envelope is unchanged; symbol flows use `EditorCommand`
  and the existing facade save path.

## Non-goals

- Per-instance content overrides (text/image slots) — instances render the
  definition as-is.
- Cross-locale definition translation — definitions are per locale, matching
  page bodies; translation flows stay as they are.
- Draft/review versioning of symbol definitions separate from body saves.
- Automatic re-publish of all pages that use a changed symbol.
- Symbol nesting cycles as a supported pattern (nesting one definition inside
  another is allowed; cycles are rejected).

## Data, transaction, and concurrency boundary

- `site_symbols` writes happen inside the body-save transaction of
  `rustok-pages` (one aggregate write: body upsert + catalog synchronization).
  The body CAS (`expected_revision`) continues to guard body edits; the
  editor-read `flySymbolsRevision` (SHA-256 of the sorted catalog) guards the
  shared definition set. Postgres uses a transaction-scoped advisory lock per
  `(tenant, locale)` before checking that token; SQLite serializes write
  transactions. Stale saves fail closed instead of erasing symbols added from
  another page. Deletion additionally checks for dependent page instances
  and rejects if any remain.
- Reads (editor load, publish, usage report) are snapshot reads of the store;
  publish resolution sees the store state at publish time. Public page reads
  and immutable served artifacts must not overlay unpublished definitions.

## Context dimensions

- Tenant and locale are part of the store key. Channel is not part of the key
  in this iteration (symbols are site-wide); a channel-scoped variant is an
  additive follow-up if product needs it.

## Events and projections

- No new outbox events. A symbol edit travels with the body save; page publish
  emits its existing events. The `siteSymbolUsage` query is an on-demand report,
  not a projection.

## Failure semantics

- Unknown `symbolId` at resolution: publish/load preview fail closed with a
  validation error naming the symbol id (no partial expansion).
- Cycles: fail closed naming the cycle participants.
- `Remove` with live references: rejected; the report points at referencing
  pages.
- Store merge is idempotent and cannot lose definitions (upserts keyed by id).

## Migration and cutover

- New table `site_symbols` (append-only migration with the standard backfill
  contract entry). Existing documents without `flySymbols` are inert; nothing
  references symbols yet, so cutover is immediate and reversible by simply not
  using symbols.

## Alternatives considered

- **Definitions embedded per document only (no site store):** rejected — the
  body document is per `(page, locale)`, so definitions would diverge per page
  and "change everywhere" would be impossible.
- **Symbol definitions as registry items** (like asset providers): rejected —
  symbols are per-site content created by operators, not code-declared schema.
- **Materialize definition content into every instance at save:** rejected —
  edits would still require rewriting every page body; it only moves the copy
  problem into the writer and breaks body CAS/history semantics.
- **Auto re-publish all dependent pages on definition change:** deferred —
  reviewed publish is an explicit per-page act with its own reviewed runtime
  gate; bulk automation belongs with the publish scheduler rollout, not hidden
  inside a save.
- **Per-instance overrides with slot merge:** deferred as future work; v1 keeps
  the definition authoritative.

## Verification

- `fly` unit tests: descriptor round-trip, command application
  (upsert/remove/keep-identity), validation failures (missing definition,
  orphan removal, cycles), resolution determinism and id remapping.
- `rustok-pages` tests: save upserts the store, load/publish merge wins over
  stale embedded copies, publish resolution fails closed on unknown symbols,
  usage report lists referencing pages.
- Static gates: ADR registry, docs, module source layout, rust constructor
  arity, i18n keys, page-builder contract/parity gates, static publish
  resource-limits and sanitization pins (unchanged), tree-sitter parse of
  changed sources. Compile/test evidence for this slice is CI-owned in this
  environment and reported as such.

## Consequences

- One edit to a definition re-issues every occurrence at the pages' next
  publish; rebranding a header becomes one edit plus re-publish.
- The published artifact model is untouched: resolution happens before
  sanitization, so hash coverage and policy-free serving remain as they were.
- Editors must remember that symbol definition edits propagate at save time
  (they are site-wide immediately), while already published pages update only
  when re-published — the usage report makes that set visible.
- Per-instance overrides, symbol-level versioning, and bulk re-publish remain
  future work (documented in module Known Limitations).
