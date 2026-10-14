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

---

## Current state

- Status: `in_progress`
- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`

| Dimension | Current Status | Notes |
| :--- | :--- | :--- |
| **FFA Status** | `not_started` | No module-owned UI package in initial phase; Flutter mobile client (`rustok_mobile`) and Leptos/Next adapters planned. |
| **FBA Status** | `boundary_ready` | Sovereign `MessengerService`, typed request context (`PortContext`, `TenantContext`), SeaORM migrations, append-only message log, multilingual translations table, `MessengerPort` boundary. |
| **Structural Shape** | `no_ui_boundary` | Sovereign backend domain module with clean physical layout and no direct host UI coupling. |

---

## Milestones

| Milestone | Scope | Deliverables | Status |
| :--- | :--- | :--- | :--- |
| **MSG-01** | **Domain Core & Invariants** | `Conversation`, `Message`, `Member`, `SyncCursor`, `SnowflakeId`, typed error hierarchy (`MessengerError`) | **Completed** |
| **MSG-02** | **Persistence & Multilingual Migrations** | SeaORM migrations, `messenger_conversations`, `messenger_channels_translations`, `messenger_members`, `messenger_messages`, `messenger_cursors` | **Completed** |
| **MSG-03** | **Application Service & FastSync** | `MessengerService` transactional operations: conversation lifecycle, message dispatch, delta fetch, cursor progression, multilingual channel updates | **Completed** |
| **MSG-04** | **Cross-Boundary Port & Outbox** | `MessengerPort` public interface, `MessageSent` / `MessageRead` domain events for Outbox bus | **Completed** |
| **MSG-05** | **Transport Adapters (REST & WebSockets)** | Axum REST controllers (`/conversations`, `/messages`, `/sync`), WebSocket real-time session hub | **Completed** |
| **MSG-06** | **Iggy Streaming Fabric Integration** | Streaming partitions via `rustok-iggy`, sub-millisecond dispatch and batched worker ingestion | Planned |
| **MSG-07** | **Zero-Knowledge E2EE Core** | `vodozemac` Olm Double Ratchet (1-on-1) and Megolm group key ratchets | Planned |
| **MSG-08** | **Full-Text Search Engine** | Pure-Rust `Tantivy` indexing pipeline for searchable conversation transcripts | Planned |
| **MSG-09** | **Matrix Gateway Adapter** | `ruma`-based federation bridge (`rustok-matrix-gateway`) for external Element/Matrix interoperability | Planned |
| **MSG-10** | **Client & UI Packages** | Flutter UI package (`rustok_mobile`) and Leptos/Next web widget adapters | Planned |

---

## Verification

1. **Local Scoped Tests:** Pure Rust unit tests for `SnowflakeGenerator`, `Conversation`, `Message`, and `SyncCursor` state invariants (`cargo test -p rustok-messenger`).
2. **Multilingual Integrity:** Validation of `messenger_channels_translations` locale resolution and fallback.
3. **Module Manifest & Composition:** `cargo xtask module validate messenger` and verification of `MessengerModule` metadata.
4. **Sequence Ordering:** Monotonicity test ensuring subsequent messages always yield strictly increasing sequence numbers without gaps.

---

## Update rules

- Update this implementation plan when milestone statuses change.
- Retain synchronization between domain services and documented contracts.
- Any architectural contract change requires an ADR before implementation.
