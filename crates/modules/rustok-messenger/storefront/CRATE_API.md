# rustok-messenger-storefront / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA storefront UI components for collaboration workstations and floating chat widgets.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-messenger` for conversation threads, messages, and read horizon updates.
- Collaborates with `apps/storefront` as a registered storefront module package.

## Public surface
- `pub mod core`: Pure Rust view-models, filters, message grouping, and state models (`ConversationFilter`, `DeliveryStatus`, `ConversationItemViewModel`, `ChatMessageViewModel`, `MessageDateGroup`, `MessengerWorkspaceState`).
- `pub mod model`: Transport DTOs (`ConversationItemPayload`, `MessageItemPayload`, `SendMessagePayload`, `UpdateCursorPayload`).
- `pub mod transport`: Remote endpoints and facade (`fetch_conversations`, `fetch_messages`, `send_message`, `update_cursor`).
- `pub use ui::{MessengerFloatingWidget, MessengerView, MessengerWorkstation}`

## Minimum Contract Set

### Input DTOs/Commands
- `SendMessagePayload`: Outbound message DTO with conversation UUID, content string, optional reply ID, and client correlation ID.
- `UpdateCursorPayload`: Read cursor advance command carrying conversation ID and last read message ID.
- `ConversationItemPayload`: Listed conversation summary.
- `MessageItemPayload`: Individual chat message details.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Message validation: message content is validated for non-empty string and length limits.
- Date grouping ensures chronological order of messages.

### Events / Outbox Side Effects
- Storefront UI adapter; message mutations delegate event publishing to `rustok-messenger`.

### Errors / Failure Codes
- Network, auth, and validation errors are surfaced through typed `ApiError` variants.
