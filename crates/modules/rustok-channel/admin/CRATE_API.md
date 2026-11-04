# Crate API: rustok-channel-admin

## Public surface

### Core
- `channel_selection_exists`: Verifies if a selected channel ID is present in bootstrap data.
- `channel_policy_selection_cleanup`: Normalizes query parameters for policy sets and rules.
- `PolicyRuleFormState`: State management for channel policy rule forms.
- `reorder_policy_rule_ids`: Calculates reordered rule ID sequences.

### Models
- `ChannelAdminBootstrap`: Complete bootstrap payload for channel administration.
- `ResolvedChannelContext`: Runtime resolution context for current request.
- `ChannelDetail`: Channel representation with target, module, and OAuth associations.
- `ChannelRecord`: Core channel entity record.
- `ChannelTargetRecord`: Deployment target configuration.
- `ChannelModuleBindingRecord`: Module surface binding record.
- `ChannelOauthAppRecord`: OAuth application binding record.
- `ChannelResolutionPolicySetDetail`: Resolution policy set with ordered rules.
- `ChannelResolutionRuleRecord`: Resolution rule definition and priority.
- `ChannelResolutionPredicateRecord`: Resolution condition predicate variant.
- `ChannelResolutionActionRecord`: Resolution target action.

### Transport
- `ApiError`: Typed error representation for REST and server function failures.
- Native server functions for bootstrap, channel CRUD, target CRUD, module bindings, OAuth bindings, and policy management.
- REST transport adapters as dual-transport fallback.

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
- `CreateChannelPayload`: DTO for channel creation.
- `CreateChannelTargetPayload`: DTO for target attachment and modification.
- `BindChannelModulePayload`: DTO for module capability binding.
- `BindChannelOauthAppPayload`: DTO for OAuth application attachment.
- `CreateResolutionPolicySetPayload`: DTO for resolution policy set creation.
- `CreateResolutionRulePayload`: DTO for resolution rule addition.
- `UpdateResolutionRulePayload`: DTO for resolution rule updates.
- `ReorderResolutionRulesPayload`: Command for reordering resolution rules.

### Domain Invariants
- Channels must maintain unique slug identifiers within tenant boundaries.
- Target bindings and module associations validate configuration before mutation.
- Zero business logic in UI view layer; state mapping stays in `core`.
- All permission checks strictly enforce tenant boundary equality before authorization admission.

### Events / Outbox Side Effects
- Mutates channel routes, target deployments, and module bindings via channel service.

### Errors / Failure Codes
- `ApiError::Rest`: REST transport communication or deserialization errors.
- `ApiError::ServerFn`: Native server function execution errors.
- Validation failures propagate with descriptive user feedback.
- Network and server function errors handled gracefully without UI panics.
