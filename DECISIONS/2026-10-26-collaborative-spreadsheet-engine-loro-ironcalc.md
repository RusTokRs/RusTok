# Collaborative Spreadsheet Engine on Loro CRDT and IronCalc with Live Business References

- Date: 2026-10-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform-architecture, rustok-docs
- Extends: doc://docs/architecture/workplace-collaboration-superapp-plan.md
- Supersedes: None
- Superseded by: None

## Context

Modern enterprise knowledge work and commercial operations require sophisticated numerical computation: price quotation modeling, inventory volume reconciliations, bill-of-materials (BOM) cost rollups, margin analysis, and multi-currency budgeting.

Historically, RusToK planned Notion-grade documents (`rustok-docs`) with dynamic databases backed by `crates/modules/flex` and `crates/ui/rustok-grid` (Table, Kanban, Gallery, Calendar views). While flexible for metadata categorization, relational databases are structurally incapable of replacing computational spreadsheets:
1. They lack fine-grained cell-level formulas (`=SUM(B2:B10)*1.2`, `=VLOOKUP(...)`, trigonometric and financial functions).
2. They cannot handle arbitrary cell coordinate addressing (`A1`, `R1C1`, range slicing `B2:E50`, matrix spills).
3. Concurrent multi-user typing in traditional spreadsheet cells leads to silent overwrite races or corrupt dependency directed acyclic graphs (DAGs).

An architectural audit of state-of-the-art collaboration suites (such as Macro, `macro-inc/macro`) revealed a proven, high-performance paradigm: decoupling collaborative state storage (using pure **Loro CRDT**) from the formula calculation engine (using pure Rust **IronCalc**).

This ADR defines the canonical architecture for native, collaborative spreadsheets in RusToK, elevating prior art by integrating our sovereign **Universal Entity Reference (UER)** protocol to allow spreadsheet formulas to compute directly over live ERP, Commerce, and Accounting entities without brittle third-party sync.

## Decision

1. **Sovereign Block Ownership in `rustok-docs`:**
   Spreadsheets are first-class document blocks (`DocBlock::Spreadsheet`) owned by `rustok-docs`. A single document may embed multiple independent or interlinked spreadsheet workbooks.

2. **Loro CRDT Container Layout:**
   Workbook state is persisted and synchronized exclusively via Loro CRDT containers:
   - `workbookMeta`: Root map storing format version, active sheet GUID, and global styles.
   - `workbookSheets`: Ordered list of sheet metadata (id, name, index, row/column boundaries, hidden flags).
   - `sheetCells_{sheet_id}`: Map of cell coordinates (e.g. `"A1"`, `"C12"`) to typed `SpreadsheetCell` records containing raw formula strings (`=SUM(A1:A10)`), explicit values, cell formats (`currency`, `accounting`, `percent`, `date`), typography, and border descriptors.
   - `sheetTables_{sheet_id}`: Named structured tables enabling structured reference syntax (e.g. `Sales[Amount]`).

3. **IronCalc as the Canonical Calculation Engine:**
   Formula evaluation is delegated to **IronCalc** (pure Rust calculation engine):
   - **Client-Side (WASM):** Evaluated locally in WebAssembly workers for sub-millisecond reactive UI updates upon user keystrokes.
   - **Server-Side (Native Rust):** Evaluated deterministically in background workers for report exports (PDF, Excel `.xlsx`), scheduled automations, and headless integrations.

4. **Live Business Reference Formulas (UER Functions):**
   IronCalc's function table is extended with RusToK-native custom functions communicating through `EntityCardResolverPort`:
   - `=RUSTOK_PRICE("product-sku", "currency")`: Fetches real-time catalog price from `rustok-pricing`.
   - `=RUSTOK_STOCK("warehouse-id", "product-sku")`: Queries real-time available stock from `rustok-inventory`.
   - `=RUSTOK_EXCHANGE("USD", "EUR")`: Resolves effective conversion rates from `rustok-currency`.
   When underlying platform entities update, transactional outbox events trigger cache invalidation and targeted reactive cell recalculations.

5. **AI Agent Sandbox & Disposable Fork Isolation:**
   Autonomous agents never mutate live spreadsheet CRDT states directly. When an AI agent executes a calculation or batch edit:
   - The engine clones a lightweight, in-memory **disposable fork** of the `LoroDoc`.
   - The agent executes scratch calculations and trial operations on the fork.
   - Upon successful verification, the engine computes a bounded, atomic delta (`prepareSpreadsheetEdit`) and merges it into the authoritative document state under a reserved AI peer ID (`is_ai_peer`).

6. **Clean-Room & License Invariants:**
   All components are implemented under RusToK's **BSL 1.1** licensing using strictly permissive foundations:
   - Loro CRDT: MIT / Apache-2.0.
   - IronCalc: MIT / Apache-2.0.
   - Zero copy-pasting of AGPL-3.0 code from external repositories.

## Sources of truth and ownership

- **Canonical Owner:** `crates/modules/rustok-docs` owns the workbook lifecycle, block binding, and permission boundary.
- **Authoritative Persisted State:** The serialized Loro CRDT operation log and periodic binary snapshots stored in PostgreSQL (`docs_pages`, `docs_blocks`, `docs_snapshots`).
- **Derived Ephemeral State:** Evaluated cell matrices (`CalculatedCell`), spill geometries, and IronCalc dependency graphs. Derived calculation matrices are strictly ephemeral and reconstructed on demand.
- **Dependency Direction:** `rustok-docs` -> `crates/libs/rustok-core`, `crates/libs/rustok-crdt` (Loro), `ironcalc`. Domain crates (`product`, `inventory`, `billing`) are referenced strictly through loose UER ports (`EntityCardResolverPort`), preserving module sovereignty.

## Invariants

### Allowed states
- A spreadsheet may contain 1 to 256 sheets, each supporting up to 100,000 rows and 16,384 columns (Excel standard XFD).
- Cell edits by multiple concurrent users or AI agents merge deterministically without merge conflicts via Loro CRDT last-writer-wins per-cell properties.
- Structured reference formulas (`Table[Column]`) dynamically expand when rows are appended or inserted into named tables.
- Offline clients may enter formulas and data locally; on reconnect, deltas synchronize bidirectionally and IronCalc updates the evaluation DAG.

### Forbidden states
- An AI agent MUST NOT write unverified calculations or raw corrupt syntax directly to the live document CRDT tree; all edits must pass through the disposable fork pipeline.
- Formula calculation MUST NOT block the UI main thread; calculation runs strictly in background WASM web workers or async Tokio tasks.
- Circular formula references (e.g. `A1 = B1 + 1`, `B1 = A1 * 2`) must be detected by IronCalc and flagged with `#CYCLE!` / `#CIRCULAR!` error tokens without crashing or infinite loops.

## Non-goals

- Emulating legacy Visual Basic for Applications (VBA) macro runtimes.
- Full 100% parity with obscure 1980s Lotus 1-2-3 quirks.
- Replacing the platform's relational PostgreSQL database for primary transactional indexing.

## Data, transaction, and concurrency boundary

- **Transaction Boundary:** Changes inside a sheet within one user gesture or one AI turn constitute a single atomic CRDT transaction (`doc.commit()`).
- **Revision / Concurrency Control:** Managed mathematically by Loro CRDT VersionVectors and Frontiers. No database row locks or optimistic locking failures (`409 Conflict`) can occur during collaborative typing.
- **Snapshot Policy:** Workbooks write incremental deltas to the operation log. Every 100 operations or every 5 minutes of active editing, an automated worker materializes a compacted Loro binary snapshot to PostgreSQL and MinIO/S3 cold storage.

## Context dimensions

- **Tenant Isolation:** Every workbook is scoped strictly to `tenant_id`. Cross-tenant cell references are rejected at the resolver port.
- **Locale & Number Formatting:** Cell display formats (`displayNumber`, date separators, currency symbols) respect the active user's effective locale (`rustok-ui-i18n`).
- **Authorization & RBAC:** Document access level (`Viewer`, `Editor`, `Admin`) enforces read-only formula inspection vs edit permissions.

## Events and projections

Transactional domain events (`SpreadsheetCreated`, `SpreadsheetUpdated`, `SpreadsheetSnapshotCreated`) are dispatched via `rustok-outbox`.
When referenced business entities (product price in `rustok-pricing`, inventory stock in `rustok-inventory`) change, corresponding domain events trigger targeted cache invalidation and reactive recalculation in open spreadsheet sessions.

## Failure semantics

- Formula evaluation errors (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NAME?`) are contained strictly within the affected cell and downstream dependents. The document remains healthy and editable.
- Resolver timeouts when evaluating live entity references (`RUSTOK_PRICE`) degrade gracefully to cached values or a `#LOADING!` / `#TIMEOUT!` cell token without aborting workbook computation.

## Migration and cutover

New documents utilize the Loro CRDT spreadsheet block model natively. Existing static tables in `rustok-docs` are retained and can be migrated on demand to spreadsheet blocks via a lossless conversion command. No breaking database migration or schema rewrite is required.

## Alternatives considered

1. **Client-only JavaScript formula parser (Formulas.js / Handsontable):** Rejected due to lack of server-side deterministic evaluation, inability to run headless report exports, and poor performance on 100k+ rows.
2. **PostgreSQL relational tables for cell storage:** Rejected due to high database lock contention during concurrent cell editing and inability to perform offline-first CRDT merging.
3. **HyperFormula:** Evaluated, but rejected due to GPL-3.0 licensing constraints and lack of native Rust integration.

## Verification

1. **Unit Verification (`cargo test -p rustok-docs`):**
   - Loro CRDT tree serialization and deserialization.
   - IronCalc formula evaluation correctness across standard math, text, and date functions.
   - Cycle detection and range bounding.
2. **AI Sandbox Verification:**
   - Proving that invalid AI operations in disposable forks are discarded with zero state mutations in the target document.
3. **UER Integration Verification:**
   - Proving that live price changes in `rustok-pricing` trigger reactive updates in dependent spreadsheet cells.

## Consequences

- Delivers a massive competitive advantage: true Excel-grade spreadsheets embedded natively in collaborative enterprise docs with zero SaaS subscriptions.
- Enables merchants and enterprises to build live quotation sheets, financial models, and inventory planning tools inside RusToK.
- Reuses open-source, battle-tested Rust primitives (Loro CRDT + IronCalc) with zero licensing ambiguity.
