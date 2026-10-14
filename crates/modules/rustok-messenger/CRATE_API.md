# `rustok-messenger` Public Crate API

This document describes the public Rust interface exported by `rustok-messenger`.

## Module Facade

```rust
use rustok_messenger::{
    MessengerModule,
    MessengerService,
    MessengerPort,
    MessengerError,
    MessengerResult,
};
```

## Core Public Types

### Domain & DTOs
- `Conversation`: Sovereign conversation aggregate (Direct, Group, Channel, EntityThread).
- `Message`: Append-only message with monotonic sequence ID (Snowflake).
- `Member`: Conversation membership and role (Owner, Admin, Member, Guest).
- `SyncCursor`: User read position and sequence tracking.
- `CreateConversationInput`: Input payload for creating conversations and channels.
- `SendMessageInput`: Input payload for sending messages with optional interactive blocks.
- `FastSyncRequest` / `FastSyncResponse`: Delta query contracts for offline reconciliation.

### Services & Ports
- `MessengerService`: Transactional service implementing all messaging logic.
- `MessengerPort`: Cross-module port trait for publishing notifications, transactional messages, and CRM alerts.

### Transports & GraphQL
- `axum_router`: Host-bound Axum HTTP router mounting `/api/messenger/*` REST, WebSocket, and E2EE key endpoints.
- `MessengerQuery`: GraphQL queries (`messengerOverview`, `messengerChannels`, `messengerE2eeDevices`).
- `MessengerMutation`: GraphQL mutations (`createMessengerChannel`, `uploadDeviceKeys`, `claimPrekeyBundle`).
- `attach_schema_data`: Schema runtime data factory binding `DeviceKeyRegistry` and `MessengerRoomHub`.

## Errors
- `MessengerError`: Structured domain error with conversion to `rustok_web::HttpError` and `rustok_api::PortError`.

