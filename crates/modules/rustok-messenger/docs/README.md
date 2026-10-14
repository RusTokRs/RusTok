# `rustok-messenger` Module Architecture & Integration Guide

`rustok-messenger` is the sovereign real-time communication fabric of the RusToK platform.

## Purpose

- Provide highload, distributed team messaging, channels, direct messages, topic threads, and reactions.
- Guarantee zero database lock contention using append-only message storage and monotonic Snowflake sequence numbers (`pts`).
- Comply with the RusToK Multilinguality (i18n) platform standard: channel titles and descriptions are localized via `messenger_channels_translations` (`VARCHAR(32)` locale), while message streams remain language-agnostic.
- Support Universal Entity Reference (UER) embeddings to link conversations to CRM deals, ecommerce orders, blog comments, tasks, and calendar events.
- Provide cryptographic foundations for Zero-Knowledge End-to-End Encryption (E2EE via `vodozemac`) and Matrix protocol federation (`ruma`).

## Responsibility Zone

- Sovereign `MessengerService`: conversation management, message delivery, cursor tracking, and delta replay.
- Persistence and migrations: `messenger_conversations`, `messenger_channels_translations`, `messenger_members`, `messenger_messages`, `messenger_cursors`.
- Cross-boundary port `MessengerPort` for system notifications, CRM alerts, and operational event publishing.
- FastSync reconnection state machine for mobile and web clients (`/conversations/{id}/sync?since_seq={pts}`).

## Core Capabilities

1. **Monotonic Sequence Cursor & Append-Only Log:**
   * Every message receives a 64-bit monotonically increasing sequence ID (Snowflake).
   * Edits and deletions append delta events rather than updating records in place.
2. **Multilingual Channels:**
   * Public/private channels have localized names and topics in `messenger_channels_translations`.
   * Follows the platform pattern: base table holds structural properties, translation table holds locale-specific metadata.
3. **Universal Entity Reference (UER):**
   * Conversations attach to any business entity via `entity_type` + `entity_id` (e.g., deal negotiations, order support tickets).
4. **Interactive Block Messages:**
   * Declarative UI payloads (buttons, cards, forms) serialized via JSON.
5. **Fast-Sync State Machine:**
   * Gap-free delta sync for disconnected or mobile clients resuming on flaky networks.

## Integration

- Consumes `rustok-api` and `rustok-core` for platform ports, context, and error propagation.
- Exposes `MessengerPort` for CRM deals, ecommerce checkout alerts, and workflow triggers.
- Publishes transactional events (`MessageSent`, `MessageRead`, `ConversationCreated`) to the Outbox bus.
- Dual-path transport readiness (Axum REST / WebSockets + future GraphQL).

## Verification

- Pure Rust unit tests: `cargo test -p rustok-messenger`.
- Manifest validation: `cargo xtask validate-manifest`.
- Publish readiness contract: `cargo xtask module validate messenger`.
- Sequence monotonicity: Snowflake generator test verifying strict monotonic ordering.

## Related Documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Workplace Collaboration SuperApp Plan](../../../../docs/architecture/workplace-collaboration-superapp-plan.md)
