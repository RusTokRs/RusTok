# Messenger architecture and integration

`rustok-messenger` is the owner of Messenger conversations, memberships, messages, localized channel metadata and cursors. Its PostgreSQL schema currently comprises `messenger_conversations`, `messenger_channels_translations`, `messenger_members`, `messenger_messages` and `messenger_cursors`. Channel metadata follows the normalized locale contract. The message body retains the author's language.

## Current source boundary

- `MessengerService` implements the owner operations. `MessengerPort` exposes cross-module operations. Consumers use owner APIs and entity references and keep business state in the owning module.
- The manifest provides Axum REST and WebSocket adapters, GraphQL, and Leptos admin and storefront packages. The storefront declares the `messages` route and a floating widget.
- Message creation writes a reference to the transactional outbox. The Messenger Notifications provider resolves that source. Realtime broadcasts currently use adapter-local calls to an in-memory hub.
- The existing FastSync route reads message rows by ID. In-place edits and deletes do not advance that position. Check the implementation plan before using the endpoint for offline reconciliation.

## Target and release gates

The [proposed write and sync ADR](../../../../DECISIONS/2026-11-11-messenger-write-and-sync.md) defines the owner transaction, replay journal and deployment boundaries under review. The [implementation plan](implementation-plan.md) sequences privacy fixes, data and sync work, transport composition, web parity, and separately qualified optional profiles. Use the [crate API](../CRATE_API.md) for current source entry points and the [Workplace proposal](../../../../docs/architecture/workplace-collaboration-superapp-plan.md) for the broader product context.
