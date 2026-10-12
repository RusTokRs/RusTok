# `rustok-docs`

Sovereign collaborative documents, wiki, and knowledge base module for the RusToK platform, powered by **Loro CRDT**.

## Purpose

`rustok-docs` provides real-time collaborative document editing, hierarchical wiki navigation, and Notion-grade knowledge management across the RusToK platform. It serves as the single source of truth for rich content collaboration, combining conflict-free replication (CRDT) with relational persistence and Universal Entity Reference (UER) embeddings.

## Responsibilities

- **Conflict-Free Real-Time Editing:** Uses Loro CRDT (`LoroDoc`, `LoroText`) for deterministic merging of concurrent edits across web, desktop, and mobile clients.
- **Cycle-Safe Block Hierarchy:** Leverages Loro `MovableTree` to organize blocks hierarchically without cycles or split-brain states during concurrent drag-and-drop reorganization.
- **Monotonic Persistence & Compaction:** Stores pages in PostgreSQL (`docs_pages`), persists append-only binary deltas (`docs_updates`), and periodically compacts deltas into full binary snapshots (`docs_snapshots`).
- **Universal Entity Reference (UER) Backlinks:** Index cross-module embedded entities (`docs_entity_links`) for bidirectional graph navigation across Commerce, Blog, Media, Forum, and Flex.
- **Deterministic Projections:** Generates Markdown projections for SEO, search indexing, static documentation generation, and AI agent retrieval.

## Entry Points

- `DocsModule`: Platform module registration implementing `RusToKModule` and `MigrationSource`.
- `DocumentService`: Sovereign application service managing page lifecycles, document loading, delta append, snapshot compaction, and backlink sync.
- `CollaborativeDoc`: Pure Rust CRDT document engine wrapping `LoroDoc`.
- `DocBlock`, `BlockType`, `EntityRef`: Domain models defining block structures and cross-module embeddings.

## Interactions

- **`flex`:** Supplies typed schema attributes and custom properties for database pages.
- **`rustok-grid`:** Renders multi-view presentations (Table, Kanban, Calendar, Gallery) over document collections.
- **`rustok-api`:** Supplies canonical permission constants (`Permission::PAGES_*`) and tenant authority.
- **`rustok-core`:** Provides module extension contracts and migration dependency tracking.
