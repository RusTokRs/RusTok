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
OAuth application or wildcard, built-in tenant role, default tenant, or magic
UUID.

The source contract now separates authority and issuance from tenant identity:

- `rustok_api::HostAuthorityContext` is a typed request context with explicit
  `Read` and `Manage` levels and a required non-nil operator actor;
- the server authenticates a dedicated high-entropy
  `X-RusTok-Host-Token` against SHA-256 digests in host-owned
  `RUSTOK_HOST_AUTHORITY_CREDENTIALS` configuration;
- tenant OAuth applications, scopes, metadata, roles and permissions do not
  participate in that credential policy;
- native `#[server]` requests receive the typed context from Axum middleware;
- HTTP GraphQL validates the same header from request data; GraphQL WebSocket
  intentionally retains no host authority and fails closed;
- `event_delivery_configuration_native` requires host read authority before
  resolving `SharedEventDeliveryControl`;
- `update_event_delivery_profile_native` requires host manage authority and
  writes the configured operator actor to the update audit path;
- matching host-global System and Settings GraphQL operations use the same
  authority levels;
- Iggy mutation additionally requires ordinary authenticated tenant context
  matching the routed tenant because encrypted connector secrets remain owned
  by that tenant; its audit actor remains the host operator;
- `scripts/verify/verify-host-global-authority-boundary.mjs` locks credential
  ownership, transport scope and guard-before-resource ordering.

The operational format, token-generation guidance and overlap rotation procedure
are documented in `apps/server/docs/host-authority.md`. Issue #2680 remains open
until same-SHA compile/unit/source evidence and live denial/admission, rotation,
revocation and replica-parity evidence are retained.

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
    host-owned credential and typed authority context.
- Last verified at (UTC): 2026-07-31
- Owner: Events module maintainers

## Milestones

1. **Retain host-operator execution evidence.** Run the source guard, server/API
   unit tests, server/events compile checks and live HTTP/native denial/admission,
   rotation, revocation, audit-actor and multi-replica probes for issue #2680.
2. Keep adapter metadata and module-owned UI placement synchronized with the
   canonical Events capability.
3. Complete native/GraphQL/Next transport parity in the canonical Events plan.
4. Replace delegated health with composed live readiness only when host runtime
   context can be injected without reversing dependencies.

## Verification

- `node scripts/verify/verify-host-global-authority-boundary.mjs`
- `cargo test -p rustok-api host_authority -- --nocapture`
- `cargo test -p rustok-server host_authority --lib -- --nocapture`
- `cargo check -p rustok-events-module`
- `cargo test -p rustok-events-module`
- `cargo check -p rustok-server --lib`
- `cargo xtask module validate events`
- `cargo xtask validate-manifest`
- Live native/HTTP GraphQL regressions: no header, wrong token and ordinary
  tenant admin denied; `read` admitted only for reads; `manage` admitted for
  reads/writes with the configured audit actor; WebSocket denied.
- Rotation/revocation regressions: old/new overlap succeeds during rollout and
  the removed token fails on every replica after rollout.

## Update Rules

- Do not move canonical event schemas into this adapter.
- Do not move outbox or Iggy delivery persistence into this adapter.
- Do not infer host-global authority from a default tenant, magic tenant UUID,
  tenant `SETTINGS_*`/`LOGS_*` permission, built-in tenant role, OAuth app,
  scope, metadata or wildcard.
- Do not store raw host tokens in repository files, application settings, tenant
  rows, logs, URLs, browser storage or issue comments.
- Keep GraphQL WebSocket host authority deny-by-default unless a separately
  reviewed revocation-safe handshake/revalidation contract is added.
- Keep local FFA/FBA status synchronized with the central readiness board.
- Track event-contract and delivery milestones in the canonical
  `rustok-events` plan rather than duplicating them here.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-31`
- Scope inspected: `Events Admin native delivery-profile configuration/update authority; SharedEventDeliveryControl ownership; host-global System and Settings HTTP GraphQL operations; tenant OAuth app administration and secret rotation; middleware and GraphQL WebSocket composition`
- Findings: `P0=1, P1=0, P2=0, P3=0`
- Fixed in this pass: `retained the typed host read/manage context; replaced the unsafe tenant-OAuth-client allowlist design before PR with a server-owned opaque credential whose raw token is supplied only in X-RusTok-Host-Token and whose SHA-256 digest, non-nil audit actor and level live only in RUSTOK_HOST_AUTHORITY_CREDENTIALS; added constant-time comparison, bounded parsing, duplicate-hash rejection, overlap rotation, independent native/HTTP GraphQL composition, WebSocket denial and tenant equality for Iggy secret ownership`
- Remaining risks or blockers: `same-SHA formatting, compile, unit and source-verifier evidence are pending; live ordinary-tenant denial, explicit read/manage admission, audit actor, rotation/revocation and multi-replica parity are not retained; issue #2680 remains open until those gates pass`
- Evidence: `crates/rustok-api/src/context/host_authority.rs; apps/server/src/host_authority.rs; apps/server/src/middleware/auth_context.rs; apps/server/src/graphql/system.rs; apps/server/src/graphql/settings/{mod,query,mutation}.rs; crates/rustok-events-module/admin/src/transport/native_server_adapter.rs; scripts/verify/verify-host-global-authority-boundary.mjs; apps/server/docs/host-authority.md; connector-only local execution remains unavailable because github.com DNS resolution fails`
- Next action: `run the source guard and targeted Rust checks on the branch SHA, fix every branch-related failure, then retain live HTTP/native admission, denial, rotation, revocation, audit and replica evidence before closing issue #2680`
- Resume command: `node scripts/verify/verify-host-global-authority-boundary.mjs && cargo test -p rustok-api host_authority -- --nocapture && cargo test -p rustok-server host_authority --lib -- --nocapture && cargo check -p rustok-events-module && cargo test -p rustok-events-module && cargo check -p rustok-server --lib`