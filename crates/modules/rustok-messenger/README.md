# `rustok-messenger`

Highload distributed team messaging, channels, direct messages, topic threads, and E2EE engine for RusToK.

## Purpose

`rustok-messenger` provides high-throughput real-time team messaging, channels, direct communications, and entity-attached chat threads across the RusToK platform. It serves as the single sovereign owner for conversation lifecycles, message delivery ordering, member sync cursors, and multilingual channel metadata.

## Responsibilities

- **Monotonic Highload Messaging:** Uses Snowflake 64-bit monotonically increasing identifiers and per-conversation sequence clocks (`pts`) for linearizability and gap-free sync.
- **Multilingual Channels:** Channel titles and descriptions are localized via dedicated translation projections (`messenger_channels_translations`) complying with the platform i18n contract.
- **Fast-Sync State Machine:** Enables instant offline client reconciliation using monotonic `pts` delta ranges.
- **Universal Entity Reference (UER):** Contextual conversations attachable to orders, CRM deals, blog posts, or support tickets.
- **E2EE Ready:** Message schema supports Zero-Knowledge Double Ratchet payload encryption (`vodozemac`).

## Entry Points

- `MessengerModule`: Platform module registration implementing `RusToKModule` and `MigrationSource`.
- `MessengerService`: Transactional application service managing conversations, messages, membership, and sync cursors.
- `MessengerPort`: Cross-module port for other modules to create conversations and post notifications.
- `router`: Axum HTTP REST and WebSocket controller routing.

## Interactions

- **`rustok-api`:** Supplies permission constants (`MESSAGES_READ`, `CONVERSATIONS_WRITE`, etc.) and canonical contexts.
- **`rustok-core`:** Provides module extension contracts, `RusToKModule`, and migration traits.
- **`rustok-outbox`:** Durable transactional event publishing for cross-module notifications and WebSocket push broadcasts.

## Features

- **Highload Architecture:** Append-only message log with monotonic sequence cursors (`pts`), Snowflake 64-bit IDs, and decoupled ingestion.
- **Multilingual Channels:** Localized channel titles and descriptions conforming to the platform translation contract.
- **Fast-Sync State Machine:** Gap-free offline reconciliation for mobile and web clients.
- **Universal Entity Reference:** Direct contextual conversations attached to CRM deals, orders, blog posts, and calendar events.
- **E2EE Ready:** Data contracts support Zero-Knowledge Double Ratchet payload encryption (`vodozemac`).

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
