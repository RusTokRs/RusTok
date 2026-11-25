# RusToK Server

## Purpose

`apps/server` owns the main backend composition root for RusToK.

## Responsibilities

RBAC generation-watchdog pre-migration diagnostics distinguish the
RBAC generation table from unrelated missing relations. Shutdown
interrupts pending watchdog admission, and multi-stage module
transition/artifact maintenance stops before entering additional
independent durable operations.


Cache listener startup for Channel/RBAC invalidation, tenant generation,
tenant locale and Redis-status is stop-interruptible during unfinished
initial recovery and replacement joins. Shutdown still takes and drains
already published JoinHandles under each listener's registration lock.


Public direct worker starters register Product Search channel repair,
Notifications intake/candidate/fanout, Forum Search inbox/contract, Product
Index refresh, Social Graph Index, Comments TCP listener and Comments durable
audit handoff JoinHandles under one runtime-local lock also used when
shutdown takes those handles. This closes
check-stop/publication races without holding the lock during async broker,
database or task-drain operations.


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

- `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` selects the remote catalog projection, SKU
  identity, and REST and GraphQL storefront list ports on the same connection.
  Set `RUSTOK_PRODUCT_VARIANT_REPLAY_GRPC_BEARER_TOKEN`
  on both processes to serve ProductVariant replay to Index. The catalog read
  selector leaves command placement to the command selector.
  Use a separate `RUSTOK_PRODUCT_LOCALE_REPLAY_GRPC_BEARER_TOKEN` to enable
  localized Product replay on the same connection.
- `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` selects Profiles public-image descriptors.
  It does not move all Media operations or image-byte delivery.

Invalid selected-remote configuration or connection aborts startup. Index query
admission and absence watermarks still require the shared database profile.
See the [configuration reference](../../docs/operations/configuration-reference.md#51-fba-provider-selectors)
and [FBA contract](../../docs/backend/module-backend-architecture.md#fba-contracts).

### Mounted Product storefront evidence

Build the host HTTP test executable with the Commerce profile selected.

```sh
cargo test --locked -p rustok-server --no-default-features --features mod-commerce \
  --test product_remote_storefront_http --no-run
```

Set `RUSTOK_PRODUCT_TEST_COMMERCE_HOST_BIN` to the executable printed by Cargo
before running the Product PostgreSQL fixture with `RUSTOK_TAXONOMY_TEST_SERVICE_BIN`.
The fixture runs the host executable with the live Product credential and endpoint.
It sends HTTP requests through the production router and middleware on loopback.
The consumer uses a fresh SQLite database for platform state, while Product and
Taxonomy use separate PostgreSQL databases and run in their own processes.
[Provider fixture setup](../../crates/modules/rustok-taxonomy/docs/README.md#verification)

## Production container configuration

The production Docker image is fail-closed by default: it sets `RUSTOK_ENV=production`, `RUSTOK_CONFIG_DIR=/app/config`, and `RUSTOK_HTTPS=true`. Mount the operator-managed `production.yaml` at `/app/config/production.yaml` (or provide an equivalent config through the configured directory). Development and test configuration files are deliberately not copied into the production image.

Every release build also enforces the host's production-security posture even when `RUSTOK_ENV` is explicitly set to `staging` or `development`. That label cannot relax database, HTTPS/HSTS, JWT, tenant-isolation, Secure-cookie, CORS, WebSocket, observability, or installer setup-token checks. Debug builds may use the selected non-production profile; environment-label-only behavior remains separate where it is intentionally not a security gate.

## Auth config

`apps/server/src/auth.rs` adapts the host configuration snapshot into
`rustok-auth`.
`auth.jwt.secret` remains the HS256 default secret source. Optional runtime
overrides live under `settings.rustok.auth`:

- `refresh_expiration`, `issuer`, `audience`
- `algorithm`: `HS256` or `RS256`
- `rsa_private_key_env`, `rsa_public_key_env`: preferred production key sources
- `rsa_private_key_pem`, `rsa_public_key_pem`: development/test fallback values

When the OAuth surface is mounted (`settings.features.oauth_enabled` is true and the host mode is not
`worker` or `registry_only`), `settings.rustok.auth.issuer` must be an absolute root URL:
HTTPS in production, or HTTPS / `http://localhost` / `http://127.0.0.1` in
development. Userinfo, path, query, and fragment components are rejected.
OAuth discovery advertises this exact JWT issuer; it never derives an issuer
from request Host or forwarded headers.

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
(rightmost) value instead of trusting a potentially spoofed first value.
When `X-Forwarded-Host` or `X-Forwarded-Proto` is present but malformed,
it is not silently replaced by a conflicting RFC `Forwarded` value.
RFC `Forwarded` remains a fallback only when the corresponding `X-Forwarded-*`
header is absent; forwarded schemes other than `http` and `https` are ignored.

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
- Installer HTTP jobs heartbeat every 30 seconds. A five-minute heartbeat gap
  atomically moves a stale `running` job to `recovery_required`; both states
  share one database-enforced admission slot, so a stale attempt is never
  reset to `failed` or silently replaced. The executor marks ambiguous
  database/schema/session/receipt/state outcomes as requiring recovery, and
  `FreshInstallCleaned` is no longer used when cleanup did not actually run.
  Late heartbeats and terminal writes cannot revive or overwrite a quarantined
  job. The status API exposes the host job even when its target session is not
  in the runtime database; the receipts API returns an explicit conflict
  rather than a misleading empty list in that case. The host job ledger and
  target session/receipts remain separate persistence boundaries: this detects
  interruption and blocks retries, but does not automatically resume or
  reconcile remote target effects. Operators must inspect the target database
  and receipts before retrying. Lease expiry is never permission for takeover.
- OAuth manifest synchronization is atomic and serialized per tenant. The
  all-tenants bootstrap pass commits one tenant at a time, not as a single
  cross-tenant transaction. Built-in auth email delivery remains best effort.
- Static module-settings changes use the canonical transactional outbox. The
  legacy in-process EventBus-to-Outbox bridge is still not crash-durable before
  its first outbox insert: a process crash or forced abort in that window can
  lose a non-transactional event. Business mutations requiring atomic publication
  must use `TransactionalEventBus`. Search lag and worker readiness do not cover
  all failure modes; see the server implementation plan.
- First-party in-place refresh rotation retains only the immediately previous
  token digest. Older revoked generations are rejected as unknown but cannot
  trigger full-lineage theft revocation. A replay inside the 30-second race
  window is rejected without revoking the live session, but the server does
  not replay the successful refresh response; if that response was lost, the
  client cannot recover its replacement cookie from the old token. Durable
  rotation history, idempotent response recovery, and incident reconciliation
  remain open; see the implementation plan.
- OAuth refresh-token reuse now propagates failed revocation persistence as
  `server_error`, allowing the caller to retry after a storage outage instead of
  falsely reporting only an invalid grant. Its 30-second grace window suppresses
  family revocation for a likely concurrent duplicate, but still returns
  `invalid_grant` and does not replay the replacement token; a lost successful
  response cannot be recovered from the old token. Durable incident/retry
  evidence and two-replica failure-injection verification remain pending.

## Acknowledged sealed delivery (source only)

The reservation-fact continuation source-wires acknowledged sealed module startup on the actual
listener bus. `outbox` awaits explicit local handlers; `outbox_iggy` accepts the original frame
at the configured primary, then awaits required local delivery. Failure retains the same Outbox
identity for retry, potentially resending remotely. Root `DomainEvent` delivery through the
Outbox target now awaits matching registered module handlers; the generic in-process broadcast
API remains best-effort. The host composes owner handlers and durable artifact intake; it never
publishes Inventory facts. See the [root-delivery decision](../../DECISIONS/2026-11-18-acknowledged-root-event-delivery.md).
The new transport-double and regression definitions are uncompiled/unrun, not real Iggy/cursor/ACK
or relay/restart/concurrency evidence. Canonical digests, SQL/Search, generic Index and Product
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
