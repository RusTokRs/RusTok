# Next Admin Documentation

> **MANDATORY FOR AI AGENTS — Read these guides BEFORE any code changes:**
>
> **FSD Architecture:** This app follows **Feature-Sliced Design** (FSD) layers: `app`, `shared`, `entities`, `features`, `widgets`.
>
> **Module Ownership:** Module business UI must NOT be placed in `apps/next-admin/src/`. It belongs in `apps/next-admin/packages/*` or `@rustok/*-admin` packages.
>
> **IMPORTANT RULES:**
>
> - **DO NOT write custom components** — check existing components in `src/shared/ui` and `packages/*` first
> - **DO NOT invent custom i18n** — use host-provided `x-rustok-effective-locale` and `@rustok/next-fluent`
> - **DO NOT duplicate transport/auth** — use shared contracts in `src/shared/api` and `src/shared/lib`
> - **DO NOT create starter-only routes** — `billing`, `exclusive`, `workspaces` must return `notFound()`
>
> **Related Guides for Rust Module UI Packages:**
>
> - [FFA Architecture Guide](../../../docs/UI/module-package-architecture.md) — explains **FFA** (Fluid Frontend Architecture) for Rust module packages
> - [Implementation Guide](../../../docs/UI/module-package-implementation.md) — internal libraries, i18n, file structure for Rust packages

Local documentation for `apps/next-admin`.

## Purpose

`apps/next-admin` is the Next.js admin host for RusToK. It provides the React/Next path for the admin panel, works in parallel with `apps/admin`, and mounts module-owned/admin-owned packages instead of moving module UI inside the host.

`apps/next-admin` is a Next.js composition host, not an FFA surface or a module-owned UI package. It maintains shell, routing, context composition and contract parity with Leptos Admin without moving module-specific workflows into the host.

## Boundaries of Responsibility

- own the Next.js admin host, routing and shared integration layer;
- mount package-owned admin surfaces from `packages/*`;
- use canonical frontend contracts for auth, GraphQL, forms and shared UI;
- maintain parity with `apps/admin` at the platform contract level;
- keep admin shell navigation in parity with Leptos Admin: `Overview`, `Management`,
  `Module Plugins`, `Account`, with module-owned items remaining registry-driven
  and filtered by enabled module slug;
- do not pull module-owned business UI into host code.

## Runtime contract

- canonical FSD layers for host: `app`, `shared`, `entities`, `features`, `widgets`;
- backend integration goes through `apps/server` and shared transport packages;
- the shared Apollo client disables in-flight query deduplication: Apollo's
  default key excludes tenant, actor and locale headers, while the client is
  shared by endpoint. Query results also use `no-cache` and HTTP `no-store`;
- effective locale is selected by the host/runtime layer via `x-rustok-effective-locale`
  and `@rustok/next-fluent`; module-owned packages read the host-provided locale, not a cookie/query fallback chain;
- user language selection in Next Admin is stored in the host-owned cookie `rustok-admin-locale`;
  middleware normalizes the effective locale in order `?locale` → cookie → `x-rustok-effective-locale`
  → `Accept-Language` → `en`, and the UI uses dropdown in header and auth screens;
- global admin search uses `rustok-search` as a host-level capability;
- shared SEO operator/headless contract must also go through the backend surface:
  registry-backed target descriptors are read from GraphQL `seoTargets`, not from host-local slug mapping;
- the host module operator page reads `marketplaceRegistryFreshness` through
  `src/shared/api/modules.ts` and renders the same minimized operator evidence
  as Leptos; it does not read aggregate readiness or registry endpoints;
- the shared module-toggle helper sends the owner-issued lifecycle revision with
  a fresh UUID idempotency key and replaces its revision from the mutation
  result; tenant, actor, permission, correlation, replay, and lifecycle error
  semantics remain authenticated server and owner responsibilities rather than
  Next-local state;
- opening module settings reads the current tenant module snapshot and its
  revision together; a failed or missing read blocks editing instead of
  substituting empty settings, and responses from a previous session are ignored.
  Formatting, submission and the Forum/Blog field merges preserve the number
  lexemes received by the editor instead of rounding unrelated large integers.
  Forum numeric controls display and write decimal integers losslessly; an
  incomplete or invalid numeric draft blocks saving and JSON editing until it
  is corrected. Empty, invalid, and non-object snapshots are not silently
  replaced or submitted as empty settings. Forum and Blog structured controls
  are disabled while the JSON is invalid or not an object. This is not an
  end-to-end arbitrary-precision guarantee: the owner reads JSON through
  `serde_json::Value`, and domain validation still determines which numbers are
  allowed. A synchronous per-dialog guard prevents duplicate concurrent saves,
  dismissal is blocked while a save is pending, and late responses from an
  unmounted/replaced dialog do not notify the current session. An error is
  presented as an unconfirmed outcome, not proof of rejection. While the dialog
  remains open, editing is frozen and retry resends the exact payload, revision,
  scope and idempotency key. This is not durable retry: closing or reloading
  loses the operation, so the operator must review the current owner snapshot
  before a new attempt. A read alone does not prove an in-flight outcome has
  settled. Settings may contain sensitive values, so a persistent replay
  journal requires a separate secure design and owner outcome contract;
- shell availability is derived only from the owner-issued enabled-module
  projection; lifecycle results do not locally edit the projection. A failed
  policy read or missing session scope is surfaced as a localized retryable
  error rather than interpreted as a disabled module, including reads made by
  the dashboard layout and nested module guards. Retrying refreshes the current
  server-rendered route. The GraphQL `enabledModules` list is capped at 100
  entries without a completeness marker. At the cap, shell and dashboard use a
  separate complete `moduleEffectivePolicy` read, which the server permits only
  with `modules:read`, `modules:list` or `modules:manage`. They validate the
  decision shape and its common revision and use that snapshot alone, not a
  union with the earlier capped result. Users without those permissions, and
  users whose complete read fails, still see an availability error at the cap;
  a public complete projection remains open. The storefront GraphQL adapters
  separately reject capped lists. Leptos storefront shell and generic module-page
  error handling require runtime verification;
- module transition finalization forwards the owner-issued checkpoint revision
  with a fresh UUID idempotency key; dynamic rollback is invoked only from the
  selected active installation contract, never by changing a transition label;
- code must go through canonical FSD paths and shared package boundaries.

Governance actions use the owner-issued publish-request ID and available actions;
the operator workspace shows lifecycle recovery plans but disables retry and
compensation when the owner reports them unavailable; new actions are currently
fail-closed pending execution identity and quiescence evidence. Transition
rollback uses the owner operation identity. Both admin hosts display all active
transition checkpoints, plus a distinct checkpoint selected by URL when present,
and surface read errors. Runtime and browser verification remain required.

## Authentication and session lifecycle

- First-party login and registration use the Rustok cookie-session REST endpoints.
  The backend issues a rotating refresh token in an HttpOnly cookie; the host
  stores it in an HttpOnly, SameSite=Lax cookie scoped to `/api`, and stores
  the short-lived access token in an HttpOnly cookie scoped to `/api/rustok`.
  Both are Secure in production.
- Auth.js contains identity and tenant scope only. Rustok access/refresh tokens
  are not exposed by the Auth.js session endpoint or client components.
- Browser GraphQL requests use the same-origin BFF. It ignores browser bearer
  headers, binds tenant scope to the authenticated identity, refreshes expiring
  access credentials server-side, and forwards only the server-held access
  token. Cross-origin requests are rejected.
- Logout revokes the backend refresh session and clears host cookies.
  Refresh calls are coalesced within one Next.js process. If another instance
  wins rotation, the backend returns an explicit already-rotated conflict;
  the BFF preserves cookies and returns a retryable GraphQL error because the
  GraphQL operation has not been forwarded. This prevents the losing response
  from clearing the winner's new cookie. If the winning refresh response is
  lost entirely, the old token cannot recover the new secret and reauthentication
  is required after the grace window. Unknown refresh outcomes are not retried
  automatically.
- Self-service email changes require the current password and a one-time token
  sent to the new mailbox. The old address remains active until confirmation;
  confirmation verifies the new address, revokes other sessions and delegated
  OAuth tokens, and notifies the previous mailbox.
- HMAC-SHA256/HKDF-SHA256 and Argon2id are not JWT signing alternatives; the
  backend JWT runtime is RS256-only.

## Ownership contract for module UI

- If a module supplies admin UI, it remains a module-owned package alongside the module or in `packages/*`.
- Host `apps/next-admin` acts only as the composition root.
- Core navigation `apps/next-admin` must not contain module-owned business routes. Each module or capability connects its Next UX through `apps/next-admin/packages/*` / `@rustok/*-admin` entrypoint, and the shell filters items by enabled module slug.
- If a tenant has only `blog` enabled, ecommerce/catalog/product UX must not appear in navigation and must not live as a host-owned starter page.
- Starter-only routes `billing`, `exclusive`, `workspaces` and `workspaces/team` are not public RusTok admin surfaces and must return `notFound()`, not a placeholder UI.
- The same rule applies for core-modules, optional-modules and capability packages.
- Capability-owned surface `rustok-ai` is mounted as a package-owned UI, not as an ad-hoc host feature.
- Route-selection contract must be in parity with `apps/admin`: selection state URL-owned,
  only typed `snake_case` query keys are used, invalid keys do not fallback to the first item,
  and local Next helpers do not invent a separate schema/policy on top of the `rustok-api` contract.

## Packages and Integrations

- shared UI and frontend contracts go through `UI/next` and internal transport/auth packages;
- backend — `apps/server`;
- module-owned Next admin packages live in `apps/next-admin/packages/*`;
- shared API helper `src/shared/api/seo.ts` provides typed access to SEO control-plane: `seoTargets`, diagnostics, sitemap status/jobs, bulk jobs and job detail with REST primary (rollout-gated) + GraphQL secondary path strategy;
- semantic SEO error taxonomy (`BAD_USER_INPUT`, `PERMISSION_DENIED`, `NOT_FOUND`, transport failures) is considered canonical for Next hosts and is reused not only in `next-admin`, but also in Next storefront SEO runtime adapters;
- package naming contract for module-owned admin UI remains `@rustok/*-admin`.

## SEO operator readiness evidence

- Next Admin participates in SEO D8/D9 as the operator host for diagnostics, sitemap/bulk read surfaces and index repair/replay controls.
- Compile-free documentation evidence is tracked from the Next storefront SEO fixture docs sync matrix; live sign-off still requires a running backend sample for semantic error mapping and repair/replay telemetry.
- The required owner sign-off evidence is: REST/GraphQL error-code parity, bounded replay input validation, before/after index delivery counters and no duplicate idempotency transition.

## Interactions

- `apps/server` provides the API/runtime contract;
- `apps/admin` remains the primary Leptos admin stack and the reference for parity;
- module-owned UI packages are connected as external surfaces, not as host-owned business code.

## Verification

- typecheck/lint runs on `apps/next-admin`
- targeted checks on package-owned admin surfaces
- cross-reference shared contract with `docs/UI/*` and `docs/modules/manifest.md`

## Related Documents

- [Implementation Plan](./implementation-plan.md)
- [Navigation RBAC](./nav-rbac.md)
- [Deprecated Clerk starter reference](./clerk_setup.md) — not an active RusTok auth contract.
- [Themes](./themes.md)
- [Documentation Map](../../../docs/index.md)

### Product editorial transaction

The Product host save action forwards typed locale/value batches in the existing create/update
input. It must not issue a subsequent value-save request or compensate create with deletion.
The editor loads attribute values for each selected locale, retains dirty localized drafts
independently, and coalesces nonlocalized edits into one shared batch.
Publication, document/category changes, values and the success receipt belong to one Product owner
transaction. Source cutover is covered by `verify:product:atomic-save`; browser/backend execution
remains pending in the Product owner plan. Save/status/delete and variant update/delete retain durable browser-local
immutable requests and caller keys through unknown outcomes. Product document retries retain the
original brand bindings as write fields without the brand directory's display data.
Image/Relation/Bundle callbacks do not yet have that retry lifecycle.
The Next app CI job runs the Product package tests through `npm run test:product`.

### Product write acknowledgement and Apollo

The shared GraphQL executor uses Apollo 4 `CombinedGraphQLErrors.errors` with `errorPolicy: all`
and preserves bounded `code`, `retryable` and `outcome` extensions; partial data with an error is
not success. Product Server Actions serialize confirmed/rejected/unknown outcomes instead of
relying on exception text surviving production redaction. Only receipt-finalized domain refusals
marked `outcome: rejected` can release a retained attempt. A generic nonretryable admission or
context error is not a decision about an earlier uncertain commit.

Editorial attempts bind the authenticated tenant/actor snapshot and a caller UUID. Server-side
scope changes do not forward a retry under another identity. Successful owner acknowledgement
survives cache-invalidation failure. The editor keeps its document predecessor until its own
confirmed write; router prop refreshes do not silently rebase dirty fields. Child/portal callbacks
are guarded while an editorial attempt is unresolved. Initial money/GraphQL Int admission happens
before retaining the command, leaving invalid scalar drafts editable.

Recovery uses module-owned `editor_intent`, `attribute_recovery` and `variant_intent` URL UUIDs,
separate localStorage journals and exclusive Web Locks in a secure context. Requests persist before IO; explicit retry dispatches the original
operation without validating current fields. Duplicated URLs share an intent and reread its record
under the lock. Pending requests have no automatic expiry; terminal outcomes retain body-free
tombstones and cannot silently become new creates. Storage/lock/identity errors fail closed.

The host passes the route resource explicitly, keys mounts by scope/resource/intent and mounts
recovery-only UI for missing Product reads with a valid intent (including a lost delete response).
No replacement draft is enabled there. Confirmed continuation opens a fresh document under a new
intent; a refused draft can explicitly rotate intent without sending or rebasing fields.

This is an origin-local plaintext Product draft projection, never token storage or owner authority.
Browser clearing/eviction, cross-device recovery and owner receipt retention are not guaranteed;
never clear storage to resolve an uncertain operation. Actual browser/real-backend execution and
durable identities for Image/Relation/Bundle child mutations remain open. See the Product write boundary and owner plan.

### Shared HTTP retry advice

`src/shared/api/http.ts` projects the canonical flat `rustok-web::ErrorBody` for the existing
registry-governance REST helper in `src/shared/api/modules.ts`. `HttpError` retains HTTP status,
public code/message and optional exact `retryable`; `retryableState` is `owner`, `absent` or
`invalid`. Explicit false remains false even on 503, and true remains true on 409. Unknown,
malformed, unreadable, oversized or unfinished metadata never becomes status-derived advice.
The reader accepts at most 16 KiB of UTF-8 JSON within a two-second metadata budget, cancels
oversized/unfinished reads and releases its timer/reader; failed or stalled cancellation is not
an additional wait prerequisite. This is source policy, not measured mounted/browser timing.
HTML/raw/nested diagnostic responses use the caller's static fallback, not response text.

The decoder performs no fetch, transport fallback or write replay, and ignores outcome/key
claims. Existing request bodies, auth/tenant headers, write identities and owner recovery stay
unchanged. This does not change GraphQL extensions, Product write-outcome classification or
native server-function envelopes. Eleven reader regression functions are **unrun**; see the
[canonical HTTP contract](../../../docs/architecture/api.md#http-error-retry-advice).
