---
id: doc://crates/modules/rustok-docs/docs/implementation-plan.md
kind: development_plan
language: en
status: active
---

# `rustok-docs` Implementation Plan

## Scope

`rustok-docs` owns collaborative documents, wikis, page trees, and Notion-grade knowledge bases across RusToK.

- **Real-Time Multiplayer Core:** Loro CRDT (`CollaborativeDoc`, `MovableTree`, `LoroText`).
- **Data Persistence:** PostgreSQL (`docs_pages`, `docs_snapshots`, `docs_updates`, `docs_entity_links`).
- **Database Engine:** Schema powered by `flex` (`FieldDefinition`), views rendered by `rustok-grid` (Table, Kanban, Calendar).
- **Transport:** Dual-path (WebSocket binary updates for real-time + GraphQL queries/mutations for CRUD).

---

## Current state

| Dimension | Current Status | Notes |
| :--- | :--- | :--- |
| **FFA Status** | `in_progress` | Framework-agnostic pure domain core (`domain/`), block models, Markdown projection, zero UI imports in domain. |
| **FBA Status** | `boundary_ready` | Sovereign `DocumentService`, DTO contracts (`dto/`), SeaORM migrations, monotonic delta log, periodic snapshot compaction, UER backlinks ledger. |
| **Structural Shape** | `core_transport_ui` | Manifest and layout aligned with target native module architecture. |

---

## Milestones

| Milestone | Scope | Key Deliverables | Status |
| :--- | :--- | :--- | :--- |
| **DOCS-01** | **Domain Core & Loro CRDT** | `CollaborativeDoc`, `DocBlock`, `BlockType`, tree operations, delta sync, Markdown projections | **Completed** |
| **DOCS-02** | **Persistence & Snapshots** | SeaORM migrations, `docs_pages`, `docs_snapshots`, append-only `docs_updates` log, `DocumentService`, snapshot compaction | **Completed** |
| **DOCS-03** | **Real-Time Sync Transport & HTTP Controllers** | WebSocket broadcaster (`DocsRoomHub`, session rooms, binary deltas, presence/awareness) + Axum REST controllers (`pages`, `tree`, `sync`, `deltas`), tenant admission guard & RBAC | **Completed** |
| **DOCS-04** | **Flex & Grid Integration** | Notion-like database views (Table, Kanban) over structured document properties | In Progress |
| **DOCS-05** | **Universal Entity Embedding & Backlinks** | UER (`EntityRef`), 4 projection modes, live embedding of Media, Blog, Forum, Wiki/Pages, and ERP entities, `docs_entity_links` reverse backlinks ledger | Planned |
| **DOCS-06** | **Client Adapters** | `@rustok/docs-editor` (`loro-wasm`) and Flutter FFI (`rustok_mobile`) | Planned |

---

## Verification

1. Pure Rust unit tests for `CollaborativeDoc` with concurrent synthetic writer convergence (`cargo test -p rustok-docs`).
2. Cycle-safety test for drag-and-drop block reorganization (`move_block`).
3. Module metadata, permissions, and migration descriptors verification (`DocsModule`).
4. SeaORM snapshot & delta replay fidelity tests on PostgreSQL.

---

## Update rules

- Update this implementation plan when milestone statuses change.
- Retain synchronization between domain services and documented contracts.
- Any architectural contract change requires an ADR before implementation.
