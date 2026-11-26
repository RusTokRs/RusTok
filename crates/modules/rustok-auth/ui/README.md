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
also use it. Both Storefront SSR render branches mount the Auth provider and
inherit the request-scoped snapshot from the host Owner. The shared SSR adapter
requires an already resolved tenant slug and UUID for Storefront, rejects foreign
or ambiguous cookies before exchange, and checks the returned direct JWT projection
against tenant UUID and subject. An empty snapshot is not proof of absent credentials.
Standalone Storefront startup still needs owner runtime/tenant-policy composition.
These paths still need mounted verification.
Next/React has an independent UI implementation; this migration does not claim
runtime parity with it.

## Host-resolved public tenant binding

`AuthProvider` accepts optional `tenant_slug`. Before browser silent refresh, it calls
`storage::bind_request_tenant`: a missing public tenant may populate the existing canonical
`ADMIN_TENANT_KEY`, but a foreign existing tenant/session is never overwritten. Failed binding
refuses restoration rather than silently borrowing a default tenant. It issues no credentials,
creates no auth storage key/protocol and does not invent an SSR session. Browser native headers
read the epoch-verified canonical auth pair; absent or invalid metadata refuses before IO. SSR hosts
must validate and supply their own auth snapshot where required; native Product reads use the
validated Axum request context. Browser header mutation uses the fallible
`web_sys::Headers` obtained from the Leptos request wrapper. The gloo convenience
setter returns unit and throws; it cannot implement this error contract.

## Client auth scope and outstanding verification

The provider exposes a non-secret generation and marks it unavailable during
transitions. Both CSR and hydrate use the same exclusive origin-wide Web Lock
through admission, I/O and publication. The existing localStorage epoch
changes at start and finish, and queued intents recheck it at lock admission.
Auth providers subscribe to the existing custom event, storage, focus and
pageshow notifications; foreign epochs invalidate projections without adopting
the new identity. Explicit sign-in or reload establishes a new scope. Shared
memory contains one verified user/session pair tagged with the completed epoch;
all readers refuse stale epochs synchronously. Authenticated native functions
refuse absent credentials rather than falling back to ambient cookies, and
recheck epoch before dispatch. Refresh must preserve tenant, subject and session
lineage. Failed logout retains provider signals only for explicit retry, not
verified memory. The document lease still poisons an abandoned transition.
No token, identity or proof enters localStorage. A routine refresh invalidates
other projections conservatively; seamless cross-tab restoration, late
noncooperating cookie delivery and mounted expected-lineage verification remain open. Backend authentication and authorization remain authoritative.

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
URL. The direct WASM Fetch path now rejects redirects before following them. These changes need real-service and mounted UI verification, including cross-tab
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

## Proof-bound native refresh (source implementation)

Native SSR/hydrate sign-in and registration generate an independent 256-bit OS
CSPRNG seed before the API call. The BFF retains it with the bare refresh token
and tenant in one canonical HttpOnly envelope. The server-only exchange derives
a distinct HMAC-SHA256 operation proof for each presented generation, forwards
only the bare token and proof, and retries an ambiguous upstream refresh once.
The same envelope works across replicas; it is not a browser-readable credential.
The shared client now attempts refresh rather than logout when access expiry
passes while a tab sleeps. Admin SSR excludes Auth server functions from its
own exchange and refuses to overwrite explicit handler cookie mutations;
cookie-bearing responses are private/no-store. Old bare BFF cookies are rejected:
stop old writers, revoke prior sessions and require a fresh sign-in before cutover.
Direct CSR uses the Auth-owned token/seed envelope. Its exclusive origin-wide
Web Lock serializes cooperating login/register/refresh/logout calls until
response parsing and bounded retry complete. The browser must support Web
Locks and AbortSignal.timeout in a secure context. Fetch uses same-origin
mode/credentials, redirect:error, no-store, and a 10-second timeout.
Storefront document routes now use the shared tenant-scoped SSR exchange;
assets and feeds are separate, HEAD and speculative/subresource requests do not
rotate, and credential-bearing documents cannot revalidate through ETags.
Admin excludes asset/health and non-document requests while retaining explicit
cookie-consuming native/preview paths. Late noncooperating response ordering,
standalone runtime composition, mounted expected-lineage verification,
mounted browser verification and real backend/replica failure injection
remain open.

Retained-session refresh/logout forwards the canonical non-nil expected
session UUID through the direct cookie or native BFF transport. A malformed
projected ID is rejected before I/O; a mismatch is rejected by the Auth
server before mutation. The native refresh adapter also checks the returned
lineage before persisting cookies. Discovery without a prior projection
remains credential-only; arbitrary late cookie delivery is not solved.
