# Documentation: `apps/server`

### Workflow durable cron admission / outcome review refinement (2026-11-23)

Cron now atomically checks the current firing attempt and locked workflow
definition, then inserts its execution journal in the same transaction before
any external step begins. This closes the operator-requeue race where an older
worker could still insert a journal after a review found none. Long-running
attempts heartbeat only while the same token is current; the engine rechecks
the token before further steps and finalizes its status under a firing-row
write fence so a late worker cannot overwrite operator recovery.

Operator `confirm_external_completed` / `confirm_external_failed` can
resolve the full overall workflow execution outcome only with human
`workflows:manage` authority, bounded evidence and explicit confirmation
that the former worker is stopped. The operator must account for **all**
remaining steps and possible remote side effects, not just one HTTP response.
`confirm_terminal` remains a reconciliation of a previously terminal
journal, while `requeue_confirmed_unstarted` refuses any firing with an
execution journal. All updates and immutable review rows commit together.
This is not provider-level exactly-once. Migrator composition already includes
Workflow-owned migrations through the canonical ModuleMigrationSource.

Source regressions now cover competing claimers, immutable audit, stale token,
unstarted requeue, terminal review, external failure resolution, HTTP
no-blind-retry and schedule revision on step edits. None were run by request.


Local documentation for the main RusToK backend host application. This file captures only the live composition/runtime contract; detailed runbooks, framework notes, and rollout plans live in specialized documents inside this folder and in the central docs.

### Additional 2026-11-23 host cache and readiness hardening

- Pages event deduplication follows the generation-store owner identity,
  rather than a process-global event-ID set or value-backend instance.
  Independent `CacheService` owners within one process each rotate their own
  generations; ports sharing the same owner suppress duplicate rotations.
  Invalidation does not initialize the cache value backend. The weak registry
  is bounded and does not retain retired cache hosts.
- The `/health/ready` aggregate now samples the shared shutdown signal
  after asynchronous dependency checks in **every** host mode, including
  registry-only and surface hosts. A stopping process responds 503 even if
  its dependencies remain healthy and it has no required workers.
- Readiness circuit breakers are held in the `ServerRuntimeContext` rather
  than keyed globally across all hosts in the process. A dependency failure
  in one runtime no longer prevents another runtime from testing its own
  healthy dependency. Clones of the same runtime still share the breaker.

## Purpose

## Server audit continuation — Workflow-owned durable cron recovery (2026-11-23)

The server only composes the Workflow owner's `start_with_stop` scheduler
after `validate_persistence` confirms the additive owner cron tables exist.
Workflow owns bounded schedule backfill, cursor/queue CAS, per-firing execution
idempotency, stale attempt quarantine and permissioned operator reconciliation.
A missing migration fails host initialization instead of leaving an apparently
running cron scheduler. Host stop still joins accepted executions with its
existing force-abort fallback. The proposed Workflow ADR, migration and
source regressions require maintainer review and execution; no runtime proof
or exactly-once external effect guarantee is claimed.

## Server audit continuation — Workflow cron execution supervision (2026-11-23)

The previous Workflow cron scheduler detached each execution task with
`tokio::spawn`; stopping its scheduler task did not account for accepted
executions. Workflow now owns the spawned tasks in a `JoinSet` and provides
`start_with_stop`. The server passes its existing host stop receiver directly
to the Workflow owner. That owner refuses new firings after stop, also checks
between database/step-load awaits, and drains already admitted executions
before its scheduler JoinHandle completes. The host's five-second forced
abort remains the outer bound; this does **not** provide distributed cron
claims, missed-firing recovery or crash-safe execution replay. Two owner
regressions were authored for accepted-task drain and pre-signaled stop.
All checks remain unrun at the maintainer's request.

## Server audit continuation — producer/dispatcher shutdown ordering (2026-11-23)

The host previously joined Outbox relay shutdown concurrently with module event
dispatcher shutdown. The dispatcher could unsubscribe while the relay was
finishing a committed Outbox delivery and waiting for the local listener
acknowledgement. Shutdown now joins all upstream producers (including the
cooperatively draining relay) first, then requests the dispatcher stop, and
finally drains the event forwarder. This keeps the required listener registry
available for accepted relay work and the forwarder available for event writes
from a finishing dispatcher handler. A focused test holds an in-flight relay
drain and asserts that the dispatcher remains registered until it completes.

Each stage still has the existing bounded shutdown/abort fallback. Process
loss, a deadline exceeded by an external provider, and a rejected local
consumer remain recoverable owner-side delivery outcomes, not exactly-once
guarantees. Regression code is authored, not compiled or executed.

## Server audit continuation — cooperative Outbox relay drain (2026-11-23)

The Outbox relay supervisor previously aborted its inner `JoinSet` on every
graceful stop. A pass can have dispatched an event to an external transport
and still be awaiting its durable status acknowledgement, so aborting the
future at that point manufactures an ambiguous result and unnecessary replay.
The inner poller now watches shutdown before each pass and while sleeping,
then finishes any admitted `OutboxRelayPort::process_pending_once` pass before
returning. The supervisor joins the inner poller on stop rather than aborting
it immediately; unexpected child exits still use the existing bounded restart
supervision. A regression constructs a real SQLite Outbox row and a blocking
transport, requests stop during publication, and verifies that the row is
acknowledged `dispatched` after the transport completes.

This is graceful process-local shutdown, not exactly-once external delivery:
the host's five-second forced-abort deadline, broker ambiguity, crash/restart,
and lease expiry still require owner-side fencing and idempotent consumers.
Test code was authored but not run; no compilation or formatting was run.

## Server audit continuation — durable worker shutdown and dispatcher recovery retry (2026-11-23)

The Product Media release loop previously raced its owner batch against a stop
notification using `tokio::select!`. A graceful stop could cancel the batch
after an external Media release but before the owner confirmed absence and
acknowledged its committed queue row. It now checks stop before admitting a
batch and awaits the already admitted batch to completion. Paid-order label
recovery also rechecks stop after waking on its ticker, before starting an
external provider sweep. Both use one server-local admission helper that never
cancels an operation after it starts. Source regressions cover stop-before-start
and stop-during-operation behavior.

Repeated module dispatcher bootstrap used to return immediately when the
required dispatcher was already healthy, so independently reserved optional
Product Media, paid-order label, financial or payment recovery workers could
never be retried after an initially absent dependency. The already-live path
now revisits those idempotent child starters. A focused `mod-product` regression
verifies a pre-published healthy dispatcher can register its missing Product
Media release worker and remains shutdown-drainable.

The graceful-stop behavior does not guarantee process-crash completion or
override the host's bounded forced-abort deadline. Replay, ownership,
cross-replica fencing and external side-effect reconciliation remain the
durable operation owner's responsibility. All added tests are source-only
and unrun by request; no compilation, formatting or runtime checks were done.

## Server audit continuation — scheduler startup liveness (2026-11-23)

The module-work and Workflow cron scheduler initializers previously treated a stored
`JoinHandle` as healthy on repeat bootstrap even if its task had already
terminated. The module-work initializer now uses the shared worker-liveness
guard to fail closed without deleting the handle needed for shutdown accounting.
The Workflow cron initializer applies the same guard and propagates this startup
failure through application bootstrap instead of returning a false success.
Concurrent healthy initializers still reuse the running handle, and host stop
continues to prevent late task publication under each initialization lock.

Focused source regressions create already-finished tasks for both paths and assert
that repeat initialization returns an error while preserving shutdown ownership.
They have not been executed; Cargo, Rust tests, compilation and formatting are
reserved for maintainer verification. This change does not solve cron execution
deduplication across replicas, missed schedules after process crashes, or detached
execution-task supervision; those require the Workflow owner's durable contract.


RBAC durable-generation bootstrap now treats only the absence of its
own `rbac_invalidation_state` table as a pre-migration condition and
cancels pending watchdog initialization on host stop. Module
transition recovery, artifact/data-object garbage collection, and
artifact admission reverification recheck stop between independent
stages and tenant/candidate iterations; an already-started durable
effect is not interrupted by these new checks.


Channel and RBAC cache-invalidation, tenant-cache and tenant-locale
generation, and Redis-status listener starters now cancel pending
**pre-publication** startup work when host shutdown is signaled. This
includes their startup-mutex wait, initial cache/Redis recovery, and
joins of previously aborted workers. A shared stop-watch helper checks
the subscribed state to avoid losing a stop between check and wait;
published task shutdown still uses each listener's own registry gate
and bounded join/abort contract.


Direct worker starters for Product Search channel repair, Notifications
outbox intake/candidate/fanout, Forum Search inbox/contract, Product Index
refresh, Social Graph Index, Comments TCP listener and Comments durable
source-retry audit handoff serialize their final stop-check → task spawn →
JoinHandle publication with the corresponding shutdown handle take,
via a process-local runtime registration lock. Durable broker opening, receipt
backfill and shutdown joins remain outside that lock. Product Search channel
repair also rejects stale/finished published workers and concurrent duplicate
starts; this fence prevents leaked local JoinHandles, not cross-process
duplicate delivery or automatic durable job recovery.


`apps/server` is the main backend composition root. The application:

- assembles platform modules, shared foundation crates, and host-level capabilities into a single runtime;
- publishes HTTP, GraphQL, Leptos `#[server]`, health, metrics, and related control-plane surfaces;
- remains a thin transport/wiring layer where domain logic has already been extracted into module crates.

RBAC persisted relation reads, tenant membership writes, role-mutation fact
collection, exact assignment checks, and active-administrator continuity live
in `rustok-rbac`. Server adapters own authenticated request evidence, user-field
persistence, caller transactions, outbox composition, cache invalidation after
commit, and process telemetry.

Request permission snapshots enter through an authenticated host adapter; HTTP
`RequestContext` consumes upstream trusted tenant/locale/channel/auth extensions.
A valid request snapshot preserves its permission ceiling, and persisted role-user
membership queries delegate to `load_role_user_ids_on`. Request-scope fast paths
also evaluate membership in `RbacService`. After invalidation of a matching
snapshot, its scope-aware permission and role fallbacks now report unavailable
authority instead of using unscoped grants; retry needs a freshly authenticated
scope, and a mutation may already have committed. The GraphQL schema also checks
the matching task-local scope before starting a normal root resolver; nested
fields of an already admitted root are not rechecked. GraphQL role-grant
replacements advance the durable generation transactionally; after commit the
server role writer clears the local cache, invalidates the calling snapshot, and
attempts all-users fan-out. The compatibility role-membership invalidation path
joins both role and user rows to the requested tenant before clearing user
snapshots. These checks do not cover every immutable `AuthContext` consumer,
other tasks or replicas, and do not establish a transaction-spanning revocation
fence. See the
[request-snapshot decision](../../../DECISIONS/2026-11-02-invalidated-rbac-request-scope.md).

Tenant-bound channel REST handlers authorize against the authenticated
`CurrentUser.permissions` snapshot, including the OAuth token ceiling, rather
than looking up broader persisted user grants without a request scope. DLQ
list/replay scope is derived from the canonical top-level outbox envelope tenant
when present, with a nested-event fallback for legacy rows only. Replays repeat
the tenant predicate in the atomic status transition; the preceding read does
not grant permission to update a different tenant's event. The module-owned
Outbox admin counters apply the same canonical-first rule, so a conflicting
legacy nested tenant cannot reveal another tenant's counts.

Artifact purge previews call `ModuleControlPlane` owner projections. Data purge
accepts an exact installation, positive namespace revision, reason, and
idempotency UUID; the authenticated server supplies tenant/actor command
evidence. The host authorizer binds owner-derived context and reads current
persisted `modules:manage` grants on the owner-supplied write transaction
without permission-cache/request snapshots. The policy has no separate database
connection. Terminal replay after lifecycle serialization precedes mutable
owner target reads.
Preview/apply errors hide storage details and receipt integers are checked.
Lifecycle retirement and exact serving-instance collision checks do not prove terminal
traffic/job/write or atomic revocation fences. The server settings recovery
plaintext-tag cipher and permissive policy were deleted. Settings mutations
require a host-composed owner service from `GraphqlRuntimeInputs` and return
unavailable without it; previews disable apply readiness. The owner supports
shared policy/encryption ports, but actual encryption/KMS, secret-handle,
retention, hold, and terminal-fence composition remain missing. Persisted RBAC
relation reading and current tenant-policy evaluation are owned by `rustok-rbac`;
the server retains cache integration, authenticated adapters, and telemetry.
Follow the owner plan and central rollback plan for these
open completion gates.

OAuth translation snapshots use the `rustok-auth` lifecycle enum directly.
Its serialized `active` and `archived` values match persisted owner lifecycle
spelling; server translation queries use SeaQuery's expression trait.

Invite acceptance and user-admin revocation share a transaction-scoped
`auth_invites` row fence. The first committed transition wins; accepted invites
cannot be retroactively revoked, and replayed revocations do not publish another
`UserInviteRevoked` event.

OAuth consent grants and revocations lock the app row and then the subject row,
matching token issuance's lock order before reading or changing consent. This
prevents an exchange from committing a token against consent that was revoked
first, and prevents grants after an app or user becomes inactive. Token
exchange and refresh also re-read the active consent, scope coverage, and grant
epoch inside the issuance transaction after acquiring those locks, so a
concurrent consent re-grant or scope change cannot persist credentials from a
stale preflight snapshot. Manual OAuth
app edits mark lifecycle, credential, identity, and runtime-owned columns
`NotSet`; a stale edit cannot reactivate a revoked client or erase a newer
`last_used_at` value.

## Durable worker bootstrap liveness

A registered runtime handle is not evidence that its task is running. The
notification outbox intake, notification candidate/fanout, Forum Search inbox
and contract consumer, Product Index refresh, and Social Graph Index consumer,
plus the host's Outbox relay/retention, Auth retention, remote executor reaper,
SEO bulk worker and module transition watchdog start paths reject a retained
handle whose Tokio task has terminated. The seven individually claimed consumers
apply this distinction in their initial, contested-reservation, and post-claim
checks. The other six host workers check liveness under the serialized runtime
initializer. A live handle permits idempotent
startup; a missing handle can be claimed once; a finished handle is an explicit
startup error rather than a successful no-op. The retained handle remains
available for shutdown joining and failure accounting.

These are process-local bootstrap/readiness protections, not an automatic
restart loop, a cross-replica leader lease, or proof of exactly-once external
effects. After an unexpected exit, restart the process under its supervisor;
owner-side durable receipts, uncommitted broker offsets, and reconciliation
rules still control which effects may be resumed safely.

## Mandatory platform baseline

For `apps/server`, the mandatory baseline consists of two layers.

Platform `Core` modules:

- `rustok-modules`
- `rustok-auth`
- `rustok-cache`
- `rustok-channel`
- `rustok-email`
- `rustok-index`
- `rustok-outbox`
- `rustok-tenant`
- `rustok-rbac`

Shared foundation / support crates used by the backend platform:

- `rustok-core`
- `rustok-events`
- `rustok-telemetry`
- `rustok-api`
- `rustok-runtime`
- `rustok-web`
- `rustok-fba`
- `rustok-cli-core`

`rustok-modules` is the mandatory module-platform control plane. The server
registers it as a Core module but must not own artifact lifecycle, marketplace
policy or executor selection; sandboxed execution is delegated to the neutral
`rustok-sandbox` foundation.

This is a foundation catalog, not a requirement that the production server binary depends on
every listed crate. `apps/server` should depend only on the crates needed for its active
runtime boundary. For example, Axum response mapping belongs to `rustok-web`, typed runtime
helper extraction belongs to `rustok-runtime` when needed, FBA descriptors belong to
`rustok-fba`, and CLI provider contracts belong to `rustok-cli-core` but command adapters
must stay outside the production HTTP runtime.

`rustok-api` remains the stable contract crate. Executable runtime helpers, Axum boundary
helpers, FBA metadata and CLI provider contracts must live in the dedicated foundation
crates above instead of accumulating in `apps/server` or expanding `rustok-api`.

The tenant-toggle logic applies only to `Optional` modules. `Core` modules should not be treated as switchable by host configuration.

The current generic `SettingsService` and `platform_settings` table are live
migration-target surfaces, not a reference owner model. They mix tenant categories
whose semantic consumers belong to Email, Auth, tenant locale policy, and platform
runtime components, and validation is still partial. Writes now commit settings and
the generic settings-change event in one transaction through `publish_in_tx`, and
`schema_version` is a real optimistic-concurrency revision: GraphQL reads expose
`schemaVersion`, admin updates must send `expectedSchemaVersion`, and stale writes
fail with `CONFLICT` instead of silently overwriting another admin. Revision
concurrency does not turn this generic bag into an owner-specific migration
contract. New owner settings must not extend this bag. Follow the accepted
[Settings and Configuration Architecture](../../../docs/architecture/settings.md)
for the owner cutover, secret handles, activation state, and transactional event
requirements.

Effective module policy remains owner-resolved in `rustok-modules`. When a
request has a channel resolution, the server/channel adapter forwards a
validated `ModuleEffectivePolicyChannelInput` to
`EffectiveModulePolicyService::resolve_for_channel`; channel lookup and
channel-owned storage stay in `rustok-channel`.
Operational maintenance is forwarded through the same context facade as a
revisioned module-scoped or global snapshot; active maintenance denies serving
without changing tenant enablement rows.
Node readiness is forwarded as the same owner context and must observe the
base policy revision before the server accepts the final policy revision.
For dynamic artifact execution, every serving server additionally requires a
stable non-nil `RUSTOK_ARTIFACT_NODE_ID` UUID. The server passes the exact
already-resolved installation to `rustok-modules`' durable observed-assignment
gate before non-lifecycle CAS/sandbox execution. The gate compares payload
kind and admitted media type as well as the digests and revisions. A missing,
degraded, stale, or identity-mismatched assignment denies execution; the
sandbox worker receives only the scoped execution request and does not gain
database, module-policy, AI, or product access. The runtime uses a bounded
digest-keyed verified CAS cache (`RUSTOK_ARTIFACT_NODE_CACHE_MAX_BYTES`, default
64 MiB) and rehashes every hit before any sandbox request. The authenticated
agent transport is deployed independently through
`rustok-artifact-node-controller`; deployment topology and agent materialization
remain separate unfinished control-plane work.

## REST transport correctness

The legacy commerce `ApiErrorResponse` adapter returns actionable client HTTP
statuses for invalid price/options/variants (400), missing products, variants,
or shipping profiles (404), and duplicate identifiers or shipping-profile
slugs (409). Server-side commerce failures are logged internally; their
HTTP 500 payloads contain no raw database/provider diagnostics. Wrapped
`RichError` categories retain meaningful HTTP status codes, but only the
explicitly declared safe `user_message` is returned to clients; internal
message text and structured metadata remain private.

The shared `rustok-web::ErrorBody` also carries optional exact owner `retryable` advice.
`Error::Http` delegates that body/status unchanged, including explicit false; other host errors
without a declared owner decision omit it rather than classify retryability from HTTP status.
The canonical DTO derives its OpenAPI schema (optional non-nullable boolean); Commerce error
references reuse it. Advice is not durable outcome or replay/key authority. Two new host
error-delegation functions and one composed-OpenAPI regression function are unrun; see the [HTTP contract](../../../docs/architecture/api.md#http-error-retry-advice).

`GET /api/users?search=...` matches user-supplied percent signs, underscores,
and escape characters literally, rather than interpreting them as SQL LIKE
wildcards. Pages past the result count return an empty `users` array without
issuing an oversized OFFSET query.

REST and cookie-session login audit the effective client IP using the host's
configured trusted-proxy chain. Untrusted forwarding headers cannot override
the observed peer address.

Production CORS authorization has no implicit loopback fallback.
`RUSTOK_HOST` and `APP_HOST` are bind-host hints, not browser-origin grants,
and are used as a fallback only in development. Without a valid
`RUSTOK_CORS_ALLOWED_ORIGINS` or `/rustok/cors/allowed_origins` list,
the production CORS layer emits no `Access-Control-Allow-Origin` header,
including for `http://127.0.0.1:5150`. Explicit origins must be valid
HTTP(S) origins without URL credentials, paths, queries or fragments;
invalid configuration entries are ignored with a bounded error code.
Unspecified bind addresses (IPv4 `0.0.0.0` and IPv6 `[::]`) are
rejected even when supplied as explicit CORS entries: they are network
listeners, not browser-authorized origins.
This changes only cross-origin browser permission; it is not an HTTP
authorization or same-origin request gate.

The module event dispatcher and its four startup-owned recovery workers
(paid-order labels, marketplace financial, payment-provider events and
product-media release) now share one per-runtime synchronous registration
lock with their corresponding shutdown handle snapshots. A direct
`spawn_module_event_dispatcher` caller can no longer publish a new
dispatcher after the stop signal has been issued and shutdown has
already inspected its handles. The stop condition is checked under
the same lock that guards dispatcher start, publication and take.
Concurrently started dispatchers and child workers remain protected by
typed startup reservations. The product-media starter now also uses a
drop-safe reservation, releasing a rejected attempt instead of leaving
a permanent no-handle marker. No worker restarts or event-delivery
contracts are changed.

The in-process `EventBus` forwarder and `TransactionalEventBus`
now obtain their `Arc<dyn EventTransport>` from one atomic,
process-local runtime singleton resolver. A caller that loses a
first-insert race cannot continue writing through its discarded
default `OutboxTransport` while other publishers use the winning
configured transport. Explicitly preinstalled transports remain
authoritative; the change does not add retries or alter the selected
delivery profile.

Tenant resolution HTTP failures now return bounded static client
messages. The status and error code still distinguish missing/conflicting
tenant headers, invalid hosts, invalid identifiers, base-domain slug
requirements and not-found outcomes, but no request-supplied Host, tenant
slug, header name or private validator detail is interpolated into the
response body. This mirrors the already sanitized tenant-context load
error surface and leaves internal typed errors intact.

The HTTP root trace span now rejects malformed or oversized
client-supplied `x-request-id`, `x-correlation-id` and W3C
`traceparent` values. Only canonical UUID/32-hex correlation
identifiers and valid fixed-format W3C version-00 trace context appear
in logs; all others render as `-`. Unrecognized HTTP extension
methods are reported as `OTHER`, preventing user-controlled method
bytes from entering traces. This does not rewrite the original
headers or change downstream request-ID propagation.

HTTP edge tracing and middleware diagnostics now emit only a fixed
`path_group` (for example `/api/:request`, `/admin/:request`,
`/graphql`, or `/health/ready`) rather than the raw URI pathname. URL
segments can contain user and token identifiers, so route-specific metrics
continue to use Axum's matched template separately after routing; the
trace layer cannot see the matched template before routing. The change also
covers metrics auth, both rate-limit entry points, tenant resolution,
auth-context, channel, locale, security headers and registry artifact/publish
authorization. All those middleware use the shared bounded path vocabulary;
custom methods in registry diagnostics use the bounded method vocabulary.
The original request pathname still drives routing, security-header
selection, request authorization, tenant/locale policy and rate-limit matching.

HTTP edge settings are bounded before reaching Tokio and Tower:
`RUSTOK_HTTP_REQUEST_TIMEOUT_SECONDS` defaults to 30 seconds and is capped at
600; `RUSTOK_HTTP_UPLOAD_TIMEOUT_SECONDS` defaults to 300 seconds and is capped
at 3,600; `RUSTOK_HTTP_CONCURRENCY_LIMIT` defaults to 512 and is capped at
16,384. Zero or malformed values use the defaults, while oversized values are
clamped with a warning. Only exact `multipart/form-data` requests to
`/api/media` and PUT requests to the registry artifact route receive the
upload timeout budget. Active rate-limit windows are normalized to 1 second
through 365 days for both memory and Redis backends; disabled limiters remain
disabled. Redis rate-limit counters without an expiry are repaired in the
atomic script; if an expiry still cannot be established, the request fails
closed with backend unavailable instead of leaving a permanent counter lockout.
Redis uses millisecond TTLs, and memory/Redis reset headers round up to whole
seconds so clients are not instructed to retry before the window actually ends.

Database pool settings are validated before connecting: an omitted
`max_connections` uses the pool default of 10, explicit values may not exceed
1,024, and `min_connections` may not exceed the effective maximum. Database
`connect_timeout` is capped at 300,000 ms and `idle_timeout` at 86,400,000 ms.
Out-of-range values fail startup instead of creating unbounded pool limits or
timeouts.

OAuth browser-consent forms preserve absent optional `scope` and `state` values
across GET-to-POST submission, so CSRF-bound data and default requested scopes
do not diverge. OAuth protocol `server_error` and `temporarily_unavailable`
are emitted as HTTP 500/503, rather than misleading client-side 400 responses.
OAuth client database lookup failures also become sanitized HTTP 500 errors, not
misleading `invalid_client` / HTTP 401 responses.
All `/api/oauth/*` responses, including consent HTML, authorization redirects,
userinfo, and failures, carry `Cache-Control: no-store` and `Pragma: no-cache`.

Cookie-session login, registration and refresh use the remaining persisted
refresh-session lifetime for cookie Max-Age. The absolute 90-day lineage cap
is not extended when a session is rotated, and unrepresentable persisted
timestamps fail closed rather than panicking.

Cookie-session logout treats a missing, expired, or already-revoked refresh
token idempotently, but returns a failure when revocation persistence or
transactional event publication fails. The server must not claim logout
success while a refresh token could remain valid. Session-id logout now locks
the active row before building its revocation update (PostgreSQL/MySQL row lock;
SQLite write fence before read), so a concurrent refresh cannot be overwritten
by a stale ActiveModel snapshot. A cookie containing the
immediately previous refresh token (for example, when a rotation response
was lost) also revokes its still-live successor session under the same row
lock. Refresh-reuse detection also resolves the immediately previous
digest under that same backend-specific row lock before revoking the session,
so a stale ActiveModel snapshot cannot overwrite a concurrent rotation. A replay
inside the 30-second race window is rejected without revoking the live session,
but this is not an idempotent response replay: a lost successful response still
cannot be recovered from the old cookie. Older generations are not represented
in the in-place session row and remain an open incident-detection gap. Multiple
cookies with the
`rustok-session` name are rejected, including across repeated Cookie headers,
rather than selecting an order-dependent token. First-party session validity,
new issuance and refresh rotation enforce the original 90-day absolute expiry
(including legacy rows without an explicit deadline). Auth lifecycle mutation
lookups reserve SQLite's write lock with a predicate-matched no-op update before
reading the user/session row; PostgreSQL/MySQL use row-level FOR UPDATE.
Login, profile changes, password changes, refresh rotation, reuse revocation,
and both logout paths therefore do not rely on SQLite's unsupported row-lock
syntax or a deferred read-to-write upgrade. OAuth token issuance also reserves
the SQLite writer by known client/subject IDs before reloading current state;
OAuth admin mutations and admin user mutations use the same write-first fence
by tenant/app or tenant/user ID.
Failed dummy Argon2 hash
initialization propagates as an internal auth error rather than substituting
an empty hash.

OAuth refresh requests may retain the original scopes by omitting `scope` or
request a non-empty subset of the currently authorized scopes. Authorization
and client-credentials requests also preserve scope presence: omission selects
the configured default, while an explicitly empty or whitespace-only value
fails with `invalid_scope`. Consent grants must contain at least one scope.
The `refresh_token` grant is valid only when the app also declares
`authorization_code`. The server persists narrowed scopes on rotated
credentials. Reuse detection revokes the scoped OAuth token family; on a
persistence error it returns `server_error` rather than swallowing the failed
security effect. Automatic durable retry after the client stops retrying is
not established. The 30-second reuse grace suppresses family revocation
for a likely concurrent duplicate, but the request still receives `invalid_grant`;
the replacement token is not replayed, so a lost successful response cannot
be recovered from the old refresh token.

GraphQL request-preparation policies classify only the operation selected by
`operationName` (or the sole operation when no name is required). The selected
operation is an execution selector, not an authorization grant: field permissions
remain enforced by server-owned policy. Fragment-spread classification is
cycle-safe because host preflight runs before GraphQL's normal document validation.

## Runtime surface

- `/api/graphql` and `/api/fn/*` are parallel transport layers; Leptos server functions do not replace the GraphQL API.
- `/api/graphql/schema.graphql` publishes the assembled SDL schema without tenant context for contract tooling; full introspection is exported via POST request to `/api/graphql`. Both snapshots are part of reference artifacts alongside OpenAPI JSON/YAML.
- Embedded UI is no longer considered an unconditional part of the backend binary: `rustok-admin` and `rustok-storefront` are linked only with compile-time feature flags `embed-admin` / `embed-storefront`, not merely by their presence in the workspace.
- Commerce OpenAPI/REST surface on `/admin/*` now includes the first post-order refund contract built on top of `payment-collections`; the host publishes these routes, but the refund lifecycle remains domain-owned in `rustok-payment` and `rustok-commerce`.
- Guest-cart HTTP capability parsing and response emission belong to
  `rustok-cart::guest_access_http`; the server only composes that owner adapter.
- Commerce surface is no longer a compile-time baseline for any server build: `controllers::commerce`, commerce-specific error mapping, and the commerce fragment in OpenAPI live only with `mod-commerce`, so a reduced/headless host can build without the ecommerce transport layer.
- Content REST/OpenAPI surface for `blog`, `forum`, and `pages` is also no longer an unconditional part of the host binary: the corresponding server controllers and OpenAPI fragments are included only with `mod-blog`, `mod-forum`, and `mod-pages`, so a module-sliced build does not have to pull in other content transport dependencies.
- `flex` attached field-definition and standalone schemas/entries GraphQL are published via `/api/graphql`, while standalone REST remains at `/api/v1/flex/schemas*` when `mod-flex` is composed; disabled builds do not mount this REST surface and do not advertise its paths in the generated OpenAPI document. This is a live tenant-scoped surface with separate `flex_schemas:*` and `flex_entries:*` permission gates. GraphQL query/mutation roots, runtime handle, and DTO belong to `flex::graphql`; roots are included in the schema via `[provides.graphql]` manifest codegen, and the server builder registers only `FlexGraphqlRuntime` on top of concrete `FlexStandaloneSeaOrmService`, `FieldDefRegistry`, DB handle, and cache adapter. REST request/response DTO, command mapping, and view mapping belong to `flex::rest`; attached field-definition row-to-core/view/command mapping, create guardrails, persisted JSON shape helpers, persisted type-name normalization, lifecycle events, and cache invalidation event taxonomy belong to `flex::registry`; the server is the Axum/SeaORM adapter.
- Health/observability surface is published via `/health*` and `/metrics`.
- Module/runtime wiring relies on `modules.toml`, `rustok-module.toml`, and generated host integration.
- Optional module REST/GraphQL surfaces are mounted only from owner-owned crate entrypoints,
  declared in `rustok-module.toml` (`provides.http`, `provides.graphql`) and `modules.toml`.
  OpenAPI fragments for optional modules also live in owner crates and are merged by the server
  as ready-made documents, without enumerating module-owned handlers/DTO in `apps/server`.
  `apps/server/src/controllers/<module>` and `apps/server/src/graphql/<module>` are not
  valid composition points for optional modules; the source guard `module_surface_boundary_guard`
  prevents the return of server-owned shims.
- Shared content canonical query and cross-module conversion mutations also arrive as ready-made
  GraphQL roots from `rustok-content` and `rustok-content-orchestration`; the host does not own their resolver/DTO.
- `rustok-content-orchestration` is registered by the host as `SharedContentOrchestrationService`,
  constructed from explicit DB and `TransactionalEventBus`; conversion GraphQL resolvers read this handle from schema data,
  not through framework-owned application context.
- Auth lifecycle and OAuth GraphQL query/mutation/types belong to `rustok-auth`; auth, OAuth and users REST request/response DTOs and OpenAPI schema derives also belong to `rustok-auth::rest`. The server implements only `AuthLifecyclePort`/`OAuthAdminPort` on top of persisted lifecycle/OAuth/email services, registers the corresponding runtimes in shared runtime extensions, and keeps auth/OAuth/users HTTP controllers as route/extractor/response adapters that re-export or import owner DTOs for Swagger and route compatibility. `AuthLifecycleService` accepts only `ServerRuntimeContext` and an explicit `AuthConfig`. `ServerAuthLifecycleProvider` receives explicit `ServerRuntimeContext`, `AuthConfig`, and mailer handle; `CurrentUser`/`OptionalCurrentUser`, `auth_context` middleware, and RBAC permission extractors use a narrow `ServerAuthRuntime`.
  Auth lifecycle GraphQL/native self-service operations consume the canonical `AuthPrincipalKind`; direct-session operations fail closed for delegated OAuth and service principals instead of inferring direct-user status from `session_id` presence.
- With `mod-ai`, the server composes the AI query, mutation, subscription, and schema data factory
  from the AI package manifest. AI is a required runtime extension in distributions that include
  it. Its runtime registration stays with the distribution. The schema data factory builds the
  AI runtime from the host context, including deployment-selected owner read ports for checkout
  status and Product catalog projection. The Leptos admin adapter uses the same AI runtime factory.
- MCP GraphQL query/mutation/types and REST/control-plane DTOs belong to `rustok-mcp`; the server implements `McpManagementPort` on top of persisted `McpManagementService`, registers `McpManagementRuntime`, and keeps HTTP controllers as Axum adapters that import owner DTOs and actor parsing.
- Content GraphQL dataloaders for `nodes`, `node_translations`, and `bodies` live in
  `rustok-content`; `apps/server` only registers owner-owned loader types in the schema builder.
- System GraphQL may publish media usage, but reads it through `rustok-media::load_media_usage_snapshot`,
  without directly importing module-owned media entities.
- Settings and system GraphQL resolvers receive `ServerRuntimeContext`, DB, and schema-owned
  `TransactionalEventBus` through GraphQL data. They do not extract or adapt a framework-global
  application context.
- Module event dispatcher startup requires `EventRuntime` before launching any
  dependent Product Media release, Commerce paid-order label, marketplace financial,
  or Payment provider recovery workers. A missing event runtime fails bootstrap
  without leaving detached module-owned recovery tasks running. An atomic
  per-runtime reservation prevents concurrent dispatcher startup from replacing
  its shutdown handle. The required-listener flag is set only after the
  dispatcher starts successfully; optional module recovery loops start only
  after that success, so a failed required dispatcher cannot leave those loops
  running. Product-media, paid-order-label, marketplace-financial and payment
  provider worker admission each uses its own atomic reservation, preventing
  concurrent composition from spawning and losing duplicate worker handles.
  Missing optional dependencies release the corresponding reservation for a
  later retry; already stopping hosts do not admit new recovery workers.
  These are **process-local** startup fences, not cross-replica singleton
  leases or a replacement for owner-level idempotency and durable recovery.
  Concurrent module-dispatcher callers now distinguish a **published, live**
  dispatcher plus its required-listener flag from an in-flight initialization.
  Contention before publication returns an explicit error to the bootstrap
  caller rather than a false successful start, and an unexpectedly finished
  dispatcher is not reported as healthy. Dispatcher and dependent paid-order
  label, marketplace-financial and payment-event startup claims are managed
  by cancellation/unwind-safe RAII guards. A failed or panicking preparation
  releases its claim for retry; a published handle retains ownership for
  the process lifetime. This is not automatic restart after a worker exits.
- Event runtime construction completes fallible transport, projector, and relay
  configuration work before it adds tenant-generation or Redis-status listener
  tasks. The RBAC invalidation listener may already be started by early server
  initialization; it remains registered in the shared runtime and is drained if
  later event-runtime validation fails.
- The HTTP server bootstrap is serialized per `ServerRuntimeContext` and shares
  its initialization fence with shutdown. Shutdown signals workers first, then
  waits for bootstrap to finish registering handles before taking the drain
  snapshot, so concurrent bootstrap cannot orphan newly spawned workers.
  `connect_runtime_workers_with_runtime` can also be called directly, without
  this router-bootstrap mutex. Its separate artifact/runtime-worker startup
  mutex now fences shutdown's handle snapshot too: shutdown waits for the
  direct initializer to finish before taking any worker handle. The mutex is
  released *before* concurrent task drains, preventing a lock cycle with
  artifact maintenance's own shutdown drain. New startup calls recheck the
  signaled stop channel after acquiring the mutex and cannot admit more work.
  The barrier prevents a live but detached late `JoinHandle`; it does not
  provide a process-crash shutdown guarantee.
- Once started, the opt-in Product Index, Social Graph Index, and Forum Search
  contract consumers are required readiness workers. Their durable broker
  offsets remain uncommitted on terminal delivery failure; the readiness probe
  therefore turns unhealthy if a consumer exits instead of leaving the HTTP
  host green while its projection queue is stalled.
- Forum Search inbox reconciliation and the opt-in Notification outbox-intake,
  fanout, and candidate workers are also required readiness workers once started.
  A dead queue/reconciliation task cannot leave the HTTP host green while its
  durable work remains unprocessed.
- Notification candidate, fanout, Outbox intake, Forum Search inbox,
  Forum Search contract, Product Index refresh and Social Graph Index worker
  startup have distinct process-local atomic lifecycle claims in
  `ServerRuntimeContext`. A second caller cannot spawn a duplicate worker,
  overwrite the only shutdown `JoinHandle`, open a duplicate Iggy consumer
  group, or run concurrent Outbox registration/backfill while the first
  startup is still in progress. A contending starter sees a retryable
  startup-in-progress error, never a false `Ok` while no worker exists yet;
  a caller that observes the already-published worker handle succeeds
  idempotently. Claims are released on preparation error and async cancellation,
  and retained after successful handle publication. All seven starters check
  the shared stop signal before publishing a worker. This is an
  in-process guarantee, not distributed leader election; durable queue
  ownership/receipt correctness remains with the Outbox, Search, Index,
  and Notifications domain workers. A previously published live handle no
  longer makes repeated startup appear successful after the shared shutdown
  signal is set; the handle stays registered for draining.
- Tenant cache-generation recovery also invalidates the entire
  `ModuleEffectivePolicyCache` when its durable generation advances or an
  unverified generation is first observed. The local/Redis/periodic listener
  paths now serialize checkpoint and cache effects; an in-order event
  acknowledges only after its local policy and backend invalidations.
  Missed Pub/Sub generations are not reduced to the single tenant key of
  a later event; unexpected key formats conservatively evict all cached
  policies rather than acknowledging an ineffective invalidation.
  The canonical `*` key retains its intended global-invalidation
  meaning; a nil UUID is not accepted as a tenant identity.
- Tenant locale generation recovery now serializes local invalidation
  messages, Redis Pub/Sub delivery and periodic reconciliation under one
  per-listener asynchronous transition gate. The shared generation tracker
  and readiness bit cannot be updated by interleaved older recovery results.
  Cloned listener task owners reuse the same gate; this is process-local and
  does not replace durable namespace generation checks.
- The global Event DLQ physical duplicate-alert scanner and identifier-free
  telemetry companion share a shutdown-aware registration lock. Their
  `shared_contains` / `shared_insert` race could spawn duplicate tasks,
  overwrite the only shutdown `JoinHandle`, or publish a new task after the
  shutdown snapshot. Repeated startup of a terminated task now emits a stable
  error diagnostic and retains the old handle for shutdown accounting; modes
  without tasks (`Disabled`, `Unavailable`, `NotApplicableOutbox`) remain
  idempotent and do not impede event delivery.
- Social Graph Index position and poison-receipt observers have independent
  single-owner startup reservations and a shared admission lock that the
  shutdown drain also acquires before snapshotting observer JoinHandles.
  Concurrent direct/bootstrap callers cannot spawn duplicate observers,
  publish after shutdown's snapshot, or overwrite their only shutdown handle.
  A retained handle whose task exited is an explicit repeated-start failure,
  not a false `Ok(())`; a contending caller receives an in-progress error
  until publication. Both observers reject startup after shutdown begins,
  and optional invalid poll configuration still leaves projection active.
- Product Index refresh, Social Graph Index and Forum Search typed broker
  consumers check shutdown again **after** broker receive completes but before
  invoking projection, semantic decode handling or poison DLQ publication.
  `tokio::select!` can choose a delivery when the stop signal becomes ready at
  the same time; the recheck prevents a new durable effect from being admitted.
  A delivery returned during shutdown is not acknowledged, so its persistent
  broker offset remains available for redelivery after restart.
- Social Graph Index and Forum Search poison DLQ publishers treat an external
  send error as **ambiguous**: the broker may already have accepted it.
  The workers retain the durable `publishing` receipt and leave the source
  offset uncommitted. Connector receipt storage prohibits automatic reclaim
  of a `publishing` receipt after its lease expires, reporting
  `ReconciliationRequired`. Redelivery of a previously confirmed `published`
  result can acknowledge its source without a second DLQ send. A late
  confirmation from the original claim owner can still mark it published.
  Unresolved sends after a crash require an explicit, evidence-based operator
  reconciliation; this patch does not supply a recovery API or guarantee
  exactly-once broker effects. The stable Iggy message ID alone is not proof
  of suppression of all duplicates.
- The Comments schedule audit source-retry handoff checks the shared stop
  signal before **each** expired-attempt dead-letter transition and each new
  claim, not only at the outer polling boundary. Exhausted claims left by a
  crash are now swept in bounded pages (up to the configured
  `max_claims_per_cycle` per cycle, independently of the claim budget), rather
  than one row per idle poll. A full sweep schedules an immediate bounded
  follow-up cycle; failed sweeps retain the configured error backoff. An
  already acquired claim completes its canonical outbox transaction and
  failure-state update before the next shutdown check; receipt identity,
  lease expiry, and PostgreSQL row fences remain authoritative. Both public
  and worker-specific failure transitions now require an unexpired matching
  claim token and attempt, so an expired worker cannot reschedule a row
  merely because no replacement has claimed it yet.
  The direct Comments audit handoff claim/publish helpers now require the
  caller's bounded `max_attempts` and use the same retry-ready SQL as the
  worker; a deferred or source-dead-letter record cannot be claimed via the
  former unrestricted shortcut. Publication additionally checks the dead-letter
  fence in its row lock and final CAS, so manual recovery must requeue the
  source row through its audited recovery API before a new publication attempt.
- Notification candidate and fanout workers recheck shutdown **after**
  asynchronous tenant capability/policy evaluation (including deferrals),
  immediately before starting their next candidate processing, source
  materialization or fanout job. A policy lookup begun before shutdown is not
  permission to admit a new durable effect afterward.
- Marketplace financial recovery runs provider-reversal adaptation, reversal
  event reconciliation and paid-order event reconciliation in that order.
  During cooperative shutdown it finishes the currently running durable
  operation but **does not start a subsequent phase** after the stop signal
  becomes visible; the next startup resumes outstanding durable work.
  Individual operations can still block until their own completion or the
  bounded host shutdown deadline.
- Notification intake registers its supported event types in the Outbox-owned
  consumer ledger before runtime retention starts. Accepted receipts and
  permanent rejections acknowledge the original event in the same transaction;
  unacknowledged events remain retained through intake outages. Registration is
  durable, so disabling the worker does not release an existing retention hold.
  Its initial legacy-receipt backfill checks the host stop signal before every
  page and receipt and aborts startup if shutdown begins; already acknowledged
  receipts are safe for replay during a later startup. In-flight DB operations
  still run to completion before the next stop check.
- Auth retention cleanup stays transactional across all artifact tables.
  A successful run keeps its daily cadence; a failed purge retries after
  15 minutes instead of deferring every affected cleanup for another full day.
  The failure log is a fixed machine code, not a raw database error.
- Product Index refresh consumer startup returns a typed server error if
  module runtime extensions were not published; a partial bootstrap no longer
  panics before its durable consumer group is opened.
- Outbox retention keeps its configured successful sweep cadence and the
  50-ms bounded follow-up for a full batch. A database failure now retries
  after at most 60 seconds, rather than inheriting a potentially daily
  successful-poll delay. The existing failure counter remains observable.
- Outbox relay polling, claim leases, retry backoff, and outbox-retention
  polling are each capped at one day. Relay batches are capped at 10,000 events,
  in-flight delivery concurrency at 128, and in-memory event channel capacity
  at 65,536. Invalid relay/retention resource or timing configuration fails
  validation instead of delaying recovery for unbounded periods, wrapping retry
  delays, or allowing unbounded allocation.
- App runtime rate-limit bootstrap and shared limiter registration use `ServerRuntimeContext`.
  In-memory fixed-window counters sample time inside the serialized per-key update;
  a request delayed behind another cannot apply an obsolete window timestamp.
  Exactly at a window deadline, its counter restarts rather than extending the
  previous window by one tick.
- Built-in password-reset and verification email sends register active Tokio
  tasks in a single host-runtime registry, independent of HTTP callers retaining
  their JoinHandles. Host shutdown closes email admission, waits for accepted
  sends for up to five seconds, then aborts and observes remaining tasks within
  another bounded second. Panics during future construction and delivery are
  caught and logged without exposing credentials. Concurrent SMTP sender
  initialization preserves the first shared connection pool. Skipped sends on
  shutdown are counted as skipped deliveries; templates match the full locale
  language subtag rather than an arbitrary `ru` prefix. This improves graceful
  shutdown **only**: email delivery remains best-effort, without a durable
  queue, retry lease, or crash recovery.
- `host::run` owns YAML configuration loading, database connection and bounded worker shutdown for the pure Axum executable, including ordinary startup and bind failures; `services::server_bootstrap` owns startup validation, default-superadmin initialization, runtime/worker setup and router composition. If the host lifecycle future is cancelled or unwinds, its drop guard schedules worker shutdown on the active Tokio runtime; normal completion awaits the bounded drain. The optional Comments TCP listener and schedule-audit handoff worker use cancellation-safe startup reservations, reject a concurrent in-flight start rather than reporting false success, recheck shutdown before task admission, and do not treat an already-exited task as live. Rate-limit cleanup tasks, the optional Comments TCP listener and schedule-audit handoff worker, DLQ duplicate-alert tasks, Product channel/index workers, Social Graph Index worker/observers, Tenant cache/locale-generation listeners, Cache Redis and field-definition cache tasks, SEO redirect reconciliation, Channel/RBAC invalidation listeners, the RBAC generation watchdog, the Event forwarder, and both Forum Search workers are owned and joined/aborted by that drain. HTTP installer jobs are also held in a runtime-owned `JoinSet` and drained
before the event forwarder. The Outbox relay supervisor owns and gracefully drains its inner processing task before the host's bounded forced-abort fallback. This cannot guarantee cleanup after process abort/runtime destruction, and lower-level bootstrap callers still own their shutdown lifecycle.
- Database selection is explicit: `DATABASE_URL` overrides `database.uri`; otherwise the configured URI is used as-is. The host never switches a PostgreSQL URI to SQLite based on its hostname. Configure `sqlite://...` directly when SQLite is intended.
- Default SuperAdmin provisioning runs only in `Full`, `Api`, and `AdminSsr` host modes that own administrative authentication surfaces. `StorefrontSsr`, `Worker`, and `RegistryOnly` replicas do not perform this privileged auth-state mutation.
- Background-worker hosts require initialized durable storage before startup because module artifact maintenance is a required worker; missing storage fails bootstrap instead of leaving readiness green with required garbage collection disabled. The optional admission-reverification leg may be omitted only by leaving `RUSTOK_MODULE_MAINTENANCE_VERIFICATION_ENDPOINT` unset. Once the endpoint is configured, invalid endpoint, mTLS, actor, or policy-revision settings abort worker bootstrap with a typed startup error instead of silently disabling admission reverification. A temporarily unreachable verifier remains a retryable per-pass runtime failure.
- Tenant cache invalidation helpers return a typed `Result`; callers must handle generation rotation/publication failures rather than treating a logged warning as successful invalidation.
- Auth-middleware early rejections use the shared JSON error envelope (with internal details redacted), and server-side request-budget expiry maps to HTTP `504` with the canonical JSON timeout error.
- Build-time runtime-extension wiring reads the canonical `[crate].entry_type` manifest field and fails when an extension omits it; it does not infer Rust types by scanning source text.
- Auth host configuration parsing is fail-closed: malformed `settings.auth` input is returned as a startup error rather than being silently replaced by default auth overrides.
- GraphQL/native password-reset requests keep the public result generic when token/email/URL preparation fails after an account lookup, so mail/configuration failures cannot disclose account existence.
- Auth lifecycle provider internal failures are logged on the server and exposed to lifecycle transports only as the stable `Auth lifecycle operation failed` error; database/provider diagnostic strings are not sent through GraphQL error messages.
- User administration uses a transaction-scoped user lock fence; SQLite rereads the row after its write fence, custom-field writes reject stale metadata snapshots, OAuth admin reads are bounded to `1..=100`, and committed user mutations build their response projection before commit so post-commit RBAC reads cannot turn a successful mutation into a client-visible failure.
- When token issuance succeeds but the provider cannot build the token-response projection, the freshly issued session is compensated by revocation using the signed access-token session claim; the token bundle is never returned to the caller.
  Alloy runtime bootstrap also registers `SharedAlloyRuntime` via `ServerRuntimeContext` from an explicit DB handle,
  and Alloy GraphQL receives this runtime as schema-owned data without a framework-global context.
- User complex fields and build progress subscription use schema-owned `DatabaseConnection`
  directly and do not depend on a framework-global host context.
- Host-owned `RootQuery` also does not extract a framework-global context: DB-only read paths use
  schema-owned `DatabaseConnection`, and marketplace/cache paths use `ServerRuntimeContext`.
- Build history and active-build GraphQL/native transports use the read-only
  host-composed `rustok_build::SharedBuildControl`; transports do not construct
  `BuildService` directly. The
  control returns typed framework-neutral `rustok-api` snapshots, so GraphQL
  only wraps canonical facts and does not map SeaORM models.
- The build progress WebSocket at `GET /ws/builds` authenticates its bearer
  credential and admits only effective `modules:read`, `modules:list`, or
  `modules:manage` permissions. It uses the same live authority lease as the
  GraphQL WebSocket: it checks the tenant, token, session, and effective RBAC
  snapshot on a 30-second interval and before forwarding an event; authority
  lookup is bounded to five seconds and the remaining token lifetime. An expiry
  deadline starts a bounded close; outbound writes are bounded by five seconds
  and the remaining token lifetime.
  Authority changes attempt a 4401 close; a blocked or disconnected peer may see
  only a dropped connection. The hub is an
  atomic process-local singleton, not a distributed or durable event stream.
- Effective-module-policy snapshots carry an owner-produced tenant/revision
  cache identity, but the server's `get_latest` path does not compare it to an
  independently read current owner revision. The cache returns only policy;
  callers needing policy with defaults use an uncached owner resolution, which
  passes the same manifest defaults into policy construction. The underlying
  database reads are not a single transactional snapshot. Local observed
  invalidations fence in-flight fills; missed or delayed events, database races
  and volatile executor inputs remain freshness gaps. TTL and process generation
  are not authorization guarantees, and dispatch checks do not cover earlier
  guard/RBAC decisions.
- Artifact binding dispatch now requires policy facts for the exact artifact
  release and installation, both at admission and at non-lifecycle runtime
  revalidation; a static module sharing its slug is not sufficient. The current
  server resolver builds a static-only policy, so artifact bindings using this
  resolver fail closed until an artifact-aware catalog and executor inputs are
  wired into the owner policy path. Node readiness and dispatch checks do not
  turn that static policy into artifact evidence. Lifecycle hooks have their
  separate lifecycle admission path.
- Production release admission, rollout, activation, recovery, and related
  events are owned by `rustok-modules`; build events contain build facts only.
- All of `apps/server/src/graphql/**`, including `RootMutation`, RBAC writer, and search rate limiter,
  accepts explicit runtime dependencies. `services/graphql_schema.rs` also accepts only
  `ServerRuntimeContext`.
- Server event runtime builds the regular and transactional event bus from `ServerRuntimeContext`; `rustok-outbox`
  exposes no framework-specific composition adapter.
- GraphQL HTTP and WebSocket handlers extract `ServerRuntimeContext`/`ServerAuthRuntime` as Axum
  substate and do not pass framework context into request/connection data.
- GraphQL WebSocket transport requires explicit subprotocol negotiation, bounds frames/messages and its incoming queue, caps active operation ids (32, 128-byte ids; duplicate 4409, overflow 4429, invalid id 4400), and admits operations as the protocol engine consumes the queue rather than when the socket receives them. Slots are released on engine `complete` before the engine consumes subsequent input. The `graphql-transport-ws` surface suppresses the engine's completion response to an active client cancellation, while legacy `graphql-ws` forwards it. Messages already in flight before cancellation can still arrive; immediate ID reuse cannot disambiguate those messages. It revalidates the connection lease on a 30-second interval and before delivery, schedules closure at signed access-token expiry, bounds outbound writes to five seconds or the remaining token lifetime, fails closed with a 10-second `connection_init` wait (close code 4408) and a separate 15-second initialization-validation limit, and never logs the raw tenant slug from the initialization payload. Operation cancellation/id-reuse behavior depends on the locked GraphQL protocol engine's completion ordering and must be exercised by the protocol regression tests when upgrading it.

- Users REST handlers also extract `ServerRuntimeContext` and use `rustok_web::json_response`
  for JSON response formatting.
- Metrics handler and the entire metrics helper pipeline use `ServerRuntimeContext`; the mailer
  state is passed via a separate `ServerEmailRuntime`, and worker handles are read through a scoped shared API.
- `settings.rustok.features.oauth_enabled` is a real composition gate. When it is false the host does
  not mount `/api/oauth/*` or the OAuth discovery metadata, and OpenAPI prunes the OAuth paths. The
  default is fail-closed; deployments that expose an authorization server must opt in explicitly.
- OAuth discovery is served at the exact root paths `/.well-known/openid-configuration` and
  `/.well-known/oauth-authorization-server`. These host-wide metadata endpoints are public and
  intentionally bypass tenant resolution; tenant-scoped OAuth operations remain under `/api/oauth/*`.
- Auth retention purges expired OAuth authorization codes on the same 120-day outer window as OAuth
  token families, so a used code remains available as the replay anchor for the delegated lineage it
  issued while still bounding storage. Session, invite, token, and code cleanup now share one
  database transaction: a later deletion failure rolls back the earlier deletions, and the purge
  counts describe only a committed sweep.
- Health readiness/runtime handlers use the same runtime contracts for DB, settings, cache,
  event transport, rate limits, worker lifecycle, and email backend state; JSON response formatting
  goes through `rustok_web::json_response`.
- Channel and standalone Flex REST handlers receive `ServerRuntimeContext`; channel JSON response
  formatting goes through `rustok_web::json_response`, and Flex controller tests assemble the same
  neutral runtime fixture.
- Auth REST handlers extract a narrow `ServerAuthRuntime`; password reset and verification endpoints
  additionally receive `ServerEmailRuntime`. The controller receives explicit config, DB, and mailer
  dependencies, and JSON response formatting goes through `rustok_web::json_response`.
- Module guard and server channel contract are typed through `ServerRuntimeContext`; no
  framework-global application context is a request/channel contract for server-owned runtime paths.
- OAuth discovery metadata also uses `ServerAuthRuntime` as the single source of auth config.
- OAuth REST token, authorize/consent, browser-session, and revoke handlers extract `ServerAuthRuntime`
  or `ServerRuntimeContext`; host-wide context does not participate in OAuth request state.
- OAuth access JWTs are registered credentials: issuance writes a `token_kind = access`
  row whose id is the JWT `jti`, and the extractor requires that row to be active,
  hash-bound, client/tenant/grant-scoped, and subject-bound. RFC 7009, app/consent
  revocation, credential change, code replay, and refresh reuse revoke the whole
  family, so an already-issued access JWT is fenced before its signed TTL. Delegated
  rows also capture the consent epoch (`consent_granted_at`); a consent revoke/re-grant
  cannot resurrect older tokens. Code exchange locks the active app and delegated
  subject in the persistence transaction, then rechecks the client-secret hash,
  authorization-code grant, current scopes, registered redirect URI, subject status,
  and password-change epoch before consuming the code. Refresh rotation similarly
  locks the app and subject and rechecks the client-secret hash, refresh grant,
  current scopes, subject status, and password-change epoch before consuming the
  refresh token. Client-credentials issuance locks and rechecks its app before
  persisting the access row. The OAuth authorization-code **creation**
  service independently reserves the same app-then-user mutation fences,
  revalidates active app/subject, authorization-code grant, registered redirect,
  scope ceiling and (for third-party clients) active scope-covering consent,
  then inserts the pending code and commits in that transaction. Browser,
  consent-form and JSON authorization paths share this service; their earlier
  presentation/consent checks are only preflight decisions. This prevents a
  code from being inserted after a competing app/consent revocation already
  invalidated the then-pending codes. App, consent, password, and user-status
  revocation also invalidate outstanding authorization codes, preventing
  pending credentials from surviving a later reactivation. When an
  authoritative issuance check rejects a stale HTTP preflight, the
  controller returns a redacted `invalid_client`, `invalid_request` or
  `access_denied` protocol error instead of incorrectly reporting a
  persistence failure; unexpected database failures remain `server_error`.
  OAuth refresh-token **reuse** handling performs an advisory preflight lookup
  without locking the app for random unknown tokens, then locks the tenant-scoped
  app and re-reads the revoked token under that lock before any live-family
  revocation. The checked replay timestamp and family update share a transaction
  and use the same app-first mutation fence as refresh issuance; failures propagate
  instead of reporting a successful revocation. No-op/grace checks explicitly
  release the transaction. An already-used authorization code similarly revokes
  its scoped access/refresh lineage in an app-locked transaction and commits
  before returning `invalid_grant`. These guarantees depend on cooperating
  writers using the canonical app-row lock, not on distributed process leases.
- Access-token authentication resolves active OAuth applications through a security-only lookup that does
  not hydrate tenant locale or presentation translations; authentication must not depend on
  `oauth_app_translations`.
- Manifest synchronization for managed OAuth apps acquires an exclusive tenant-row lock and reconciles
  app upserts, translations, orphan deactivation, token revocation, consent revocation, and pending-code
  invalidation in one transaction per tenant. Existing app rows are also locked through the same mutation
  fence used by token issuance and consent before their security-sensitive configuration changes.
  Reactivation also revokes dependent credentials left live by older versions. The all-tenants bootstrap
  sweep is sequential and does not provide one global cross-tenant transaction.
- OAuth client-secret rotation performs a tenant-scoped, permission-checked
  preflight **without** holding a SQL transaction while generating and Argon2-
  hashing the replacement secret. The commit phase locks the current app row,
  repeats the permission and rotation-admission checks, and rejects a prepared
  rotation with a typed conflict when another operation has already changed
  the secret hash. Thus
  expensive hashing does not hold an app mutation lock, and simultaneous
  rotations from the same original secret cannot both report success while
  one silently invalidates the other. A stale preparation must be retried;
  rotation still does not by itself imply delivery or credential revocation.
- Manual OAuth app configuration edits and explicit app revocation now acquire the
  same tenant-scoped app mutation row fence as token issuance/consent and manifest
  reconciliation before testing whether an app can still be changed manually.
  The edit keeps validation, base-row mutation, presentation translations and
  their change records inside one transaction; revocation deactivates the app
  and revokes its active access/refresh tokens, user consents and pending
  authorization codes in one transaction. A concurrent manifest ownership
  transfer cannot slip between a stale manual-edit/revoke permission check and
  the subsequent write. The SQLite branch reserves the write lock before its
  first security-state read; PostgreSQL/MySQL use FOR UPDATE.
- RFC 7009 family revocation takes the app lock before looking up and updating
  the token family, sharing the token-issuance/refresh lock order. This prevents
  a new refresh successor being issued after a stale revocation read. Repeated
  revocations preserve the first `revoked_at` timestamp of every family member;
  unknown scoped tokens retain the standard success response.
- OAuth secret rotation's database CAS additionally checks that the app still
  belongs to a type permitted to rotate credentials. A concurrent configuration
  change to an embedded/unsupported app type cannot cause a stale preflight
  to install a secret; the caller receives the existing retryable conflict.
- OAuth grant admission uses the exact persisted `grant_types` set. Manifest-managed first-party admin/storefront
  applications declare `authorization_code`, `refresh_token`, and `client_credentials` explicitly; the auth
  model does not expand `refresh_token` implicitly for auto-created applications. Third-party web redirect
  URIs require HTTPS; mobile clients may use HTTP only on loopback and may use reverse-domain private-use
  schemes. First-party managed apps may use loopback HTTP only when their configured public URL is itself
  loopback HTTP, keeping local development separate from production web-client policy.
- The OAuth token endpoint resolves clients through the security-only OAuth app lookup, so token issuance does
  not depend on tenant locale or `oauth_app_translations` presentation data.
- Auth self-service endpoints (`/api/auth/me`, sessions, password change, profile, and history) require
  the typed `AuthPrincipalKind::DirectUser` principal. Delegated OAuth user principals are valid for
  delegated OAuth/storefront flows but do not inherit the direct-session self-service contract.
- Password-reset and email-verification request endpoints keep their public response generic even when
  email transport setup or URL preparation fails for an existing account; delivery/preparation failures
  are server-observable but must not become account-enumeration signals.
- OAuth browser-session and consent flows use the same case-insensitive Bearer authentication scheme
  parsing as the HTTP authentication boundary; the browser-session helper requires exactly one Bearer
  token and does not silently accept Basic or multi-token authorization values.
  Repeated `Authorization` fields are rejected. When an `Authorization` header is present but
  malformed, consent does not fall back to a cookie from a different principal. OAuth browser
  cookies are read across all Cookie fields and duplicate `rustok_oauth_browser_session` names
  fail closed instead of using an order-dependent token.
- Marketplace registry/governance REST handlers extract `ServerRuntimeContext`; catalog projection,
  artifact storage and remote executor policy are read through DB/settings/shared handles neutral runtime.
- Per-registry marketplace freshness is projected through
  `SharedModuleMarketplaceCatalog`, not directly from the server health
  aggregate. The operator-only GraphQL/native contract exposes stable logical
  registry identity, typed status, last success, and consecutive failures and
  omits deployment endpoints and remote error content.
- Swagger document filtering, installer persistence reads, admin DLQ, MCP management/remote tools
  and build WebSocket extract `ServerRuntimeContext`; DB/shared runtime semantics do not depend on a
  framework-global context. Optional module HTTP composition recognizes manifest-declared Axum routers:
  `rustok-blog` is merged from its `HostRuntimeContext` entrypoint.
- Channel runtime surface remains a thin transport around `rustok-channel`: `/api/channels/*` already covers bootstrap, channel CRUD-lite, policy-set/rule authoring endpoints and request-level `resolution_trace` diagnostics, while the resolution pipeline, REST/control-plane DTOs and rule-payload mapping helpers live in the module. Request-level `tenant`, `channel` and `locale` middleware receive `ServerRuntimeContext`, and auth context receives `ServerAuthRuntime`. For host resolution, the propagated `ChannelContext.target_type/target_value` identifies the concrete web-domain target that matched the effective host, while non-host resolution retains the channel's primary/first target representation. Channel REST mutations invalidate the local cache immediately and publish the durable generation to other replicas; the five-second database reconciliation remains the recovery path if fan-out is unavailable.
- Module-owned event listeners are assembled from `ModuleRegistry` into a common `EventDispatcher`; `apps/server` no longer holds separate host-owned index/search/workflow listener paths.
- Server migrator is the backend composition root for module-owned schema: content-family modules (`blog`, `pages`, `comments`) and search must connect here via `crates/modules/rustok-*/src/migrations`, otherwise external Next/Leptos admin surfaces get a working route shell without the needed tables.
- Product/search title filtering helpers are not server-owned services: product translation
  search predicates stay in the owner/foundation commerce search contract, and
  `apps/server` must not reintroduce `services::product_search`.
- `apps/server` can run as a `full`, `registry_only`, `api`, `admin_ssr`,
  `storefront_ssr`, or `worker` host. API and SSR modes skip background
  workers; the worker mode completes normal runtime bootstrap and starts those
  workers while mounting the health/metrics HTTP surfaces plus the
  profile-independent registry remote-runner boundary when
  `runtime.registry.remote_executor.enabled` is true. `host_mode`
  does not replace a deployment profile or choose build artifacts. Artifact
  HTTP and command dispatch resolve the shared effective module policy before
  invoking a binding and fail closed for a disabled or denied module; they do
  not reconstruct tenant enablement from transport-local SQL.
- `settings.rustok.runtime.background_workers` governs only maintenance workers on top of the already published HTTP/GraphQL surface. In `development.yaml`, for standalone admin debug, `workflow_cron_enabled`, `seo_bulk_enabled`, `seo_sitemap_enabled`, and `seo_index_repair_enabled` are disabled so that maintenance loops do not saturate the local PostgreSQL pool; the production/default runtime keeps them enabled.
- The generic module-work host supplies artifact queues with an active-tenant
  enumerator from `rustok-tenant` and a server-composed CAS-backed Rhai/WASM
  executor before artifact registrations run. The runtime CAS is obtained from
  the same operation-scoped `ModuleControlPlane` as artifact capability,
  installation, audit, and sandbox-policy services, so the server does not
  create an independent artifact staging identity source. Executor registration
  records placement explicitly and rejects same-kind fallback registration.
  Artifact and Alloy Rhai now share one mTLS-authenticated
  `rustok-sandbox-worker` client after exact readiness and register only the
  `isolated_worker` placement; connection, attestation/cgroup readiness,
  protocol, cancellation, or worker failure has no in-process fallback.
  Wasmtime remains explicitly `in_process` under its accepted component-runtime
  threat model. The canonical Kubernetes renderer now supplies the
  digest-pinned gVisor/Kata, mTLS-probed, restricted-network worker profile;
  retained cluster enforcement and supervisor evidence remains open.
  The neutral Rhai bridge and WIT import both reach only `SandboxHost`;
  `platform.data`, `platform.data.objects`, `platform.secrets`, and
  `platform.mcp` are the composed sandbox capability routes. Object transfer is
  bounded and larger writes use resumable owner-owned uploads: every decoded
  base64 chunk is at most 44 KiB, durable private sessions verify ordering,
  final size, and SHA-256 before publication, and no call exposes a storage key,
  URL, bucket, or credential. The secret route returns only a logical reference
  and revision after an immediate exact installation, lifecycle, grant-revision,
  grant, and derived-scope recheck; secret values and resolver identities remain
  host-only. The MCP route maps only the stable `rustok` alias to the
  owner-defined read-only registry tools, derives an MCP service identity from
  the admitted artifact installation, reapplies the owner access policy, and
  requires redacted durable audit before invocation. It accepts no endpoint,
  token, credential, transport, or discovery input. All other unregistered
  capability names remain default-deny. Artifact HTTP is a separate host
  transport at `/api/artifacts/{installation_id}/{*path}` and artifact commands
  use `POST /api/artifacts/{installation_id}/commands/{binding_id}`. Host-rendered
  declarative actions/forms use only
  `POST /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/execute`;
  that route resolves the contribution to its exact admitted Command binding
  before it joins the generic command execution path. Its companion
  `GET /api/artifacts/{installation_id}/ui/contributions` reads the effective
  locale only from server middleware, filters contributions by their dynamic
  RBAC permission, and returns only host-safe localized declarative metadata.
  Its response uses the shared `rustok_api::ArtifactUiContributionView` DTO,
  not a server-local descriptor projection. Headless GraphQL exposes the same
  projection through `artifactUiContributions(installationId)`; both
  transports share one server adapter, receive locale only from resolved
  request context, and omit unavailable exact-locale contributions. GraphQL
  `executeArtifactUiAction(installationId, contributionId, input,
idempotencyKey)` is the corresponding headless action path. It resolves the
  contribution to its admitted Command binding before using the same effective
  policy, dynamic-RBAC, durable-idempotency, sandbox-dispatch, and audit path
  as REST; it cannot select a raw binding ID.
  Callers cannot select a locale; an unavailable exact locale omits the
  contribution without fallback. Its audit companion
  `GET /api/artifacts/{installation_id}/ui/contributions/{contribution_id}/audit`
  resolves and authorizes that same contribution before returning its redacted
  binding-specific evidence. Headless GraphQL exposes the same evidence through
  `artifactUiActionAudit(installationId, contributionId)` using that exact
  contribution-resolution and dynamic-RBAC path. Both audit transports project
  the one `rustok_api::ArtifactBindingExecutionAuditEntry` DTO and do not expose
  a raw binding selector. All five routes resolve only an active exact
  installation and check the binding's dynamic RBAC grant. The three execution
  routes enforce JSON and shared durable binding-idempotency limits through
  `rustok-modules` and call the shared sandbox executor rather than mounting
  artifact routers or injecting dynamic GraphQL fields; the audit route does
  not dispatch the artifact.
- `development.yaml` keeps `database.max_connections: 30` because heavy admin bootstrap routes like AI control plane resolve several GraphQL root fields in parallel. This is a local debug guardrail for both admin panels, not a new production contract.
- Routed artifact HTTP and UI-command bindings accept an optional non-nil UUID `Idempotency-Key` only when their admitted binding declares an idempotency policy. The server creates the tenant-matched `ModuleCommandContext`; the owner receipt binds actor, trace, correlation, request digest, and UUID key before it can replay a stored binding response.
- For registry/governance surfaces the server remains the canonical validator of lifecycle policy, `reason` / `reason_code` contract and allowed action set; thin clients may do preflight but do not define policy locally.
- Registry release, publication, and validation adapters obtain their shared transactional governance aggregate through `ModuleControlPlane`; host authorization and artifact storage remain adapter concerns. A live validation enqueue requires a non-nil UUID `Idempotency-Key`: the server derives one platform-scoped `ModuleCommandContext`, and the owner replays only an exact request revision, actor, trace, correlation, principal, and rejected-retry-policy match from its durable enqueue receipt. A remote validation claim returns the owner-issued `requestRevision`; a terminal runner request must return it as `expectedRequestRevision`, so claim and terminal stage transitions use the same request CAS. Heartbeats only renew an already-issued operational lease.
- For control-plane composition install/uninstall/upgrade server uses a single orchestration path: full manifest validation, CAS-update `platform_state`, owner-operation receipt completion, and build enqueue are executed atomically within one transaction boundary. The authenticated transport supplies a positive expected revision and UUID idempotency key; the owner admits the immutable command before manifest adaptation and exact retries replay the original build. `platform_state.manifest_hash` is the SHA-256 canonical composition snapshot digest, `manifest_ref` is always `platform_state:<revision>`, and the build's `manifest_hash` is the distinct immutable execution-request identity over the snapshot, composition digest, deployment profile, and execution plan.
  New build receipts preserve the persisted execution plan and reason. Exact replay of receipts written before this fix preserves the original, possibly incomplete response; query the build read surface for current state.
- Server module-control-plane adapters construct `ModuleControlPlane` once per scoped operation and obtain composition, lifecycle, installation, or sandbox-policy services only through that owner facade. Static tenant enablement, normalized settings, post-hook retry, and compensation share the owner-owned `module_static_tenant_lifecycle` aggregate: its inherited/default revision is zero, each authenticated command carries a tenant-matched `ModuleCommandContext` plus its aggregate revision, and its execution claim fails concurrent work closed. The lifecycle journal and settings receipt retain actor, trace, correlation, and idempotency evidence. Lifecycle replay matches tenant, idempotency key, module, requested state, expected revision, and recovery source when present; settings receipts also bind the actor. Trace and correlation remain observability evidence. The settings writer completes its exact owner-operation receipt in the same transaction as settings persistence, revision advancement, and claim release; direct model writes, server-built lifecycle state, and native settings write fallbacks are not production contracts. GraphQL derives tenant, actor, trace, and `modules:manage`, requires a non-nil UUID plus non-negative expected revision, and returns the owner-issued next revision. `module_operations` retains hook lifecycle status as `validated/running/committed_hook_pending/committed/committed_hook_failed/failed/orphaned`; only pre-validation failures create no journal row. GraphQL maps canonical lifecycle/recovery facts to transport errors (`BAD_USER_INPUT`, `IDEMPOTENCY_CONFLICT`, `REVISION_CONFLICT`, `MODULE_HOOK_FAILED`, `INTERNAL_ERROR`), with idempotency conflict non-retryable; admin/SSR clients must not remap these fields.
- GraphQL `tenantModules` reads bounded explicit override/settings snapshots
  through the module owner facade. It does not query `tenant_modules` or infer
  effective availability; enabled/denied state remains owned by
  `ModuleEffectivePolicy`.
- GraphQL `artifactTenantLifecycle` and `setArtifactTenantEnabled` expose the
  owner-issued intent state for one admitted Optional artifact. The read
  returns the exact next CAS precondition (one for inherited enabled intent);
  the mutation derives tenant, actor, and `modules:manage` authority from the
  authenticated context and requires a non-nil installation UUID, positive
  expected revision, non-empty reason, and idempotency UUID. It delegates
  directly to the owner lifecycle transaction, so audit/outbox facts and exact
  replay remain owner-owned and raw admission or storage errors never cross
  GraphQL.
- GraphQL `activateTenantArtifact`, `deactivateTenantArtifact`,
  `uninstallTenantArtifact`, and `rollbackTenantArtifact` expose the matching
  owner lifecycle commands only for the authenticated tenant. Their scope is
  constructed from `TenantContext`, never accepted from the client; actor and
  `modules:manage` are derived by the same boundary. Rollback permits no
  target-installation selector: the owner can select only its retained direct
  predecessor after re-evaluating the supplied target capability-grant
  revision and migration rollback mode. There is deliberately no platform
  lifecycle GraphQL endpoint: a tenant-derived permission cannot authorize a
  platform-wide operation, and the transport remains fail-closed until a
  separate platform operator authority is defined. The shared GraphQL module
  authorization extension classifies the lifecycle snapshot as `modules:read`
  and every lifecycle mutation as `modules:manage` before resolver execution;
  resolver-level owner checks remain a second boundary. The same pre-resolver
  guard classifies the immutable composition snapshot as read and the
  operational marketplace registry freshness projection as manage, matching
  their owner-resolver checks. Unlike `moduleEffectivePolicy`, the current
  tenant-scoped `enabledModules` resolver is not classified by that module
  permission guard and does not call `ensure_modules_read_permission`; storefront
  and admin navigation use it. Its list is capped at 100 without a completeness
  marker, so it cannot establish a complete policy at the cap. The intended
  public-versus-admin availability authorization and a complete projection need
  an explicit contract before widening or replacing this surface.
- GraphQL build history and active-build reads use the host-composed read-only
  `rustok-build::SharedBuildControl`. Pagination limits are enforced by the
  owner, and the transport does not import build persistence entities.
- Native admin and GraphQL marketplace reads share the host-composed
  `SharedModuleMarketplaceCatalog`. The server adapter combines active local
  composition with configured remote registry providers, applies durable
  governance projection once, and attaches the owner-derived registry
  lifecycle snapshot for detail reads. Transport adapters perform DTO mapping
  only and do not scan the workspace or recreate governance policy. A
  production remote registry set is configured as a JSON array of unique,
  canonical stable identities and HTTPS endpoints. Endpoints may contain no
  embedded credentials, query, or fragment, and identity is independent from
  endpoint location. List reads may retain the local catalog during a remote
  outage, while per-provider health becomes explicitly degraded in
  `/health/ready`; remote detail transport/contract failures fail closed while
  an explicit remote `404` remains not-found. Slug collisions between
  non-local providers fail instead of selecting one by order, while the local
  manifest remains the explicit authority for a matching compiled module.
  Active registry-governed versions receive their immutable installable
  artifact contract only from the module owner projection. The adapter fails
  the catalog read when an active release lacks that contract; it never
  reconstructs registry, descriptor, source-lineage, or trust evidence from a
  checksum or server ORM row.
- Effective module availability now uses the facade's `EffectivePolicyService`, which reads tenant overrides and applies Core/default policy inside `rustok-modules`. The service returns the owner decision with its deterministic revision, typed facts, and denial reasons; server guards, GraphQL, and installer adapters consume only its enabled projection.
- Static module manifests and catalog entries are parsed by the server host, but `rustok-modules` owns module metadata, resolved static-catalog topology (defaults, dependencies, conflicts, and version compatibility), static manifest-versus-registry comparison, settings-schema, marketplace metadata, static UI classification, UI i18n semantics, static HTTP provider exclusivity, crate-local runtime binding normalization, deployment build-surface semantics, and settings normalization before lifecycle persistence with the effective-enablement and Core invariants; server filesystem, registry-fact extraction, and ORM code are adapters only.
- For uncertain lifecycle hook outcomes see `module-lifecycle-retry-compensation-runbook.md`; committed tenant intent is not rolled back automatically. The owner currently rejects new retry and compensation commands because the journal does not establish the original executor identity or external quiescence, even if the aggregate claim is absent. A matching historical committed retry may still be replayed without dispatch. Plans advertise no automatic action; older writers and ordinary new toggles do not yet comprehensively fence legacy uncertainty.
- For server-composed static enable, the host supplies the active manifest schema and the lifecycle owner validates retained settings after acquiring the aggregate claim, before calling hooks. It persists normalized settings with the intent and revision. Unresolved tenant or platform transitions block fresh lifecycle changes; this does not fence static rollouts. Existing semantic lifecycle replays preserve the original operation result.
- Schema lookup rejects unknown modules and missing or unmaterialized package manifests, including inline settings declarations without package evidence. Exact lifecycle replays are checked before a schema lookup failure blocks new enable or enable-direction compensation commands. The filesystem lookup is not release-bound; release images without these files reject fresh settings and enable commands until release-owned schema evidence is wired.
- Registry metadata now follows the common multilingual storage contract: publish/release base rows hold language-agnostic state and `default_locale`, while display metadata (`name`, `description`) live in `registry_*_translations`.
- Registry publication request classification now has one canonical path
  recognizer shared by the outer atomic-claim middleware and the inner
  metadata/authorization policy. Trailing-slash variants of
  `POST /v2/catalog/publish` cannot bypass the ownership/trust-level
  validation and `modules:manage` session requirement just because the
  middleware layers disagree on the URL shape. Rewritten JSON bodies also
  discard their obsolete `Content-Length` and `Transfer-Encoding` headers,
  preserving the verified principal and non-framing headers.
- Registry publish-request translations are written only through the owner create/publication transactions; the server no longer retains a parallel translation upsert helper.
- Registry audit payload no longer holds historical runtime fallback: `registry_governance_events.details` is normalized to a typed shape (`stage_key`, nested `owner_transition`, structured principal objects), and the controller maps lifecycle failures from typed `RegistryGovernanceError`, not from substring matching.
- Final registry publication at `POST /v2/catalog/publish/{request_id}/approve` requires a non-nil UUID `Idempotency-Key` header when `dry_run=false`. The session-backed transport derives one platform-scoped `ModuleCommandContext`; the owner binds its actor UUID to the structured approval principal and persists actor, trace, correlation, idempotency, and approval facts with the resulting release. It replays only an identical retry and rejects changed context, changed approval data, or a published row without a receipt.
- Live registry review transitions at `POST /v2/catalog/publish/{request_id}/reject`, `request-changes`, `hold`, and `resume` likewise require a non-nil `Idempotency-Key`. They derive a platform-scoped context from the session and telemetry, use one immutable owner receipt ledger keyed within the publish request, and replay only the same operation kind, revision, actor, trace, correlation, reason, and reason code. Dry-run previews remain non-mutating and do not reserve an operation key.
- Live registry release yanking likewise requires a non-nil `Idempotency-Key`. The session and telemetry derive the global-registry command context; the owner locks the exact release and records actor, trace, correlation, principal, privilege, reason, and reason code in an immutable receipt. Only the exact retry succeeds after a yank; changed input fails closed.
- Live registry owner transfer likewise requires a non-nil `Idempotency-Key`. The owner locks the slug binding, records the previous and new owner with actor, trace, correlation, privilege, and reason facts, and accepts only an identical retry.
- Live registry publish-request creation requires a non-nil `Idempotency-Key`. The request ID remains the deterministic business-command identity, while a separate immutable create receipt binds the authenticated actor, trace, correlation, principal, and privilege facts for exact replay.
- Live registry artifact upload requires a non-nil `Idempotency-Key` before the host stores bytes. The platform-scoped context accompanies both owner slot authorization and the durable attach; the attach transaction locks the request and records metadata, storage result, actor, trace, correlation, and privilege in one immutable receipt. Only that exact retry returns the committed attachment result.
- `GET /v2/catalog/publish/{request_id}` remains a machine-readable operator status contract: without bearer auth it returns a status-driven superset `governanceActions`, and with a session-backed user bearer it scopes request-level actions to those actually allowed for that principal.
- The registry-governance service carries the owner-issued
  `ModuleGovernancePublishRequestStatusSnapshot`, nested
  `ModuleGovernanceRequestSnapshot`, and host-only
  `ModuleGovernancePublishArtifactDownloadSnapshot` directly. It has no
  server shadow DTOs or owner-to-server field-copy adapters; the controller is
  the sole mapping point to browser-safe `rustok_api` response contracts.
- Registry artifacts are never read or written through server-local filesystem paths: persisted state stores only `artifact_storage_key`, owner services call the shared `ObjectStore` directly, and `GET /v2/catalog/publish/{request_id}/artifact/download` uses native signing with stream fallback. Validation is selected by immutable artifact origin: platform-built and external-prebuilt uploads retain the bounded metadata-bundle contract, while `alloy_authored` accepts only the bounded canonical Rhai workspace media type. Final Alloy staging still requires that upload checksum to equal the reviewed source-revision digest.
- `POST /v2/catalog/publish/{request_id}/author-signature` accepts an authenticated platform-scoped command context plus immutable signature reference, SHA-256, signer, policy revision, and body idempotency key. It never accepts the signed artifact digest: `rustok-modules` locks the publish request, derives its current attached-artifact checksum, persists the signature digest in the immutable evidence fact verified at final publication, and records an exact-replay receipt over the context and signature facts before returning the owner status snapshot.
- Alloy release staging is host-composed on both transports: `POST /api/alloy/scripts/{id}/releases/stage` and the manifest-owned GraphQL `stageRelease` mutation require the authenticated tenant's current script revision, `scripts:manage` plus `modules:manage`, reviewed source digest, publish request ID, and idempotency key. They delegate all marketplace writes to `rustok-modules`; final promotion remains the registry governance approval operation, and publish-status `nextStep` now points Alloy-authored requests to this staging boundary instead of implying direct approval.
- Alloy marketplace import is host-composed on authenticated HTTP `POST /api/alloy/releases/import`, GraphQL `importPublishedRelease`, and remote MCP `alloy_import_published_release` on `/api/mcp/runtime/tools/call` and `/api/mcp/runtime/tools/stream`. The server derives tenant and actor identity, verifies identity-to-binding tenant equality, requires `scripts.manage` plus `modules.manage`, and injects an owner-backed source provider that accepts only the exact active publication projection and verified digest-pinned CAS workspace. The adapter cannot read catalog DTOs, upload objects, or mutable OCI tags. Generic stdio MCP does not advertise this tool because it lacks the server's durable tenant-bound composition. Imported-draft preview and workspace-test execution resolve the immutable parent reference through the owner to the exact active tenant installation and policy on every run; inactive, disabled, stale, mismatched, or missing parent state fails closed without a default policy fallback.
- Marketplace catalog transport exposes only the current `GET /catalog` and
  `GET /catalog/{slug}` contracts. The server router and outbound registry
  clients do not probe or serve version-suffixed compatibility paths.
  Catalog generation fails closed when the active platform composition is
  invalid; it never substitutes a builtin manifest as a legacy fallback. The
  bounded registry client disables redirects and fails startup if its bounded
  client cannot be constructed; it never falls back to an unbounded default
  client.
- The repo-side surface for the current `module-system` is considered closed for the purpose of Admin-driven install/uninstall/upgrade/deploy with progress feedback; ongoing work is limited to targeted verification and docs/audit, while rollout of `modules.rustok.dev` remains an external infra task.
- GraphQL control-plane surface publishes a read/write contract for lifecycle recovery: `moduleOperationRecoveryPlan` and `failedModuleOperationRecoveryPlans` return tenant-scoped retryability/action metadata from `module_operations`, and `retryFailedModuleOperationPostHook` / `compensateFailedModuleOperation` route through `ModuleLifecycleService` and `modules:manage`. New recovery actions currently fail closed; the endpoints must not bypass that restriction.
  Recovery plan projection validates both UUIDs and fails the affected field with a sanitized `INTERNAL_ERROR` rather than panicking or silently omitting a malformed plan. The retry command runs before its response is projected: a projection error does not imply that the command was rolled back. Clients should inspect the operation and use the existing idempotency/revision contract when recovering from an ambiguous response.
- GraphQL auth surface `me.permissions` returns a request-scoped RBAC snapshot for headless/mobile UI gating; this does not replace server-side permission enforcement on mutations/queries.
- `PUT` and `DELETE /api/rbac/artifact-permissions/roles/{role_id}` are the
  host transport for RBAC-owned explicit artifact permission grants. They
  require `modules:manage`, derive tenant and actor identities from trusted
  request context, and accept only an exact admitted installation, registered
  permission key, and idempotency key. The routes never write static
  `role_permissions` and do not auto-grant anything at installation time.
- MCP remote bootstrap surface `POST /api/mcp/runtime/bootstrap` performs a server-owned token-to-runtime-binding handshake for non-stdio transport: accepts Bearer/plaintext MCP token, returns tenant/client/token binding and effective access context, updates last-used timestamps and writes an audit event `remote_session_bootstrapped` with correlation id. Remote tool transport is complemented by `POST /api/mcp/runtime/tools/call` for JSON invocations and `POST /api/mcp/runtime/tools/stream` for SSE invocations of core registry tools (`mcp_health`, `mcp_whoami`, `list_modules`, `query_modules`, `module_exists`, `module_details`), Alloy scaffold draft tools (`alloy_scaffold_module`, `alloy_review_module_scaffold`, `alloy_apply_module_scaffold`), remote-only Alloy script authoring tools, published-release import, and source-free deleted-evidence retention tools. Script authoring, published-release import, and retention authority resolve the durable binding, require their declared permissions, verify identity-to-binding tenant equality, derive actor provenance, and construct the same `SharedAlloyRuntime::scoped` owner service used by HTTP/GraphQL where applicable. Source-bearing authoring and published-release import metadata is redacted before audit persistence; retention remains source-free. Generic stdio/in-process MCP cannot advertise those owner-bound names. Scaffold tools use the server-owned persisted draft store, so stage/review/apply go through `mcp_scaffold_drafts`, tenant/client binding, and audit surface, rather than process-local memory of the MCP runtime. Remote correlation IDs are normalized:
blank values receive a generated UUID, while supplied values must be 1–128 printable ASCII
characters before they are persisted or used as SSE event IDs.
- The hybrid product installer is introduced through `rustok-installer` and
  `rustok-installer-persistence`. The latter owns the shared SeaORM database,
  durable-state, and bootstrap-writer adapter. `/api/install/*` delegates plan,
  receipt, preflight and execution semantics to these shared crates; the web wizard
  must not become a separate bootstrap implementation. The server binary has
  no `install` parser. `rustok-installer-cli` owns `install plan`, `install
preflight`, `install apply`, `install status`, and `seed apply`; apply uses
  the shared executor-port extraction rather than server code.
- The HTTP adapter binds one portable instance root before preflight. Production
  requires host-selected `RUSTOK_INSTANCE_ROOT`; local storage and filesystem
  release materialization derive `storage` and `releases/platform/sha256`
  beneath it through `rustok-runtime::InstanceLayout`.
- For distributed plans the HTTP adapter discards any client-supplied bundle
  identity. It accepts the locally configured
  `RUSTOK_INSTALL_DISTRIBUTION_RELEASE_ID`, resolves the exact preparation,
  bundle-root, and role-set identity from the current admitted
  `rustok-modules` ledger. A fresh target instead configures
  `RUSTOK_INSTALL_BASE_DISTRIBUTION_RECEIPT` and
  `RUSTOK_INSTALL_BASE_DISTRIBUTION_PUBLIC_KEY`; the host strictly verifies the
  bounded signed receipt and executable composition before mutation. The two
  authority sources are mutually exclusive, and all client-supplied bundle or
  receipt values are discarded. The old
  per-role `rustok-build` activation adapter has been removed. Distributed
  apply remains unavailable until the single owner-controlled rollout adapter
  and verified fresh-bootstrap owner-ledger import are composed.
- The HTTP adapter for the Leptos wizard is available as a thin surface on top of the same
  pipeline: `GET /api/install/status`, `POST /api/install/plan`,
  `POST /api/install/preflight`, `POST /api/install/apply`,
  `GET /api/install/jobs/{job_id}`, and
  `GET /api/install/sessions/{session_id}/receipts`. HTTP `apply` starts a
  background job and returns `202 Accepted` with `job_id`; the wizard must poll
  the job status and read persisted receipts for progress UI. The host reserves
  the local installer slot only after validating the request and checking the
  persisted running-job fence. A single runtime-supervised task then owns both the durable
  `running` row creation and its executor work; the runtime retains its `JoinSet`
  handle and closes admission during shutdown. The HTTP response awaits the
  creation acknowledgement, but disconnecting cannot interrupt the claim-to-run
  handoff or leak the local slot. Shutdown waits up to 60 seconds for accepted
  installer work, then aborts and joins it while leaving any still-running
  durable claim in place for operator reconciliation. Failed claims release the slot and preserve the
  409 conflict/500 storage-error mapping. New apply requests receive 503 while
  host shutdown is active. Neither this mechanism nor the existing unique
  running-job index recovers a job after process termination: ambiguous effects
  must be reconciled against durable receipts before an operator releases the
  global installer fence. A failed pre-execution HTTP claim now attempts an
  exact-identity, bounded CAS against a possibly committed-but-unacknowledged
  `running` INSERT before releasing its process-local slot; a missing row is
  harmless and a different job/session/submission or terminal status is never
  overwritten. This is best-effort when storage is unavailable and never
  relabels a job after the installer executor has been entered. If shutdown is detected after durable claim
  creation but before executor entry, the background owner transitions that
  unstarted job to `failed` and releases the slot, instead of beginning new
  installer side effects. A stop racing after the executor's final admission
  check still requires operator reconciliation of any interrupted effects. Mutating HTTP install
  requests support a setup-token guard via
  `RUSTOK_INSTALL_SETUP_TOKEN` and header `x-rustok-setup-token` or
  `Authorization: Bearer <token>`; production HTTP apply without a setup token
  is rejected. These are mutually exclusive ways to present the setup
  credential: requests containing both headers are rejected rather than
  selecting one source and ignoring the other. `/api/install/*` intentionally bypasses the tenant resolution middleware,
  because the first install runs before a tenant context is created. The CLI remains
  the canonical automation path.
- The tenant middleware resolution contract is fixed by integration tests in
  `apps/server/tests/tenant_resolver_invariants_test.rs`: the active tenant
  is resolved via `header`, `host`, and `subdomain`; a disabled tenant consistently
  returns `403`; a missing tenant returns `404`.
- The provisioning/deprovisioning path must trigger cache invalidation
  (`invalidate_tenant_cache_by_uuid/slug/host`) after create/update/deactivate/
  domain-change operations: the positive cache lives for `TENANT_CACHE_TTL=300s`,
  the negative cache miss lives for `TENANT_NEGATIVE_CACHE_TTL=60s`, so without
  invalidation stale resolver state is acceptable only within these TTLs.
  The regression matrix additionally captures lifecycle scenarios: stale positive
  cache after deactivate/update, negative cache after create-like flow, host
  cache after domain-change, and UUID invalidation.

## Responsibility boundaries

`apps/server` is responsible for:

- transport adapters, middleware, request/runtime context, and host wiring;

- the overall GraphQL schema surface and Leptos server-function entrypoints;
- composition of owner-owned AI GraphQL roots from `rustok-ai` and the narrow RBAC persistence adapter
  `AiGraphqlRoleSlugProvider`; AI resolver/DTO surface is not placed in `apps/server`;
- composition of `rustok-media::MediaQuery`, including owner-owned `mediaUsage`; media resolver/DTO
  are not placed in server `SystemQuery`;
- composition of dashboard order statistics via `rustok-order::load_order_stats_snapshot` when
  `mod-order` is enabled; SQL and DTO for order analytics belong to `rustok-order`, not
  `apps/server::RootQuery`;
- composition of dashboard post statistics via `rustok-content::load_post_stats_snapshot` when
  `mod-content` is enabled; SQL and DTO for content analytics belong to `rustok-content`,
  not `apps/server::RootQuery`;
- host-level user dashboard statistics and recent user activity via
  `services::dashboard_user_activity`; `RootQuery` only maps service DTO into GraphQL DTO and does not
  contain SQL/read-model logic for these dashboard widgets;
- manifest-driven composition of owner-owned `flex::graphql::FlexQuery` / `flex::graphql::FlexMutation` and
  registration of the concrete persistence adapter in `FlexGraphqlRuntime`; standalone Flex
  and attached field-definition resolver/DTO/error/RBAC/event mapping is not placed in `apps/server`;
- Axum REST handler for standalone Flex, which uses owner-owned `flex::rest`
  request/response DTO, request-to-command mapping, and view mapping; the server does not own Flex REST contract types;
- SeaORM adapter for standalone Flex, which stores persisted schema rows,
  but performs fields_config parsing/schema build/serialization, localized key derivation, row-to-view mapping, normalize/defaults/strip/validate, shared/localized split, read resolution, and PATCH merge via owner-owned helpers from `flex::standalone`;
- field-definition registry bootstrap, which registers donor persistence adapters for `user`/`order`/`product`/`topic`,
  but delegates row-to-core `FieldDefinition` mapping, `FieldDefinitionView` shape mapping, command-to-adapter-input conversions, persisted JSON shape helpers, and lifecycle policy/event construction to owner-owned `flex::registry`;
- bootstrap of the common module-owned event runtime via `ModuleRegistry` and `EventDispatcher`;
- health/runtime guardrails, build/release orchestration, and operator control-plane endpoints;
- installer HTTP/CLI adapters on top of `rustok-installer`, install locks, and
  persisted installer session receipts;
- composition of RBAC-owner decisions with auth/session context and host-level
  observability. Permission resolution and persisted role/grant readers belong
  to `rustok-rbac`.

`apps/server` must not:

- duplicate module-owned domain services, storage, and permission logic;
- replace module interaction contracts with its own ad hoc conventions;
- turn a cron, relay worker, or maintenance task into a pseudo-`event_listener` bypassing the module runtime contract;
- break the dual-path contract between GraphQL and `#[server]` when adding a new internal path.

## Shared richtext frame assets

When the server hosts the Leptos admin static fallback, `/richtext/frame` and
`/richtext/frame/<hashed-asset>` are immutable capability assets copied from
`@rustok/richtext`. The security middleware recognizes this narrow path and
uses the frame CSP, `X-Frame-Options: SAMEORIGIN`, `no-referrer`, and disabled
browser capabilities. It does not create a second editor bundle or pass auth,
tenant, locale, or document data to the frame URL.

## Health and runtime guardrails

- [health.md](./health.md) is the canonical document for readiness, runtime guardrails, `registry_only` smoke, and rollout evidence.
- `apps/server` must explicitly distinguish between `DeploymentProfile` and `settings.rustok.runtime.host_mode`.
- For reduced hosts, the health/runtime surface must describe the actually deployed runtime, not the full monolith by default.

## Verification

Minimum local verification path for changes in `apps/server`:

- targeted `cargo check` and `cargo test` on affected crates and transport slices;
- for build/profile wiring changes, separately verify at least one reduced build without embedded UI and one module-sliced profile such as `mod-commerce`-only or no-commerce content host, so that the server binary does not pull in extraneous surface dependencies;
- `cargo xtask module validate <slug>` for modules whose host wiring or manifest contract has changed;
- targeted contract checks for GraphQL, REST, server functions, and health/runtime surface;
- separate check of health/runtime paths if deployment profile, `host_mode`, or remote executor/runtime guardrails are affected.
- export API contracts via `node scripts/verify/export-reference-artifacts.mjs artifacts/reference`; the Bash wrapper `scripts/verify/export-reference-artifacts.sh` is intended for CI and Unix environments.

## Related documents

Artifact MCP calls consume the owner-issued `ArtifactCapabilityScope`; server
identity checks use its exact installation/release matcher and audit stable
owner/installation facts. MCP no longer carries a data-contract revision or
data namespace. It remains subject to host alias/tool policy and durable audit.
Logical-secret scope/storage now uses independently persisted opaque secret
instances and exact installation authority. Updated secret runtime fixtures,
host management/use fences, and stateless MCP runtime verification are tracked
in the module control-plane owner plan. The RBAC request-boundary fixture uses
actual platform/RBAC migrations without unrelated PostgreSQL-only product
migrations; its failed broad SQLite setup is not authorization evidence.

Artifact data maintenance holds belong to `rustok-modules`. Structured pages
reserve immutable request/frozen evidence before writes and retain source/target
between pages through the same database guards used by object migration, purge,
and recovery CAS. GraphQL maps `NamespaceHeld` to a sanitized precondition error;
it does not expose storage details or implement a second hold policy. The purge
host adapter binds operation context and delegates transaction-current permission
resolution to `rustok-rbac`; that read alone is not a revocation or write fence.

- [Health and runtime guardrails](./health.md)
- [Backend module guides](../../../docs/backend/README.md)
- [Library stack](./library-stack.md)
- [Axum runtime and operations CLI boundary](../../../DECISIONS/2026-07-02-axum-runtime-and-ops-cli-boundary.md)
- [Event transport contract](./event-transport.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Manifest layer contracts](../../../docs/modules/manifest.md)
- [Runbook retry/compensation lifecycle hook failures](./module-lifecycle-retry-compensation-runbook.md)
- [Documentation map](../../../docs/index.md)

## Product Media release delivery

The background-worker role starts Product Media delivery with `mod-product`, independently
of the embedded `mod-media` adapter. Lifecycle reservation is atomic before spawn. Each
batch reads one selected `ProductCatalogCommandRuntime` and drains through that owner's
embedded CatalogService; it never pairs the host-default database with a separately shared
Media reference port. Absent capability or an external Product profile does not authorize
local fallback. Pending owner replacement requires an explicit drain/cutover, not hot
repointing. See the [Product retention runbook](../../../crates/modules/rustok-product/docs/media-reference-retention.md)
for caller identity, bounded IO, operator permissions and unexecuted runtime gates.

## Bounded server review: route policies, reporting, static assets, metrics (2026-10-29)

- Path-scoped rate limit policies match only the configured route and its
  slash-delimited descendants. Similar-looking unrelated paths never consume
  an authentication or readiness-specific quota.
- The root GraphQL `dashboardStats` query requires an authenticated caller
  with current `analytics:read` authority. `recentActivity`, which contains
  user email addresses, requires current `users:list` authority. Both are
  guarded in the GraphQL document preflight policy and in the resolvers.
- The root GraphQL user connection orders results by creation time then
  immutable user ID, and treats search text literally rather than allowing
  SQL LIKE wildcards to expand the result set.
- A healthy outbox with zero pending events reports a pending lag of zero and
  a successful metric collection status; database failures still yield unknown
  metrics instead of false success.
- Tenant-scoped DLQ listing reports the total matching failed events independently
  of its bounded response page, and resolves equal timestamps with stable ID ordering.
- Refresh-token rotation verifies that the referenced user belongs to the
  same tenant as the selected refresh session, preventing a malformed
  cross-tenant reference from issuing a token with inconsistent authority.
- Host shutdown requests a cooperative drain from the module event dispatcher
  before joining it; already admitted dispatches and their handler tasks are
  awaited, and the host five-second deadline aborts the dispatcher task only as
  an emergency fallback.
- Host shutdown now awaits the payment-provider, paid-order-label,
  marketplace-financial, notification intake/candidate/fanout, Forum search
  inbox, and Product media-release worker handles after broadcasting stop.
  Each module drain retains the host's five-second timeout; failures and
  timeouts remain visible to operators. Durable owner-managed work is still
  responsible for retry after an interrupted provider operation.
  Payment provider recovery additionally checks the shutdown signal between
  tenant work items, without cancelling a recovery operation already underway.
  Tenant enumeration uses a bounded, UUID-ordered keyset cursor instead of
  offset pagination, so deletions of earlier tenant rows do not shift pending
  recovery work out of the current sweep.
- Relay polling rejects `relay_interval_ms = 0` instead of starting an
  unthrottled polling loop. Retry backoff requires a nonzero base and a
  maximum at least as large as the base; malformed policies fail at config
  load rather than causing a retry storm.
- Auth retention cleanup logs include OAuth token rows. A sweep that only
  prunes expired OAuth tokens is a reportable cleanup, not an invisible no-op.
- Embedded admin HTML fallback is limited to SPA navigation paths. Missing
  scripts, stylesheets, fonts, images and other recognized static assets return
  404 rather than HTML with status 200.

This section describes only the affected host boundaries, not an exhaustive
whole-crate or workspace audit. Changes require scoped Rust checks and tests
plus any applicable CI gates before integration.

- The outbox relay supervisor handles both a panicked child task and an
  unexpected normal task exit before applying its bounded restart delay;
  it no longer assumes the join result must be an error.

## Acknowledged sealed delivery (2026-11-01; source only)

The existing EventRuntime/listener bus remains the single module delivery composition. Fallible
module dispatcher startup binds explicit typed handlers before background relay delivery. The
`outbox` Memory target awaits required sealed handlers; `outbox_iggy` forwards the unchanged
frame to the configured primary first, then awaits matching local handlers. Unavailable registry
or local failure propagates, retaining Outbox retry/DLQ and original event identity; remote send
can repeat after partial local success. Families with no matching local handler remain remotely
consumed, but an unbound registry is never interpreted as proof that no handler is required.
Root `DomainEvent` records delivered by the Outbox relay now await every matching registered
module handler through the same listener registry; an explicit empty registry is valid when no
root handler matches. The generic `EventBus.publish_envelope` broadcast API remains best-effort and
is not the Outbox acknowledgement boundary. Artifact intake and cache/generation wrappers forward
typed frames through the existing chain. No extra bus, raw `sys_events` pipeline or host domain publisher.
See the [accepted root-delivery decision](../../../DECISIONS/2026-11-18-acknowledged-root-event-delivery.md).

The transport-double and regression definitions are **uncompiled/unrun**; they do not certify a
real Iggy connector, persistent receive/cursor ACK, relay/startup/restart/lag/crash windows,
artifact worker or PostgreSQL Search ordering. See the [Core API](../../../crates/libs/rustok-core/CRATE_API.md#acknowledged-sealed-module-delivery-2026-11-01-source-only)
and [Inventory packet](../../../crates/modules/rustok-inventory/contracts/evidence/reservation-events-source.json).

## Current service contexts for selected source work (2026-11-02; source only)

`build_shared_runtime_extensions_with_host_providers` installs the Auth-owned worker context
provider before native source factories. Deployment SecretRef resolution and the canonical current
OAuth resolver/current module availability supply genuine service permissions; no grant set or
actor is reconstructed from events/jobs. The port delegates actor identity to canonical
`AuthContext::port_actor()` (the authenticated service `client_id`, distinct from the separately
verified subject) and resolves credentials through the canonical Secrets cache-bypass operation
on each invocation. The generic
Index materializer owns replay work
registration; existing background-worker role and ModuleWork lifecycle own polling/shutdown.
[Actual configuration/contract](../../../crates/modules/rustok-distribution/docs/inventory-index.md).
Current page failures park terminally; retry/original-job recovery, real credentials/backend/
broker/restart/operator-client evidence remain open. No tests/build/SQL/runtime were executed.

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Crate README](../README.md)
- [Cache Stampede Protection](./CACHE_STAMPEDE_PROTECTION.md)
- [Host authority context and permissions](./host-authority.md)
- [Index reconciliation operator runtime](./index-reconciliation-operator-runtime.md)
- [Index drift diagnosis GraphQL transport](./index-drift-diagnosis-graphql-transport.md)
- [Index drift source page diagnosis](./index-drift-source-page-diagnosis.md)
- [Index drift source page GraphQL transport](./index-drift-source-page-graphql-transport.md)
- [Index replay GraphQL transport](./index-replay-graphql-transport.md)
- [Index replay GraphQL shutdown evidence](./index-replay-graphql-shutdown-evidence.md)


## Server audit continuation — installer HTTP job quarantine (2026-11-24)

HTTP installer jobs now refresh their host-runtime `updated_at` heartbeat
every 30 seconds. An atomic compare-and-set quarantines a `running` job whose
heartbeat is older than five minutes as `recovery_required`. A unique
expression index maps both unresolved states to the same admission key, so a
recovery-required job continues to block a second HTTP apply. The migration
creates the stricter index before dropping the old running-only guard; if
legacy unresolved duplicates exist, migration fails without removing the
previous guard.

The executor now preserves a typed `requires_recovery` classification through
the HTTP adapter. Ambiguous database preparation, owner-schema application,
session creation/claim, receipt persistence, state transitions and post-schema
effects are quarantined instead of being reported as clean failures. The
installer state machine no longer permits skipping `Deploying` or creating
new `failed` transitions, and it does not claim `fresh_install_cleaned` when
no cleanup ran. The global status endpoint projects the durable host job when
the target session is not present in the runtime database; the receipts route
returns a conflict rather than an empty receipt list in that topology.

This is deliberately **not** automatic replay or target-effect reconciliation.
The host job ledger is in the server runtime database while install sessions
and step receipts may live in a separate target database. The HTTP host does
not persist a recoverable target credential/reference, so it cannot safely
open an arbitrary target after process loss. A stale job remains
`recovery_required` until an operator reconciles the target database and
receipts; lease expiry never transfers ownership. Source regressions were
authored, but tests, compilation and formatting were not run at the
maintainer's request.
