# `rustok-docs` Module Architecture & Integration Guide

`rustok-docs` is the sovereign collaborative knowledge base, wiki, and interactive document module of the RusToK SuperApp platform.

## Purpose

- Provide the sovereign collaborative knowledge base, document, and wiki engine for RusToK.
- Support conflict-free real-time multiplayer editing powered by Loro CRDT.
- Maintain movable block-tree structures, Notion-grade flex database collections, and embedded ERP business cards.
- Expose dual-path transport surfaces (WebSocket binary deltas and GraphQL/REST query and mutation APIs).

## Responsibility Zone

- Sovereign `DocumentService`, `CollaborativeDoc`, tree block models, and Markdown projections.
- Persistence and compaction for `docs_pages`, `docs_snapshots`, append-only `docs_updates`, and `docs_entity_links`.
- Real-time collaboration rooms via `DocsRoomHub`, session presence, and binary CRDT delta exchange.
- Tenant isolation, document access policies, and version milestones.

## Core Capabilities

1. **Conflict-Free Multiplayer Editing (Loro CRDT):**
   * Uses `CollaborativeDoc` powered by [Loro](https://loro.dev) in Rust.
   * Concurrent edits to paragraphs, lists, and headings converge deterministically across multiple authors and AI agents without merge conflicts.
2. **Movable Block-Tree Structure:**
   * Every page is a tree of atomic `DocBlock` elements with unique `TreeID` identifiers.
   * Drag-and-drop structural reorganization is backed by Loro's cycle-safe `MovableTree` algorithm.
3. **Database Views (Flex + Grid):**
   * Notion-grade database collections reuse [`crates/modules/flex`](../../flex) for schema/field definitions (Status, Tags, Dates, Assignee, Select).
   * Visual layouts are powered by [`crates/ui/rustok-grid`](../../../ui/rustok-grid) polymorphic views: Table, Kanban, Calendar, Gallery, and List.
4. **Live ERP Business Blocks:**
   * Embeds live operational cards directly within documents:
     - `/product`: Live catalog cards and specifications from `rustok-product`.
     - `/inventory`: Real-time stock counts from `rustok-inventory`.
     - `/invoice`: Interactive billing status and PDFs from `rustok-billing`.
     - `/deal`: Live CRM pipeline status from `rustok-deal`.
5. **Two-Tier Versioning:**
   * **Real-Time Tier (Loro CRDT):** Sub-millisecond keystrokes, remote cursors, and delta sync.
   * **Business Milestone Tier (`rustok-revisions`):** Named release versions ("Release v2.0", "Approved by Legal"), audit trails, and retention cleanup policies.

## Integration

- Consumes `rustok-api` and `rustok-core` for platform ports, context, and error propagation.
- Integrates with `rustok-flex` for database view field definitions and schemas.
- Exposes UI primitives through `rustok-grid` for tabular and card view layouts.
- Re-exports sovereign entrypoints for host applications via `DocsModule`.

## Verification

- Pure Rust unit tests: `cargo test -p rustok-docs`.
- Module validation gate: `cargo xtask module validate docs`.
- SeaORM snapshot and delta replay tests on PostgreSQL.

## Related Documentation

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Module Registry](../../../../docs/modules/registry.md)
