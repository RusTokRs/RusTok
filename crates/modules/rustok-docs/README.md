# `rustok-docs`

Sovereign collaborative documents, wiki, and knowledge base module for the RusToK platform, powered by **Loro CRDT**.

## Architecture & Responsibilities

- **Zero Merge Conflicts (Loro CRDT):** Implements real-time collaborative editing using the high-performance Rust CRDT library [Loro](https://loro.dev). Concurrent text edits, formatting, and structural block movements merge deterministically across web, desktop, and mobile clients.
- **Hierarchical Block Tree (`MovableTree`):** Every document is represented as an atomic tree of movable blocks (Paragraphs, Headings, Lists, Quotes, Tables, Callouts, and Live ERP Cards). Loro's cycle-safe movable tree algorithm prevents split-brain documents and broken hierarchies during concurrent drag-and-drop operations.
- **Offline-First & Cross-Platform:**
  - **Backend:** Native Rust core compiled in `apps/server` with microsecond delta-log processing and snapshot persistence.
  - **Browser:** Same Rust core compiled via `loro-wasm` for Next.js and Leptos web interfaces.
  - **Mobile:** Direct FFI integration in `rustok_mobile` (Flutter) enabling zero-latency offline authoring with automatic catch-up on reconnection.
- **Markdown Projections:** Deterministically generates clean Markdown read models for SEO, search indexing, static documentation sites, and AI agent retrieval.
- **Cooperation with RusToK Ecosystem:**
  - **`flex`:** Powers typed database properties (Status, Tags, Assignee, Dates).
  - **`rustok-grid`:** Renders multi-view databases (Table, Kanban, Calendar, Gallery).
  - **`rustok-revisions`:** Manages legal versions, release milestones, and retention policies on top of real-time CRDT sessions.
