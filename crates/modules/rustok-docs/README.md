# rustok-docs

## Purpose

`rustok-docs` owns collaborative documents, corporate wiki, knowledge base, and Notion-like structured databases powered by Loro CRDT for RusToK.

## Responsibilities

- **Block-Tree Data Model & Loro CRDT:** Own the hierarchical block tree data model with high-performance, conflict-free collaborative editing (Loro CRDT) across concurrent human users and AI agents.
- **Notion-Like Structured Databases:** Provide 5 polymorphic view projections (Table, Kanban, Calendar, Gallery, List) backed by [`flex`](../../modules/flex) typed column definitions (`FieldDefinition`) and [`rustok-grid`](../../../ui/rustok-grid).
- **Universal Entity Reference Protocol (UER / `EntityRef`):** Define the canonical reference standard `urn:rustok:<tenant>:<module>:<entity_type>:<id>` and publish `EntityCardResolverPort` for live embedding of any platform entity across 50+ RusToK modules.
- **Polymorphic Display Modes:** Support 4 projection views for any embedded entity:
  1. Inline Mention / Pill (`@Topic`, `@Blog`, `@Product`, `@Page`);
  2. Compact Preview Card (Author, excerpt, thumbnail, metrics);
  3. Interactive Live Widget (Media player, forum thread preview with reply composer, live price & inventory, CRM deal stage switcher);
  4. Database Relation & Rollup (Cross-table foreign relations and computed property rollups).
- **Media Reference Governance:** Implement `DocsMediaReferencePolicy` to retain active document media references in [`rustok-media`](../../modules/rustok-media) (`MediaAssetWritePort::retain_asset_reference`), preventing premature garbage collection and serving private assets via signed streaming tokens.
- **Bidirectional Backlinks Ledger (`docs_entity_links`):** Maintain an append-only relational ledger of entity references, feeding reverse backlinks into target module UIs and powering the `rustok-mcp` AI knowledge graph.
- **RBAC Security Delegation & Redaction Boundary:** Enforce strict tenant isolation and canonical `SecurityContext` propagation; unauthorized viewers in shared documents receive a masked placeholder without data leakage.

## Interactions

- Depends on `loro` for native Rust CRDT document synchronization.
- Consumes [`rustok-media`](../../modules/rustok-media) via `MediaAssetReadPort` / `MediaAssetWritePort`.
- Consumes [`rustok-content-orchestration`](../../modules/rustok-content-orchestration) for cross-domain conversion workflows: `promote_topic_to_doc`, `publish_doc_to_blog`, `transclude_wiki_to_doc`.
- Reuses [`flex`](../../modules/flex) for typed database column schemas.
- Reuses [`rustok-grid`](../../../ui/rustok-grid) for UI grid, kanban, and list views.
- Composes with [`rustok-rbac`](../../modules/rustok-rbac) for workspace, page, and block permissions.
- Exposes graph relationships to `rustok-mcp` for agentic retrieval and question answering.

## Entry Points

- `DocBlock` / `BlockType`
- `EntityRef` / `EntityModule` / `EntityViewMode`
- `EntityCardResolverPort`
- `DocsMediaReferencePolicy`
- `docs_entity_links` persistence schema

## Documentation

- [Workplace & Collaboration SuperApp Plan](../../../docs/architecture/workplace-collaboration-superapp-plan.md)
- [Module docs](./docs/README.md)
