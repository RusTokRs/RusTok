---
id: doc://crates/rustok-events-module/docs/implementation-plan.md
kind: module_plan
language: en
status: in_progress
last_reviewed: 2026-07-31
---

# Events runtime adapter implementation plan

## Scope

Keep the `events` module registration, manifest, and module-owned admin package
boundary cycle-free while the canonical event and delivery implementation
evolves in its owning crates.

## Current State

- `EventsModule` registers the required core module and declares its `outbox`
  dependency.
- The adapter intentionally owns no migrations and delegates concrete runtime
  readiness to the server event runtime.
- Canonical schemas and validation remain in `rustok-events`; transactional
  persistence remains in `rustok-outbox`.
- Leptos and Next admin packages are present, while live transport parity and
  production Iggy evidence remain in progress under the canonical Events plan.
- The Next package exposes a client-safe registration entrypoint separately
  from its server page and API exports. The package declares its host i18n
  dependency, while server translations are resolved by the host route and
  passed into the package boundary.

## Host-global authority blocker

The native Events Admin surface controls host-global event delivery state, but
currently authorizes it with an ordinary tenant-scoped permission snapshot:

- `event_delivery_configuration_native` accepts `SETTINGS_READ`, then reads the
  host-global `SharedEventDeliveryControl` configuration;
- `update_event_delivery_profile_native` accepts `SETTINGS_MANAGE`, then changes
  the host-global desired delivery profile.

There is no routed tenant because this resource is not tenant-owned. The current
`rustok_api::AuthContext` contains one tenant id and tenant permission snapshot,
but no typed host/platform/root authority. A tenant equality check therefore
cannot close this boundary, and inferring authority from the first/default
active tenant, a nil UUID, OAuth `*:*`, or an ordinary tenant settings role
would recreate the wrong-authority pattern removed from other admin adapters.

GitHub issue #2680 records the cross-owner P0 decision and the matching System
GraphQL diagnostics (`system_health`, `cache_health`, and `events_status`) that
expose host-global state through tenant-scoped `LOGS_READ`. Until Auth/RBAC owns
a typed host-global principal and deny-by-default helper, these transports are
not release-ready. If that authority model cannot land before release, the safe
temporary action is to disable the host-global admin transports rather than
admit ordinary tenant administrators.

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - module registration and manifest boundary are cycle-free;
  - module-owned Leptos and Next packages are colocated with the adapter;
  - the Next host imports only the client-safe registration entrypoint during
    shell composition, so server-only page exports cannot enter the client
    module graph;
  - canonical event/runtime verification remains tracked by `rustok-events`;
  - source inspection proves the native delivery-profile endpoints operate on
    one host-global control while authorization remains tenant-scoped.
- Last verified at (UTC): 2026-07-31
- Owner: Events module maintainers

## Milestones

1. **Define host-global read/manage authority with Auth/RBAC.** Add a typed
   authority that cannot be granted by an ordinary tenant role, propagate it
   through issuance and refresh, and require it before reading or changing
   `SharedEventDeliveryControl`. Add equivalent protection to System GraphQL
   diagnostics. Track the cross-owner contract in issue #2680.
2. Keep adapter metadata and module-owned UI placement synchronized with the
   canonical Events capability.
3. Complete native/GraphQL/Next transport parity in the canonical Events plan.
4. Replace delegated health with composed live readiness only when host runtime
   context can be injected without reversing dependencies.

## Verification

- Source audit: ordinary tenant `SETTINGS_READ`/`SETTINGS_MANAGE` must not admit
  host-global Events Admin operations.
- Source audit: ordinary tenant `LOGS_READ` must not admit host-global System
  GraphQL diagnostics.
- Native regressions: ordinary tenant admin denied; explicitly issued host
  principal admitted for configuration read/profile update.
- GraphQL regressions: ordinary tenant admin denied; explicitly issued host
  principal admitted for system/cache/events diagnostics.
- `cargo check -p rustok-events-module`
- `cargo test -p rustok-events-module`
- `cargo xtask module validate events`
- `cargo xtask validate-manifest`

## Update Rules

- Do not move canonical event schemas into this adapter.
- Do not move outbox or Iggy delivery persistence into this adapter.
- Do not infer host-global authority from a default tenant, magic tenant UUID,
  tenant `SETTINGS_*`/`LOGS_*` permission, or OAuth wildcard alone.
- Keep local FFA/FBA status synchronized with the central readiness board.
- Track event-contract and delivery milestones in the canonical
  `rustok-events` plan rather than duplicating them here.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `Events Admin native delivery-profile configuration/update authority; SharedEventDeliveryControl ownership; AuthContext and Permission authority shape; matching host-global System GraphQL diagnostics`
- Findings: `P0=1, P1=0, P2=0, P3=0`
- Fixed in this pass: `no safe component-local fix; created issue #2680 with the exact cross-owner Auth/RBAC contract and fail-closed temporary option`
- Remaining risks or blockers: `tenant-scoped SETTINGS_READ/SETTINGS_MANAGE currently admit host-global event delivery configuration and mutation; tenant-scoped LOGS_READ admits host-global system/cache/events diagnostics; AuthContext has no platform/root authority discriminator`
- Evidence: `crates/rustok-events-module/admin/src/transport/native_server_adapter.rs reads and mutates SharedEventDeliveryControl without a routed tenant; apps/server/src/graphql/system.rs exposes host-global diagnostics and all-tenant event counts; crates/rustok-api/src/context/auth.rs carries only tenant_id plus tenant permission snapshot; crates/rustok-api/src/permissions.rs defines resource/action permissions without authority scope`
- Next action: `Auth/RBAC owners define and issue typed host-global authority, add deny-by-default helpers, bind Events Admin and System GraphQL to them, and retain ordinary-tenant denial plus explicit-host-principal admission regressions; otherwise disable the host-global transports before release`
- Resume command: `cargo check -p rustok-events-module && cargo test -p rustok-events-module && cargo test -p rustok-server system --lib`
