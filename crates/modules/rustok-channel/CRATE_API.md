# rustok-channel / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `invalidation_generation`, `migrations`, `module`, `policy`, `ports`, `resolution`, `services`, `status`, `target_type`.

## Platform Capability Bindings
- **Core Module Registration**: Exposes `ChannelModule` implementing `RusToKModule` (slug `"channel"`, kind `ModuleKind::Core`) and `MigrationSource`.
- **Runtime Selected Marker**: `ChannelRuntimeSelected` proving channel module participation in runtime extension registration.
- **Channel Service & Resolution**:
  - `ChannelService` managing tenant-scoped channels, targets, module bindings, and OAuth apps.
  - Channel lifecycle commands: `create_channel`, `update_channel` (name/settings; the
    slug is immutable), `set_channel_active`, `delete_channel`, `set_default_channel_for_tenant`,
    `ensure_channel_for_tenant`. Deactivating or deleting the tenant default promotes the
    oldest remaining active channel inside the same transaction, so a tenant never keeps a
    default pointer at an inactive channel.
  - `ChannelStatus` (`active`/`inactive`) is the typed projection of `channels.is_active`.
    There is no separate stored status column: `dto::ChannelResponse::status` and the index
    projection in `index_projection::SALES_CHANNEL_ROWS_CTE` are both derived, so the Sales
    Channel Index `status` field can never contradict `is_active` again.
  - Read surfaces for consumers: `list_channel_options(tenant_id, include_inactive)` returns
    the complete tenant channel option list (no pagination, no silent truncation) for
    selectors and filters — storefronts pass `false`, admin filters pass `true`. Detail reads
    (`list_channel_details`, `list_channel_details_page`, `get_default_channel`,
    `get_channel_by_host_target_value`) assemble targets, module bindings and OAuth apps with
    three set-based queries per page, not three per channel.
  - Deletes and target/binding removals are single scoped statements (`delete_many` +
    `rows_affected`), so a row can only be removed through the channel that owns it.
    `ChannelError::Conflict` marks a lost concurrency race; transports answer it with a
    retryable conflict (`409` on REST, `CHANNEL_CONCURRENT_CONFLICT` on Commerce GraphQL).
  - `is_module_enabled_for_tenant(tenant_id, channel_id, module_slug)` is the only sanctioned
    module gate for request paths: it checks channel ownership before the binding. The
    unscoped `is_module_enabled` is an owner-internal convenience and must not be used where
    the channel id came from a request.
  - `ChannelResolver`, `RequestFacts`, `ResolutionDecision`, `ResolutionOutcome` evaluating request facts against resolution rules.
  - `StoredChannelResolutionRule`, `ChannelResolutionRuleDefinition`, `ResolutionAction`, `ResolutionPredicate`.
- **Ports & Transports**:
  - `ChannelReadPort` (`read_channel`, `list_channels_for_tenant`); the declared
    `deadline_required` contract is enforced: every owner lookup runs inside the caller's
    `PortContext::deadline_ms` budget and an exhausted budget yields
    `PortErrorKind::Timeout` / `port.deadline_exceeded`.
  - `in_process_channel_read_port` constructing direct in-process provider adapters.
  - `ChannelReadSelector` (`Id`, `Slug`, `HostTargetValue`, `Default`).
  - `ChannelIndexIdentityReadPort` reads generation and UUID membership from
    one tenant snapshot. The typed contract and gRPC adapters live under
    [ports](ports/README.md).
  - `PostgresChannelIndexIdentity::read_in` uses the same query with the
    embedded resolver's existing PostgreSQL transaction.
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

## Saved commerce selector read (2026-10-30; source only)

`ChannelReadPort::read_selected_channel(context, channel_id, channel_slug)` returns
`Result<Option<ChannelReadProjection>, PortError>` by composing the selected provider's
`read_channel`. Explicit selectors must describe one active, tenant-owned, non-nil Channel;
slug-only or ID-only requests complete the pair. Blank/nil, contradictory, missing, foreign,
inactive and unavailable results refuse. Neither selector means global (`None`), independent
of page/host/default context. This does not change `ChannelResolver` request-routing precedence.
Pricing retains its separate trusted-context admission before delegating to this owner method.
No storage, migration or public GraphQL/HTTP shape changes are implied; regression sources
are unexecuted and mounted/provider/runtime evidence remains open.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`

## Common AI Mistakes
- Using wildcard re-exports instead of explicit named imports.
- Hardcoding default channels instead of evaluating tenant resolution rules or querying `ChannelReadPort`.
- Bypassing tenant validation on channel target operations.
- Ignoring inactive channel visibility rules (`include_inactive`).
- Calling the unscoped `is_module_enabled` from a request path, or re-introducing a `250`-row page as a stand-in for `list_channel_options`.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateChannelInput` (`tenant_id`, `slug`, `name`, `settings`) and `CreateChannelRequest` (`slug`, `name`, `settings`; the tenant comes from the request context).
- `UpdateChannelRequest` / `UpdateChannelInput` (`name`, `settings` only — the slug is immutable, and a patch with neither field is rejected).
- `SetChannelActiveRequest` (`is_active`).
- `CreateChannelTargetInput` / `UpdateChannelTargetInput` (`target_type`, `value`, `is_primary`, `settings`).
- `BindChannelModuleInput` (`module_slug`, `is_enabled`, `settings`) and `BindChannelOauthAppInput` (`oauth_app_id`, `role`); binding is an upsert on the binding's unique key.
- `CreateChannelResolutionPolicySetInput` (`tenant_id`, `slug`, `name`, `is_active`), `CreateChannelResolutionRuleInput` (`priority`, `is_active`, `definition`), `UpdateChannelResolutionRuleInput` (`priority`, `is_active`, `action_channel_id`, `host_equals`, `host_suffix`, `oauth_app_id`, `surface`, `locale`; an explicitly empty predicate field clears that predicate and the remaining rule must stay a valid conjunction), `ReorderChannelResolutionRulesInput` (`rule_ids`, which must list every rule of the set exactly once).
- `ChannelReadRequest` (`selector`, `include_inactive`), `ChannelListRequest` (`page`, `per_page`, `include_inactive`), `RequestFacts` (`tenant_id`, `surface`, `header_channel_id`, `header_channel_slug`, `query_channel_slug`, `host`, `oauth_app_id`, `locale`).

### Domain Invariants
- Unique channel slugs: Slugs must be unique within a tenant; attempting to create an existing slug returns `ChannelError::SlugAlreadyExists`. Slugs are immutable afterwards.
- Single default channel: Only one channel per tenant can be marked default, the default must be active, and promoting a new default demotes the previous default atomically. Deactivating or deleting the default promotes the oldest remaining active channel in the same transaction.
- Unique target claims: A `web_domain` value is claimed by at most one channel per tenant (`uq_channel_targets_tenant_host`) and a channel has at most one primary target (`uq_channel_targets_channel_primary`); both are database invariants, and target writes carry the owning `tenant_id` denormalized so the composite foreign key proves the channel belongs to that tenant.
- Scoped removals: Target, module-binding and OAuth-binding removals are single channel-scoped statements, so a row can only be removed through the channel that owns it. Resolution rules are removed the same way: one `policy_set_id`-scoped delete whose zero row count means the rule is already gone, answered as `NotFound` rather than a storage failure.
- Bounded pagination: `ChannelListRequest` requires non-zero `page` and `per_page` values; zero values result in `PortError::validation`. Option lists use `list_channel_options` and are never truncated.
- Tenant isolation: All queries and mutations strictly verify that the requested channel belongs to the authenticated context's `tenant_id`; the unscoped `is_module_enabled`/`set_default_channel` forms are owner-internal only.
- Closed `settings` shape: `create_channel` and `update_channel` reject a `settings` value that is not a JSON object, or that exceeds 64 KiB, with `ChannelError::Validation`. The keys are module-owned; only the shape readers rely on is enforced.
- Publication state: `is_active` is the only stored fact. `ChannelStatus`, the API `status` field and the index projection are derived from it, so no writer can store a status that contradicts it.

### Events / Outbox Side Effects
- Mutating resolution rules or channel targets increments resolution invalidation generations, invalidating cached channel resolution decisions.

### Errors / Failure Codes
- `ChannelError` variants: `Validation`, `SlugAlreadyExists`, `NotFound`, `InactiveChannel`, `InvalidTargetType`, `InvalidTargetValue`, `InvalidPolicyDefinition`, `InvalidPolicyOperation`, `TargetAlreadyExists`, `PolicySetSlugAlreadyExists`, `Conflict` (a lost concurrency race the caller may retry), `Database`, `Serialization`.
- Target conflicts are classified from state, not from the unique-violation text: a duplicate value in the tenant is `TargetAlreadyExists`, a lost primary slot (`uq_channel_targets_channel_primary`) is `Conflict` (retryable).
- Bind upserts (`bind_module`, `bind_oauth_app`) read the row they wrote back through the same transaction.
- Transport mapping: `NotFound` → `404`; validation and policy-input failures → `400`; `InactiveChannel`, `SlugAlreadyExists`, `TargetAlreadyExists`, `PolicySetSlugAlreadyExists` and `Conflict` → `409`; `Database`/`Serialization` → `500` with a redacted internal-error code.
- `PortError` mapping (`ports.rs`): `Validation|Invalid*` → `PortErrorKind::Validation`, `NotFound`/`InactiveChannel`/foreign-tenant projection → `PortErrorKind::NotFound`, the conflict family → `PortErrorKind::Conflict`, storage failure → `PortErrorKind::Unavailable` (retryable), encoding failure → `PortErrorKind::InvariantViolation`, and a call that outlives the caller's deadline budget → `PortErrorKind::Timeout` (`port.deadline_exceeded`, retryable).
- `ChannelError::Conflict` also surfaces through Commerce GraphQL as `CHANNEL_CONCURRENT_CONFLICT` with `retryable: true`.
