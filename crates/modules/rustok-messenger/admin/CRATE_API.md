# rustok-messenger-admin / CRATE_API

## Platform Capability Bindings
- Implements Leptos FFA UI components for messenger administration and E2EE observability.
- Integrates with `rustok-ui-core`, `rustok-ui-transport`, and `rustok-ui-i18n`.

## Domain Collaborations
- Collaborates with `rustok-messenger` module backend for channels, federated rooms, and E2EE device audits.
- Collaborates with `apps/admin` as a registered admin module package.

## Public surface
- `pub mod core`: Pure Rust view-models, validation, and tab state (`MessengerAdminTab`, `ChannelFormDraft`).
- `pub mod model`: Transport-neutral DTOs (`MessengerOverviewPayload`, `ChannelListItemPayload`, `CreateChannelPayload`, `E2eeDeviceAuditPayload`).
- `pub mod transport`: Transport facade (`fetch_overview`, `fetch_channels`, `fetch_e2ee_devices`).
- `pub use ui::MessengerAdmin`: Root admin view component.

## Minimum Contract Set

### Input DTOs/Commands
- `MessengerOverviewPayload`: Overview counts for conversations, topics, devices, and federation.
- `ChannelListItemPayload`: Listed conversation channel metadata.
- `CreateChannelPayload`: Channel creation parameters.
- `E2eeDeviceAuditPayload`: Cryptographic keys and one-time key audit metrics.

### Domain Invariants
- FFA boundary separation: `core/` contains no UI framework types.
- Navigation tabs support safe fallback to Overview when unknown string is provided.
- Channel title must not be empty upon creation submission.

### Events / Outbox Side Effects
- UI adapter package; channel creations delegate command execution and event publishing to `rustok-messenger`.

### Errors / Failure Codes
- Transport and validation errors are surfaced through typed `ApiError` variants.
