---
id: doc://crates/modules/rustok-messenger/docs/implementation-plan.md
kind: development_plan
language: en
status: active
---

# `rustok-messenger` Implementation Plan

## Scope

`rustok-messenger` owns highload distributed team messaging, direct messages, topic threads, channel hierarchies, read receipts, and optional end-to-end encryption across the RusToK platform.

Consolidated from platform concepts (`docs/architecture/workplace-collaboration-superapp-plan.md`, `docs/concepts/social-modules.md`, `docs/architecture/crm-erp-accounting-matrix-and-plan.md`):
- **Highload Architecture:** Horizontally scalable messaging fabric backed by monotonic sequence cursors (`pts`), append-only immutable message log, in-memory presence ring-buffers, and decoupled ingestion via [`rustok-iggy`](../../rustok-iggy).
- **Relational Integrity & Multilinguality (FBA / i18n):** PostgreSQL persistence via SeaORM (`messenger_conversations`, `messenger_channels_translations`, `messenger_members`, `messenger_messages`, `messenger_cursors`). Channel metadata (title, topic, description) follows the canonical platform i18n pattern (`VARCHAR(32)` normalized locale in `*_translations` table).
- **Universal Entity Reference (UER):** Contextual conversations link to external business aggregates (`entity_type` + `entity_id`: CRM deals, commerce orders, tasks, wiki pages, call rooms) without direct coupling.
- **Interactive Block Messages:** Declarative UI payloads (buttons, form inputs, status cards, quote blocks) serialized via standard JSON.
- **Fast-Sync State Machine:** Offline-first synchronization using monotonic sequence checkpoints (`GET /conversations/{id}/messages?since_seq={last_seen_seq}`) to eliminate gap vulnerabilities.
- **Cryptographic Readiness:** Architectural boundaries for Zero-Knowledge E2EE (`vodozemac` Double Ratchet / Megolm) and Matrix federation gateway (`ruma`).
- **Decoupled Search Integration (CQRS):** Engine-agnostic message indexing via `rustok-search` (supporting Meilisearch, PostgreSQL FTS, Tantivy) fed by durable `rustok-outbox` events (`MessageDispatched`, `MessageEdited`, `MessageDeleted`), with zero-knowledge exclusion for E2EE chats.
- **Fluid Frontend Architecture (FFA) Dual-Surface:**
  - Dedicated Leptos admin package `rustok-messenger-admin` with control plane tabs (Overview, Channels, E2EE, Federation).
  - Dedicated storefront package `rustok-messenger-storefront` providing both a full-page 3-column workplace workstation (folders, active chat, context drawer) and a compact contextual floating widget.

---

## Current state

- Status: `ready`
- FFA status: `dual_surface`
- FBA status: `ready`
- Structural shape: `core_transport_ui`

| Dimension | Current Status | Notes |
| :--- | :--- | :--- |
| **FFA Status** | `dual_surface` | Sovereign module-owned admin package `rustok-messenger-admin` and storefront package `rustok-messenger-storefront` with clean `core/transport/ui` decomposition and Project Fluent catalogs (`en.ftl`, `ru.ftl`). |
| **FBA Status** | `ready` | Sovereign `MessengerService`, typed request context (`PortContext`, `TenantContext`), SeaORM migrations, append-only message log, multilingual translations table, `MessengerPort` boundary. |
| **Structural Shape** | `core_transport_ui` | Sovereign backend domain module with clean physical layout and dual-surface FFA admin + storefront packages. |

---

## Milestones

| Milestone | Scope | Deliverables | Status |
| :--- | :--- | :--- | :--- |
| **MSG-01** | **Domain Core & Invariants** | `Conversation`, `Message`, `Member`, `SyncCursor`, `SnowflakeId`, typed error hierarchy (`MessengerError`) | **Completed** |
| **MSG-02** | **Persistence & Multilingual Migrations** | SeaORM migrations, `messenger_conversations`, `messenger_channels_translations`, `messenger_members`, `messenger_messages`, `messenger_cursors` | **Completed** |
| **MSG-03** | **Application Service & FastSync** | `MessengerService` transactional operations: conversation lifecycle, message dispatch, delta fetch, cursor progression, multilingual channel updates | **Completed** |
| **MSG-04** | **Cross-Boundary Port & Outbox** | `MessengerPort` public interface, `MessageSent` / `MessageRead` domain events for Outbox bus | **Completed** |
| **MSG-05** | **Transport Adapters (REST & WebSockets)** | Axum REST controllers (`/conversations`, `/messages`, `/sync`), WebSocket real-time session hub | **Completed** |
| **MSG-06** | **Iggy Streaming Fabric Integration** | Streaming partitions via `rustok-iggy`, sub-millisecond dispatch and batched worker ingestion | **Completed** |
| **MSG-07** | **Zero-Knowledge E2EE Core** | `vodozemac` Olm Double Ratchet (1-on-1) and Megolm group key ratchets, prekey bundles | **Completed** |
| **MSG-08** | **Decoupled Search Integration** | Multi-backend search seam via `rustok-search` (Meilisearch / PG FTS / Tantivy), membership scoping, and E2EE zero-knowledge exclusion | **Completed** |
| **MSG-09** | **Matrix Gateway Adapter** | Matrix room bridge, event bridge (`m.room.message` / `m.room.encrypted`), `MatrixGatewayAdapter` | **Completed** |
| **MSG-10** | **Admin Control Plane (FFA)** | Leptos admin UI package (`rustok-messenger-admin`), 4-tab dashboard, Fluent i18n, framework-agnostic `core/` view-models | **Completed** |
| **MSG-11** | **Storefront Workplace & Widget UI (FFA)** | Leptos storefront UI package (`rustok-messenger-storefront`), 3-column workstation page, contextual floating chat widget, optimistic state machine, Fluent i18n, Dual-path GraphQL bridge | **Completed** |

---

## Verification

1. **Local Scoped Tests:** Pure Rust unit tests for domain invariants, Snowflake ordering, E2EE prekey pools, Matrix projections, and FFA view-models (`cargo test -p rustok-messenger -p rustok-messenger-admin -p rustok-messenger-storefront`).
2. **Multilingual Integrity:** Validation of `messenger_channels_translations` locale resolution and fallback, plus Project Fluent `.ftl` catalog loading tests.
3. **Module Manifest & Composition:** `cargo xtask module validate messenger` and `cargo xtask validate-manifest`.
4. **Architecture Guard:** `python scripts/architecture_dependency_guard.py` verifying layer/dependency separation.
5. **Static Code Analysis:** `cargo clippy -p rustok-messenger -p rustok-messenger-admin` with zero warnings.

---

## Update rules

- Retain synchronization between domain services and documented contracts.
- Any architectural contract change requires an ADR before implementation.
