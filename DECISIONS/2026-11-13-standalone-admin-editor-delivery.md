# Standalone admin editor delivery

- Date: 2026-11-13
- Decision status: Accepted
- Implementation status: In progress
- Owners: `apps/admin`, `rustok-pages-admin`, `rustok-page-builder-admin`, `fly-browser`
- Extends: [Page Builder server preview delivery](./2026-11-12-page-builder-server-preview-delivery.md), [Fly SSR control contract and route-carried view state](./2026-11-11-fly-ssr-control-contract-and-route-view-state.md), [Fly SSR-first browser runtime](./2026-07-16-fly-ssr-first-browser-runtime.md)
- Supersedes: None
- Superseded by: None

## Context

The standalone admin host (`apps/admin`) renders the Pages workspace and no editor reaches a
browser. Three facts, read from `leptos` 0.8.21 source rather than inferred, decide the shape of the
fix:

1. **A `LocalResource` never resolves on the server, it is not merely slow.** `leptos_server`'s
   `ArcLocalResource::new` wraps the fetcher in `if cfg!(feature = "ssr") { pending().await }`, and
   the `ssr` doc comment says the same. Every `LocalResource` a surface renders therefore reads as
   pending during SSR, and the `Suspense` fallback is what the response contains. `PagesAdmin`
   loads both the page list and the open page this way
   (`crates/modules/rustok-pages/admin/src/composition.rs`), so the standalone host's HTML has the
   skeleton and never `PageWorkspace`.

2. **`Effect` is not a server-side bridge.** `Effect::new` dispatches to `new_isomorphic`, which
   "runs once on the next tick" by spawning a task. Under SSR that task is not driven to completion
   before the response is produced, so the pattern the modules use today — `LocalResource` plus an
   `Effect` that copies the value into a signal — reproduces exactly the same empty render. Adding
   an effect to `EnabledModulesProvider` would not make the module registry resolve either, and the
   route view already waits on that registry's `is_loading`.

3. **A `Resource` is not enough on its own: the route's SSR mode decides whether it is awaited.**
   `SsrMode::default()` is `OutOfOrder` (`leptos_router::ssr_mode`). In that mode a suspended
   fragment is written as `<!--s-N-o-->fallback<!--s-N-c-->` followed by a `<template>` chunk that a
   script swaps in, so an out-of-order route whose resources *are* on the server still puts the
   skeleton in the document and the workspace in a template nothing executes. `SsrMode::Async` is
   the mode that runs the render to completion and emits one document (`to_html_stream_in_order`
   collected before responding). `SsrMode::PartiallyBlocked`, the mode that would block only on
   resources without giving up streaming, is a TODO in the axum integration in 0.8.21
   (`_ = replace_blocks; // TODO`), so it is not an option. The route listing inherits the mode from
   the route definition (`GeneratedRouteData.ssr_mode` → `RouteListing`), which is why the mode is
   declared per route rather than per host.

4. **Leptos' own nonce is not this host's nonce.** `leptos_axum` calls
   `leptos::nonce::provide_nonce()` for every request, generating a fresh `leptos::nonce::Nonce`.
   `HydrationScripts` writes that value on its `<script>`/`<link>` elements. `apps/admin` enforces
   its own nonce, generated per request in `admin_security_headers` and written into the CSP
   header. The two are different values, so a `<HydrationScripts>` added as-is is blocked by the
   host's own policy — the failure would look like "hydration silently does nothing".

The admin host is expected to run without a hydration bundle today: `apps/admin/src/app/shell.rs`
states that the default SSR profile omits `HydrationScripts` and that browser interactivity comes
from standalone adapters such as `fly-browser`, and `verify-fly-ssr-first.mjs` pins that comment. The
Fly editor controls are already built for that model — the SSR control contract renders
`form[data-fly-intent-form]` pairs that the `fly-browser` host bootstrap binds — which is why the
editor is reachable without Leptos hydration at all, once its data is in the response.

## Decision

1. **The standalone admin keeps the no-hydration profile, and the editor loads its server state
   through `Resource`, not `LocalResource`, on a route that declares `ssr=SsrMode::Async`.** A
   surface that must appear in SSR HTML reads its data from an awaitable resource whose future is
   driven during rendering, *and* the route it renders under waits for that future. The two are one
   decision: a `Resource` under `OutOfOrder` still ships its fallback, so "moved to `Resource`" is
   only the first half. This is the ADR-2026-11-12 outcome "resolves the page before it renders",
   with the mechanism named: `Resource` plus an awaiting route mode, never `LocalResource` plus an
   `Effect`.

2. **Hydration is not the delivery mechanism for the editor, and it is still a supported profile.**
   `apps/admin` keeps `csr` and `hydrate` features for hosts that build a hydration bundle. The
   default SSR profile does not gain one, because the editor does not need it: its controls are the
   SSR intent forms the browser adapter binds.

3. **If a host does ship hydration, its bundle must run under the host's nonce.** That means the
   host's nonce — the one in the CSP header — has to be the one Leptos writes, by providing it to
   `leptos::nonce` for the request rather than generating a second one. A `<HydrationScripts>` whose
   script carries a nonce the header does not name is a defect, not a warning, and the host that
   ships it owns the test that proves the two agree.

4. **A host that can do neither renders the reason.** The backlog's rule stands: no host mounts a
   control that cannot act. Where a surface still depends on `LocalResource`, the host renders an
   explicit unavailable state rather than an editor whose buttons do nothing.

## Delivered so far

`PagesAdmin` (`crates/modules/rustok-pages/admin/src/composition.rs`) loads the page list and the
open page through `Resource` as of 2026-11-13, so a standalone SSR render contains them instead of
the `Suspense` fallback. The resource payload is a serialisable projection (`PagesWorkspaceData`):
the provider status is a view of rollout flags and an optional health snapshot, only those two
values cross the boundary, and the render derives the status again. That keeps the payload
serialisable without a second wire type and without letting a consumer fabricate an observed
snapshot — an absent health value stays absent.

The layout route (`<ParentRoute view=AppLayout>`) declares `ssr=SsrMode::Async`
(`apps/admin/src/app/router.rs`), so the resources every admin surface loads are resolved into the
document rather than streamed as a fallback plus a template no script replaces.

The mode belongs on that route and not on the module leaves, because the route owns the resource: it
mounts `EnabledModulesProvider`, and the sidebar renders the module navigation from it on *every*
admin page, `ModuleGuard` on `/mcp` and `/email` included. Declaring the mode per module route left
every other page streaming the sidebar's `Suspense` — a defect this decision had, briefly, before the
assertion moved to the route that actually mounts the provider. Leptos promotes a listing to the most
restrictive mode among its ancestors, so the layout's declaration covers its children and a child
cannot lower it.

Because that mode now governs every admin page, the layout's own pages had to stop depending on a
component that can only resolve in the browser: under `Async` there is no stream left to fill a
`Suspense` later, and a `LocalResource` is `pending()` on the server, so the dashboard, the modules
index, the search composition and the email settings form arrived empty. They load through `Resource`
now.

The email form needed more than the swap, because its fields were cells an `Effect` filled from the
response: a `Resource` behind an `Effect` still renders an empty form, which is the ADR's second
invariant. The form is now the resolved settings with the user's edits layered over them — a field
shows the edit if there is one and the backend's value otherwise — so the response contains the
form, and a field nobody touched never carries a value nobody set. The revision sent with a save is
read from the same resolved response. The installer is the one page outside the layout — it declares no mode of its own and keeps its
browser-side loader, because the default stream degrades there to the page's own fallback rather than
to a server render that waits.

That scope includes every `/modules/<slug>` surface, because module admins are mounted inside the
same layout. The product admin package leads: its attributes page
(`rustok-product/admin/src/ui/attributes.rs`), product grid (`product_grid.rs`), categories page
(`categories.rs`), catalog filter controls (`catalog_admin.rs`) and schema editor (`ui/leptos.rs`)
resolve their state through `Resource` now, and the forum split, merge, rename and fork panels, the
auth invites and security pages, the blog moderation queue and the forms inbox followed; 46
`LocalResource` call sites remain across the module admin packages. Each is the
same defect — a `Suspense` fallback served where the response should hold the surface — so they are
worked through one page at a time, and the count is recorded in the backlog rather than left
implicit.

Two of the product surfaces are not a swap, and they are the shape the rest will meet. The product
editor fills about twenty form fields from its response, so pointing the loader at `Resource` would
only move the defect: the fields are signals written by an `Effect`, which never runs on the server,
so the response would carry an empty form. It needs the arrangement the email settings form already
uses — the response resolves into a rendered value and the operator's edits are an overlay on it.
The attribute lifecycle panel's loader returns a `LoadedScope` holding a `WriteJournal`, which is not
serialisable and not the payload a `Resource` can carry; it needs a projection to the fields the panel
renders, as the Pages workspace needed. The third is a shape rather than a payload: a `LocalResource`
is `Copy` and a `Resource` is not, so a resource passed down as a prop and read through `.get()` in
several closures does not compile after the swap — the reads have to move behind a value that is
`Copy`, as the schema editor's category, schema and attribute views now do. None of the three is a
reason to leave the surfaces on `LocalResource`; they are work the conversion schedules rather than a
target it misses.

Module availability itself was the second half of the same defect: `EnabledModulesProvider` resolved
the effective policy through `LocalResource` and copied the result into signals from an `Effect`. On
the server that effect never ran, so `is_loading` stayed `true` and the module route rendered its
loading branch — the editor was never reached regardless of the page workspace. The provider now
resolves the policy through `Resource` and derives `modules`, `is_loading` and `error` as `Memo`s over
it; `ModuleAdminPage` and `ModuleGuard` read the decision inside a `Suspense` and distinguish "not
resolved" from "disabled for this tenant".

The remaining `rustok-pages-admin` surfaces then moved the same way: the inline edit launch, the
rollback control, the published metadata admission, and both layout template catalogs
(`inline_edit_launch.rs`, `rollback_control.rs`, `standalone_metadata.rs`, `templates_admin.rs`). Each
was a `LocalResource` read under a `Suspense` that the server could never fill, so each rendered as
nothing or as a skeleton in every response.

The consumer properties panel (`rustok-page-builder/admin/src/editor/consumer_properties.rs`) was
the one surface held back, because `Resource::new` requires `S: PartialEq + Clone + Send + Sync` and
`Fut: Future + Send`, while `ConsumerPropertyEditorPort::load` returns `ConsumerPropertyLoadFuture`,
which is `Send` only on native targets. That turned out to be a constraint the workspace already
answers: `rustok-graphql` wraps a browser-owned fetch in `send_wrapper::SendWrapper` for exactly this
bound. The panel now does the same under `target_arch = "wasm32"`, and its source is a unit marker
rather than the runtime handle, because a handle has no meaningful identity to compare and the panel
is rebuilt when the handle changes. The port is untouched, so every consumer property editor keeps
its contract.

`scripts/verify/verify-standalone-admin-editor-delivery.mjs` holds the moved list: it fails if a
listed surface creates a `LocalResource` again, if a module route loses `ssr=SsrMode::Async`, or if
module availability goes back to a `LocalResource`/`Effect`. It also fails when a file listed as
pending stops being pending, so the list cannot go stale. Nothing is pending any more: the six
surfaces the ADR names all resolve their state during the render.

## Sources of truth and ownership

- Canonical owner of the editor's server-state loading: `rustok-pages-admin` (the views that render
  the workspace), with `apps/admin` providing the host context.
- Canonical owner of the request nonce: `apps/admin/src/app/security.rs` (`admin_security_headers`,
  `select_csp`); `leptos::nonce` is a mechanism a host feeds, never a second source of policy.
- Canonical owner of the browser binding: `fly-browser` (`PAGE_BUILDER_BROWSER_HOST_BOOTSTRAP_JS`),
  which `verify-fly-ssr-first.mjs` derives the control contract from.
- Derived, never authoritative: the `Suspense` fallback, the `srcdoc`/`src` choice, and any
  `LocalResource` value.

## Invariants

- A surface that a host is expected to display in SSR HTML does not load its data through
  `LocalResource`.
- A route whose surface is expected in the response declares an SSR mode that awaits its resources;
  `OutOfOrder` is not used for such a route while the host serves no script.
- An `Effect` is never the only path from fetched data to rendered output.
- The nonce on any script of an admin response is a nonce the response's own
  `content-security-policy` names.
- Adding a hydration bundle to a host does not change what that host serves with no bundle.

### Allowed states

- Standalone SSR admin: `Resource`-loaded workspace, SSR intent forms, `fly-browser` binding the
  controls. No hydration script.
- A host that ships hydration: the bundle runs under the host-provided nonce, proven by a test that
  compares the rendered script's nonce with the header.
- A surface that cannot load on the server: the host renders why, not a dead editor.

### Forbidden states

- An admin response whose HTML contains an editor skeleton and no editor, presented as working.
- A hydration script whose nonce differs from the one in the enforcing policy.
- `LocalResource` plus an `Effect` used as if it were server-side loading.
- A `Resource` under `OutOfOrder` counted as delivered, because its value reaches the browser only
  through a script the host does not serve.

## Non-goals

- This does not convert the 72 files that use `LocalResource` in one change; each surface moves when
  it is expected to render server-side, and the rest keep the behaviour they have.
- It does not add a hydration bundle to the standalone images, and it does not delete the `hydrate`
  feature.
- It does not change `fly-browser`'s protocol or the intent endpoint.

## Data, transaction, and concurrency boundary

Loading editor state is a read. The move from `LocalResource` to `Resource` changes when a request is
issued (during rendering rather than from the browser) and therefore makes the number of reads per
page render explicit; it does not change what is read, and it adds no write, no revision, no draft
generation and no publish. A `Resource` that fails renders the error the surface already has, as the
`LocalResource` path did in the browser.

## Context dimensions

Tenant and principal reach the resource the way they already do — from the verified
`ServerAuthSnapshot` the host provides (`use_token`, `use_tenant`) — so the server-side read is made
as the caller the host verified, not as a service identity. Locale participates through the route
context as it does today. Channel and timezone do not participate.

## Events and projections

No events. Loading editor state emits nothing and reads no projection beyond the queries it already
issues.

## Failure semantics

- A server-side read that fails renders the surface's existing error state with the message, in the
  response, rather than a fallback that never resolves.
- A request without a verified session renders the unauthorised state the route already has; the
  resource is not issued at all.
- A host that ships hydration with a mismatched nonce fails its test before it ships; at runtime the
  browser refuses the script, which is the policy working as intended.

## Migration and cutover

Surfaces move one at a time, in the order a host is expected to show them: the Pages workspace
first, since it is the surface this ADR was written for. Each move carries its render test — the
same `view!` rendered under SSR must contain the workspace rather than the `Suspense` fallback — so a
surface that regresses back to `LocalResource` fails a test rather than an inspection. No route, no
feature flag and no persisted state changes.

## Alternatives considered

- **Ship the hydration bundle and call the editor delivered.** Rejected as the answer for the
  default profile: it makes every admin page pay for a WASM bundle to make one workspace
  interactive, and the editor's controls are already bound by `fly-browser`. It stays available for
  hosts that choose it, under invariant 3.
- **Keep `LocalResource` and add an `Effect`.** Rejected: shown above to reproduce the same empty
  render, because the effect is not driven before the response is produced. It would look like
  progress and change nothing.
- **Make `LocalResource` resolve under SSR.** Rejected: that is a change to the framework, and it
  would make every existing `LocalResource` do work it was never designed to do (its own doc comment
  says the value is ignored on the server so that hydration cannot mismatch).
- **Leave the host rendering the fallback and document it.** Rejected: that is the state this ADR
  exists to end; the backlog already refuses to present an editor whose controls cannot act.

## Verification

- A render test asserts the workspace HTML is present under `ssr` and that the `Suspense` fallback
  string is absent for the moved surface.
- A source gate asserts no surface named in the migration list loads its data through `LocalResource`
  — derived from that list, not from a marker in the file.
- For a hydration host: a test extracts the nonce from the rendered script and asserts its presence
  in the response's `content-security-policy`, so the two cannot drift.
- `verify-fly-ssr-first.mjs` keeps pinning the shell contract, so removing the hydration omission
  from the default profile is a change to the decision, not a silent edit.

## Consequences

The standalone admin serves an editor that works, without a WASM bundle, and the host keeps the
policy it already enforces. The cost is that every surface moved this way issues its reads during
rendering, so a slow query delays the response instead of showing a skeleton; that is the trade the
SSR-first profile already made for routing, and a surface that must not pay it stays a browser
surface and is not rendered by the host. The hydration profile remains available and now has a
stated nonce contract, so a host that chooses it cannot ship it half-wired.
