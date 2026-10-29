# rustok-calls-storefront / CRATE_API

## Public surface

### Entry Point & Components
- `CallsView`: mounted Leptos storefront component rendering customer-facing video/audio calls.
- `CallsViewModel`: view model representing call participant state and room details.
- `CallsStorefrontTransport`: transport facade managing backend communication.

## Platform Capability Bindings
- **Storefront Route Integration**: Registers `CallsView` for public conferencing and customer support calls.
- **Fluid Frontend Architecture**: Strictly isolates Leptos rendering from framework-agnostic models in `core`.
- **Media & Signaling Presentation**: Manages media device toggles, connection quality badges, and signaling state.

## Domain Collaborations
- **`rustok-calls`**: Core calls module providing WebRTC room coordination and peer signaling.
- **`rustok-ui-core`**: Core UI layout and accessibility foundations.
- **`rustok-ui-i18n`**: Fluent translation catalogs (`en.ftl`, `ru.ftl`) synchronized with host locale context.

## Minimum Contract Set

### Input DTOs/Commands
- `CallsViewModel`: constructor parameter (`title`).
- `CallsStorefrontTransport`: constructor creating transport facade.

### Domain Invariants
- **Non-Swallowed Errors**: Peer connection drops or signaling failures must display feedback to callers.
- **Graceful Media Fallback**: Device permission rejections degrade gracefully without crashing the view.
- **Session Privacy**: Call identifiers and SDP/ICE payloads are transmitted strictly over secure channels.

### Events / Outbox Side Effects
- Dispatches peer join/leave notifications through the backend transport facade.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `CallsTransportError`: network and signaling failure codes for storefront call operations.

