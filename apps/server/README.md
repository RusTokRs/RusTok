# RusToK Server

## Purpose

`apps/server` owns the main backend composition root for RusToK.

## Responsibilities

- Compose platform modules into the live runtime.
- Expose GraphQL, HTTP, and Leptos server-function surfaces.
- Host runtime orchestration, background tasks, and platform-owned control planes.

## Entry points

- `src/main.rs`
- `/api/graphql`
- `/api/fn/*`
- server task/runtime entrypoints under `src/tasks` and `src/services`

## Interactions

- Uses `crates/libs/rustok-core` and platform/domain modules as the backend composition root.
- Serves `apps/admin`, `apps/storefront`, `apps/next-admin`, and `apps/next-frontend`.
- Hosts platform-owned runtime layers such as MCP management, module composition, and orchestration bridges.

## FBA provider composition

Server bootstrap owns deployment configuration, secret supply and provider construction.
This is composition-root work; domain rules and transport mechanics remain with their
owners. Current selectors are deliberately bounded:

- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` selects the remote catalog projection and SKU
  identity ports on the same connection. Product commands remain embedded.
- `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` selects Profiles public-image descriptors.
  It does not move all Media operations or image-byte delivery.

Invalid selected-remote configuration or connection aborts startup. Current Product
commands remain separately embedded; this is mixed composition, not read-failure failover.
See the [configuration reference](../../docs/operations/configuration-reference.md#51-fba-provider-selectors)
and [FBA contract](../../docs/backend/module-backend-architecture.md#fba-contracts).

## Production container configuration

The production Docker image is fail-closed by default: it sets `RUSTOK_ENV=production`, `RUSTOK_CONFIG_DIR=/app/config`, and `RUSTOK_HTTPS=true`. Mount the operator-managed `production.yaml` at `/app/config/production.yaml` (or provide an equivalent config through the configured directory). Development and test configuration files are deliberately not copied into the production image.

## Auth config

`apps/server/src/auth.rs` adapts the host configuration snapshot into
`rustok-auth`.
`auth.jwt.secret` remains the HS256 default secret source. Optional runtime
overrides live under `settings.rustok.auth`:

- `refresh_expiration`, `issuer`, `audience`
- `algorithm`: `HS256` or `RS256`
- `rsa_private_key_env`, `rsa_public_key_env`: preferred production key sources
- `rsa_private_key_pem`, `rsa_public_key_pem`: development/test fallback values

When `algorithm` is `RS256`, both RSA keys are required and boot-time config
assembly must fail instead of silently falling back to `HS256`.

## Request trust and client IP

The host ignores `Forwarded` and `X-Forwarded-*` by default. To enable
forwarded client identity, configure `rustok.runtime.request_trust` with
`forwarded_headers_mode: trusted_only` and explicit
`trusted_proxy_cidrs`. Never use a public client subnet as a trusted proxy.

When the immediate peer is trusted, client identity is resolved from the
rightmost `X-Forwarded-For` / RFC `Forwarded` hop towards the caller, stopping
at the first untrusted address. This identity is also used for API rate-limit
keys. Missing or malformed trusted-side hops fall back to the actual peer;
an untrusted peer cannot supply an authoritative forwarding header. If both
header formats are present, `X-Forwarded-For` takes precedence. For
forwarded host and scheme, the host consumes the most recently appended
(rightmost) value instead of trusting a potentially spoofed first value;
forwarded schemes other than `http` and `https` are ignored.

**Ingress requirement:** the edge proxy must discard untrusted incoming
forwarded headers or append the actual observed client IP. A proxy that
blindly passes through client-supplied forwarding headers without appending
its observed peer cannot supply verifiable client identity. Keep the trusted
CIDRs narrow, including only proxies under operator control.

In production, refresh-session cookies are always marked `Secure`, even
when forwarded-protocol headers are absent or not trusted. Outside production,
the `Secure` flag is enabled only when trusted proxy context reports HTTPS.

## Docs

- [App docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)

## Known Limitations / Pending Implementation

- Platform build history and the process-local build-event hub are exposed to
  tenant module-read permissions. The host-global versus tenant visibility
  contract is unresolved and must be decided before changing all transports and
  clients. Build progress is not a durable cross-process stream.
- Installer HTTP jobs can remain `running` after a process crash or failure to
  persist the terminal result; recovery requires fencing and reconciliation of
  actual effects, not automatic expiry.
- OAuth manifest synchronization is not one atomic, serialized tenant-wide
  reconciliation; built-in auth email delivery remains best effort.
- Module-settings change notification uses a non-transactional event forwarder;
  notification delivery can be lost. Search lag and worker readiness do not
  cover all failure modes. See the server implementation plan for these gates.

## Acknowledged sealed delivery (source only)

The reservation-fact continuation source-wires acknowledged sealed module startup on the actual
listener bus. `outbox` awaits explicit local handlers; `outbox_iggy` accepts the original frame
at the configured primary, then awaits required local delivery. Failure retains the same Outbox
identity for retry, potentially resending remotely. Root best-effort fanout is unchanged. The
host composes owner handlers and durable artifact intake; it never publishes Inventory facts.
Five new transport-double definitions are uncompiled/unrun, not real Iggy/cursor/ACK or
relay/restart/concurrency evidence. Canonical digests, SQL/Search, generic Index and Product
step 8 remain open. See the [runtime contract](docs/README.md#acknowledged-sealed-delivery-2026-11-01-source-only)
and [source packet](../../crates/modules/rustok-inventory/contracts/evidence/reservation-events-source.json).

## Inventory source fan-out continuation (2026-11-02; source only)

Fan-out checks compiled sealed required-module policy before primary sending: an active but
unmatched registry cannot silently ACK Inventory source/reservation facts. With a match, primary
acceptance precedes awaited local completion; failure retains the original frame for at-least-once
retry. Product remote-only routes remain supported; host has no Inventory source SQL/conversion.

Four memory/fan-out definitions use transport/listener doubles, not Iggy cursor or Index evidence.
At this publication handoff the production consumer code was not yet selected. The following
selected-consumer source continuation supersedes that missing-code boundary, not deployment proof. Tests/compilation/runtime remain unrun;
[owner evidence](../../crates/modules/rustok-inventory/contracts/evidence/source-delivery-source.json)
claims no readiness. No live baseline before real handlers and current operator recovery.

## Selected current worker contexts and Index replay (2026-11-02; source only)

Host-provider composition now supplies the Auth-owned current-service context port before source
factories. It resolves exact tenant/consumer SecretRef bindings using canonical secrets/current
OAuth authorization and module availability, not event actors, saved grants or invented service
UUIDs. `RUSTOK_WORKER_SERVICE_TOKENS_JSON` and optional
`RUSTOK_WORKER_SERVICE_SECRET_MOUNT_ROOT` are documented in the
[selected Distribution contract](../../crates/modules/rustok-distribution/docs/inventory-index.md).
The canonical Index materializer registers generic `index_replay` work through the existing
ModuleWork lifecycle; only a server role running background workers polls it. There is no host
Inventory SQL/conversion or domain-specific root worker.

### Known Limitations / Pending Implementation

Tests/build/SQL/runtime were not executed. Native source/listener/worker code is authored, not
proof of deployed credentials, PostgreSQL locking, broker ACK/restart or source readiness. Page
failures currently park; automatic backoff/original-job and full operator-client recovery remain
pending. Do not initialize a live baseline/cutover without explicit credential/schema/digest/
worker/backlog/history/recovery preflight. Original job/lease metadata grants no source authority.
