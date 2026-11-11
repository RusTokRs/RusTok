# `rustok-messenger`

RusToK's owner module for team messaging, direct messages, channels, and conversation history.

## Purpose

`rustok-messenger` owns conversation lifecycles, membership, message state, member cursors, and multilingual channel metadata. The source contains a PostgreSQL-backed application service, API adapters and Leptos packages. See the [implementation plan](docs/implementation-plan.md) for the gaps between the current scaffold and the first private messaging release.

## Responsibilities

- Conversations, membership, message history and read markers in the Messenger schema.
- Localized channel titles and descriptions in `messenger_channels_translations`.
- Owner APIs and cross-module operations through `MessengerPort`.

## Entry Points

- `MessengerModule`: Platform module registration implementing `RusToKModule` and `MigrationSource`.
- `MessengerService`: Transactional application service managing conversations, messages, membership, and sync cursors.
- `MessengerPort`: Cross-module entry point for system messages, direct messages, contextual conversations and unread counts.
- `router`: Axum HTTP REST and WebSocket controller routing.

## Interactions

- **`rustok-api`:** Supplies permission constants (`MESSAGES_READ`, `CONVERSATIONS_WRITE`, etc.) and canonical contexts.
- **`rustok-core`:** Provides module extension contracts, `RusToKModule`, and migration traits.
- **`rustok-outbox`:** Transactional publication of message creation references. Notifications uses its Messenger source provider to resolve those references.

## Readiness

The current service uses Snowflake IDs and in-place message edits and deletes. Its existing FastSync endpoint does not replay those changes. The process-local room hub, encrypted envelope and device registry, Iggy dispatcher and Matrix adapter require the release evidence described in the owner plan. The proposed [write and sync ADR](../../../DECISIONS/2026-11-11-messenger-write-and-sync.md) defines the target under review.

## Module Layout

Following the canonical platform FBA physical layout:
```text
crates/modules/rustok-messenger/
  Cargo.toml
  README.md
  CRATE_API.md
  rustok-module.toml
  contracts/
  docs/
    README.md
    implementation-plan.md
  src/
    lib.rs              # Crate facade, exports
    module.rs           # RusToKModule implementation
    error.rs            # Typed owner errors and HTTP mappings
    domain/             # Pure domain entities, value objects, invariants
    dto/                # Request/response contracts
    entities/           # SeaORM entity mappings
    migrations/         # SeaORM database migrations
    services/           # MessengerService transactional application logic
    ports/              # MessengerPort cross-module port
    controllers/        # Axum REST endpoints and WebSocket handlers
```

## Documentation

- [CRATE_API.md](CRATE_API.md) — Public API contract, module registration, capability bindings, and domain invariants.
- [Architecture & Integration Guide](docs/README.md)
- [Implementation Plan & Milestones](docs/implementation-plan.md)
