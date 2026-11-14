# rustok-auth-ui

The Auth module's shared FFA client/session boundary for both Leptos admin and
storefront. This is an Auth-owned capability package, not a separately mounted
storefront route or another authentication runtime. `rustok-auth/admin` owns
the admin pages; the backend `rustok-auth` package owns server-side auth and is
not a dependency of this browser-capable package.

See [CRATE_API.md](./CRATE_API.md), the [Auth client ADR](../../../../DECISIONS/2026-11-13-auth-owned-client-ffa-boundary.md), the [Auth plan](../docs/implementation-plan.md),
and the FFA [architecture](../../../../docs/UI/module-package-architecture.md),
[implementation](../../../../docs/UI/module-package-implementation.md), and
[verification](../../../../docs/UI/module-package-verification.md) guides.

## Layers and composition

- `core`: framework-independent cookie inspection/formatting and the unverified
  session-lineage parser. They do not authorize requests.
- `model`: the Auth client lifecycle scope plus canonical framework-neutral
  `rustok-ui-auth` user/session/error values; do not define duplicate models.
- `transport`: the single facade selecting native `#[server]` or GraphQL/cookie
  adapters according to the build profile; server-side policy stays in Auth.
- `ui::leptos`: the one context/provider, hooks, route guards, browser storage
  and authorized server-function client shared by both hosts. Rendered Auth
  pages and locale bundles remain in `rustok-auth-admin`.

Clients import the appropriate layer (`rustok_auth_ui::ui::leptos`, `::model`,
`::core`, or `::transport`). Dependency features `csr`, `hydrate`, and `ssr`
must propagate from every consuming package; they select one path rather than
a runtime fallback. Native SSR needs the `ssr` feature and a validated
request-scoped snapshot supplied by the host. The standalone storefront CSR entry
wraps both its shell and module routes in this provider; its Product and Blog islands
also use it. The pre-existing separate storefront SSR shell creates an owner without
an Auth provider or validated snapshot, so an SSR surface that requires a hook such
as `use_token` can fail rather than receive a session. It needs explicit, validated
request composition and cache-safe rendering; an empty snapshot must not be treated
as proof that no credential exists. These paths still need mounted verification.
Next/React has an independent UI implementation; this migration does not claim
runtime parity with it.

## Host-resolved public tenant binding

`AuthProvider` accepts optional `tenant_slug`. Before browser silent refresh, it calls
`storage::bind_request_tenant`: a missing public tenant may populate the existing canonical
`ADMIN_TENANT_KEY`, but a foreign existing tenant/session is never overwritten. Failed binding
refuses restoration rather than silently borrowing a default tenant. It issues no credentials,
creates no auth storage key/protocol and does not invent an SSR session. Browser native headers
still read canonical auth storage; header application failures now refuse before IO. SSR hosts
must validate and supply their own auth snapshot where required; native Product reads use the
validated Axum request context. Browser header mutation uses the fallible
`web_sys::Headers` obtained from the Leptos request wrapper. The gloo convenience
setter returns unit and throws; it cannot implement this error contract.

## Client auth scope and outstanding verification

The provider exposes a non-secret generation and marks it unavailable during transitions. In
source, cooperating providers in the same browser document use a shared lease and refuse overlap;
a dropped future poisons that document's lease because it does not cancel a request or its cookie
side effects. Notifications contain no credentials. If notification storage is unavailable, a
transition fails before network I/O. Other tabs receive an asynchronous invalidation stamp, not an
atomic lock, and old requests may still affect cookies after a reload. The Auth provider itself
does not subscribe to these notifications; Cart's bridge invalidates its own state, but other
provider projections can remain stale across roots or tabs. Route authentication and some
storage-backed transports also do not use the ready-scope getters. Server authentication and tenant
checks remain authoritative. On the cutover commit, [CI run 43575756113](https://github.com/RusTokRs/RusTok/actions/runs/43575756113)
passed locked Auth native default/SSR tests, the public contract and Auth admin SSR check; Auth
and Auth admin CSR WASM checks and the Product buyer island (including Auth hydrate) also passed.
Full host checks and mounted-provider verification remain open. Standalone state-machine tests do
not prove the browser protocol. The Blog reference plan records earlier bounded package/WASM
compilation, not validation of this change.

The native BFF classifies an absent refresh cookie separately from malformed, duplicate,
foreign, rejected or unreachable credentials. Only explicit absence with no existing session
can establish a guest scope. The direct-cookie endpoint currently uses the same HTTP 401 for
absence and rejection, so its client fails closed instead of guessing. Native logout now forwards
the validated refresh-cookie value only server-to-server to the idempotent cookie-session logout
endpoint, refuses redirects, and clears the BFF cookie only after a confirmed response. A failed
logout retains the in-memory identity for explicit retry but leaves the client scope unverified;
admin callers display failure rather than silently treating it as a guest. Direct-cookie logout
also requires the API's explicit `ok` response rather than accepting any 2xx body. Direct-cookie
session operations reject a cross-origin API before sending and reject a changed final response
URL. Browser fetch may have already followed a redirect, so this is not a redirect-prevention
guarantee. These changes need real-service and mounted UI verification, including cross-tab
refresh/logout races. Admin SSR
inspects every Cookie header and preserves the cookie on an unclassified exchange failure; it
still renders an anonymous snapshot on failure, not proof of guest identity.

`AuthContext` exposes its user, session, loading and error signals through read-only methods
(`user()`, `session()`, `is_loading()`, and `error()`). The handles are private to prevent a
clone from pairing a replacement projection with the provider's scope. Host consumers must use
transition methods instead of mutating signals or the crate-private browser session writers; SSR
hosts supply verified state via `provide_server_auth_snapshot`. Reading or writing the former
public fields requires a source migration. This does not make client state an authorization
boundary or constrain unrelated scripts and tabs.
