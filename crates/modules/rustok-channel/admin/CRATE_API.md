# Crate API: rustok-channel-admin

## Public surface

### Core
- `ChannelAdminFormState`: State management for channel administration forms.
- `ChannelAdminLabels`: Internationalized labels for channel operations.

### Models
- `ChannelAdminModel`: Channel representation with target and module associations.
- `ChannelTargetModel`: Deployment target configuration.

### Transport
- `ChannelAdminTransport`: Facade for server functions and channel administration endpoints.

### UI Components
- `ChannelAdmin`: Root Leptos administration component for channel management.

## Platform Capability Bindings
- Fluid Frontend Architecture (FFA) administration package for `rustok-channel`.
- Compatible with Leptos hydration and SSR modes via `rustok-ui-core` and `rustok-ui-i18n`.
- Uses native server functions through `HostRuntimeContext`.

## Domain Collaborations
- Collaborates with `rustok-channel` domain service for channel routing and target mapping.
- Interacts with host authorization context for operator permissions.

## Minimum Contract Set

### Input DTOs/Commands
- `ChannelAdmin`: View component mounting the channel administration workspace.
- Channel creation and target attachment commands dispatched via transport.

### Domain Invariants
- Channels must maintain unique slug identifiers within tenant boundaries.
- Target bindings and module associations validate configuration before mutation.
- Zero business logic in UI view layer; state mapping stays in `core`.

### Events / Outbox Side Effects
- Mutates channel routes, target deployments, and module bindings via channel service.

### Errors / Failure Codes
- Validation failures propagate with descriptive user feedback.
- Network and server function errors handled gracefully without UI panics.

