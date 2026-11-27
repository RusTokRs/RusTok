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


## Transaction-bound artifact HTTP authority (2026-11-27)

The host retains canonical HTTP RequestFacts and the selected channel identity
from middleware, including an unresolved result. Artifact policy admission
re-resolves those facts directly through Channel (never the TTL cache), brackets
the resolution with the durable channel generation, and rejects a different
selection or a changed generation. Map the exact owner binding list into the
existing neutral Modules input. Optional modules need explicit enabled bindings
in a resolved channel; no-channel host policy remains separate.

The serving lease retains that input and the generation, including unresolved
HTTP context. Fresh capability checks use captured host evidence rather than
task-local context from a sandbox worker. Structured SQL effects hold a shared
lock on the Channel-owned generation row (SQLite write reservation) after
selection/readiness/composition fences until commit. Existing resolution
triggers serialize channel, target, binding, OAuth association, policy set and
rule mutations with the effect; a disable/enable ABA cannot renew a lease.
The global generation conservatively invalidates unrelated contextual leases.
This is source implementation, not PostgreSQL contention evidence. Deferred
transports that do not carry the middleware request scope, channel-specific
durable background work, non-SQL effect fencing and operational maintenance
authority remain open. No new generation or channel-table dependency in
Modules is introduced.
