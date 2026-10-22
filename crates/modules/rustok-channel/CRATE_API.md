# rustok-channel / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `invalidation_generation`, `migrations`, `module`, `policy`, `ports`, `resolution`, `services`, `target_type`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `ChannelModule` implementing `RusToKModule` (slug `"channel"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Runtime Selected Marker**: `ChannelRuntimeSelected` proving channel module participation in runtime extension registration.
- **Channel Service & Resolution**:
  - `ChannelService` managing tenant-scoped channels, targets, module bindings, and OAuth apps.
  - `ChannelResolver`, `RequestFacts`, `ResolutionDecision`, `ResolutionOutcome` evaluating request facts against resolution rules.
  - `StoredChannelResolutionRule`, `ChannelResolutionRuleDefinition`, `ResolutionAction`, `ResolutionPredicate`.
- **Ports & Transports**:
  - `ChannelReadPort` (`read_channel`, `list_channels_for_tenant`)
  - `in_process_channel_read_port` constructing direct in-process provider adapters.
  - `ChannelReadSelector` (`Id`, `Slug`, `HostTargetValue`, `Default`).
- **Cache Invalidation & Generation**:
  - `read_resolution_invalidation_generation` and `CHANNEL_RESOLUTION_INVALIDATION_SCOPE`.

## Domain Collaborations
- **Core Platform**: Integrates with `rustok-core` module system and `rustok-api` port context, call policies, and error types.
- **Storefront & Delivery Surfaces**: Delivers authoritative channel scoping and routing decisions to frontends and upstream service workers.

## Primary Public Types and Signatures
- `pub struct ChannelModule`
- `pub struct ChannelRuntimeSelected`
- `pub struct ChannelService`
- `pub trait ChannelReadPort`
- `pub struct ChannelResolver`
- `pub enum ChannelReadSelector`
- `pub struct ChannelReadRequest`
- `pub struct ChannelListRequest`
- `pub struct ChannelReadProjection`
- `pub struct ChannelResponse`
- `pub struct ChannelDetailResponse`
- `pub enum ChannelError`

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`

## Common AI Mistakes
- Using wildcard re-exports instead of explicit named imports.
- Hardcoding default channels instead of evaluating tenant resolution rules or querying `ChannelReadPort`.
- Bypassing tenant validation on channel target operations.
- Ignoring inactive channel visibility rules (`include_inactive`).

## Minimum Contract Set

### Input DTOs/Commands
- `CreateChannelInput` / `CreateChannelRequest` (`name`, `slug`, `default_locale`, `currency`, `is_default`, `is_active`)
- `CreateChannelTargetInput` (`target_type`, `target_value`, `is_primary`)
- `ChannelReadRequest` (`selector`, `include_inactive`)
- `ChannelListRequest` (`page`, `per_page`, `include_inactive`)
- `RequestFacts` (`host`, `path`, `headers`, `client_ip`, `target_surface`)

### Domain Invariants
- Unique channel slugs: Slugs must be unique within a tenant; attempting to create or update to an existing slug returns `ChannelError::SlugAlreadyExists`.
- Bounded pagination: `ChannelListRequest` requires non-zero `page` and `per_page` values; zero values result in `PortError::validation`.
- Single default channel: Only one channel per tenant can be marked default; promoting a new default demotes the previous default atomically.
- Tenant isolation: All queries and mutations strictly verify that the requested channel belongs to the authenticated context's `tenant_id`.

### Events / Outbox Side Effects
- Mutating resolution rules or channel targets increments resolution invalidation generations, invalidating cached channel resolution decisions.

### Errors / Failure Codes
- `ChannelError` variants: `NotFound`, `Validation`, `SlugAlreadyExists`, `TargetAlreadyExists`, `InactiveChannel`, `Database`, `Serialization`.
- `PortError` mapped uniformly to `PortErrorKind::Validation`, `PortErrorKind::NotFound`, or `PortErrorKind::Unavailable`.
