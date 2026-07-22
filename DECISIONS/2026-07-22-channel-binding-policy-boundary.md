# Channel binding policy boundary

## Status

Accepted

## Decision

`rustok-channel` remains the owner of channel resolution and channel-owned
storage. At the module policy boundary, a host or channel adapter supplies a
validated `ModuleEffectivePolicyChannelInput` containing tenant and channel
identity, target surface, an immutable `sha256:` channel revision, active
state, and module binding decisions. `rustok-modules` evaluates this snapshot
alongside catalog, tenant, installation, capability, executor, dependency, and
security facts without depending on `rustok-channel` or querying its tables.

Core definitions are not required to have an explicit binding while an active
channel is resolved. Optional definitions require an enabled binding; missing
and disabled bindings, as well as an inactive channel, produce typed denial
reasons. The channel snapshot is included in the deterministic effective-policy
revision, so routing, lifecycle, and runtime consumers can reject stale
channel state by revision rather than rebuilding channel semantics.

## Consequences

- Channel transport and storage remain isolated from the module owner.
- Hosts must map `ResolutionDecision`/channel detail into the neutral input and
  call `EffectivePolicyService::resolve_for_channel`.
- Maintenance and generic node-readiness remain separate policy inputs and are
  not implied by channel binding.
