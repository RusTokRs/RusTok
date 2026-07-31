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

## Host-global authority boundary

The native Events Admin surface controls host-global event delivery state. It
must never infer authority from a routed tenant, tenant `SETTINGS_*` permission,
OAuth wildcard, built-in tenant role, default tenant, or magic UUID.

The immediate cross-owner P0 exposure is source-mitigated:

- `rustok_api::HostAuthorityContext` is a separate typed request context with
  explicit `Read` and `Manage` levels and a required non-nil operator actor;
- ordinary tenant authentication does not issue this context;
- `event_delivery_configuration_native` requires host read authority before
  resolving `SharedEventDeliveryControl`;
- `update_event_delivery_profile_native` requires host manage authority and
  writes the bound operator actor to the update audit path;
- matching host-global System and Settings GraphQL operations use the same
  authority contract;
- `scripts/verify/verify-host-global-authority-boundary.mjs` locks the separation
  from tenant RBAC and the required guard-before-resource ordering.

No operator issuance path exists yet. The transports therefore fail closed in
normal tenant requests. GitHub issue #2680 remains open for the Auth/RBAC-owned
operator credential, issuance, refresh/revocation, GraphQL/native composition,
and live admission evidence. This is intentional: unavailable host controls are
safer than tenant administrators receiving process-wide authority.

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
  - source inspection proves host-global delivery controls require a separate
    authority context that ordinary tenant authentication does not issue.
- Last verified at (UTC): 2026-07-31
- Owner: Events module maintainers

## Milestones

1. **Complete host-operator issuance with Auth/RBAC.** Issue the typed host
   context only from an approved operator credential, propagate it through
   refresh/revocation and GraphQL/native composition, and retain ordinary
   tenant denial. Track the cross-owner contract in issue #2680.
2. Keep adapter metadata and module-owned UI placement synchronized with the
   canonical Events capability.
3. Complete native/GraphQL/Next transport parity in the canonical Events plan.
4. Replace delegated health with composed live readiness only when host runtime
   context can be injected without reversing dependencies.

## Verification

- `node scripts/verify/verify-host-global-authority-boundary.mjs`
- Source audit: ordinary tenant `SETTINGS_READ`/`SETTINGS_MANAGE` must not admit
  host-global Events Admin operations.
- Source audit: ordinary tenant `LOGS_READ` must not admit host-global System
  GraphQL diagnostics.
- Native regressions: ordinary tenant admin denied; explicitly issued host
  principal admitted for configuration read/profile update.
- GraphQL regressions: ordinary tenant admin denied; explicitly issued host
  principal admitted for system/cache/events diagnostics.
- `cargo test -p rustok-api host_authority -- --nocapture`
- `cargo check -p rustok-events-module`
- `cargo test -p rustok-events-module`
- `cargo xtask module validate events`
- `cargo xtask validate-manifest`

## Update Rules

- Do not move canonical event schemas into this adapter.
- Do not move outbox or Iggy delivery persistence into this adapter.
- Do not infer host-global authority from a default tenant, magic tenant UUID,
  tenant `SETTINGS_*`/`LOGS_*` permission, built-in tenant role, or OAuth
  wildcard.
- Do not insert `HostAuthorityContext` from ordinary tenant authentication.
- Keep local FFA/FBA status synchronized with the central readiness board.
- Track event-contract and delivery milestones in the canonical
  `rustok-events` plan rather than duplicating them here.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `Events Admin native delivery-profile configuration/update authority; SharedEventDeliveryControl ownership; host-global System and Settings GraphQL operations; AuthContext, OAuth app and manifest issuance paths`
- Findings: `P0=1, P1=0, P2=0, P3=0`
- Fixed in this pass: `added a typed host authority context independent of tenant RBAC; required host read/manage authority before every confirmed host-global Events/System/Settings operation; bound mutations to a non-nil operator actor; kept ordinary tenant authentication from issuing the context; added a source boundary verifier`
- Remaining risks or blockers: `operator credential issuance, refresh/revocation and host composition are not implemented, so host-global admin transports intentionally fail closed; same-SHA formatting, compile, unit, source-verifier and live ordinary-tenant-denial evidence remain pending`
- Evidence: `crates/rustok-api/src/context/host_authority.rs; crates/rustok-events-module/admin/src/transport/native_server_adapter.rs; apps/server/src/graphql/system.rs; apps/server/src/graphql/settings/{mod,query,mutation}.rs; scripts/verify/verify-host-global-authority-boundary.mjs; connector-only local execution remains unavailable because github.com DNS resolution fails`
- Next action: `run the source verifier and targeted Rust checks on the same SHA; then implement an Auth/RBAC-owned operator issuance path for issue #2680 and prove ordinary tenant denial plus explicit host-principal admission in native and GraphQL transports`
- Resume command: `node scripts/verify/verify-host-global-authority-boundary.mjs && cargo test -p rustok-api host_authority -- --nocapture && cargo check -p rustok-events-module && cargo test -p rustok-events-module`
