# Next Admin Implementation Plan

## Current Contract

`apps/next-admin` is a Next.js composition host. It owns the
Next App Router shell, shared runtime context, host navigation and package
registration. Module and capability UI lives in package-owned surfaces under
`apps/next-admin/packages/*` or external `@rustok/*-admin` packages.

The live host structure is:

- `src/app/` for Next routes and layouts;
- `src/shared/` for shared API, auth, i18n, UI and utility contracts;
- `src/entities/` for host-local read models;
- `src/widgets/` for composite shell UI;
- `src/features/` only for host-owned composition and platform screens;
- `packages/*` for module-owned Next admin surfaces.

Routes and navigation must import package entrypoints for module UI. Package
entrypoints must not re-export `src/features/*` implementations.

## Active Work

- Keep module navigation registry-driven and filtered by enabled module slug.
- Keep locale selection host-owned through `x-rustok-effective-locale` and
  `next-intl`.
- Keep GraphQL/REST access centralized in `src/shared/api` and package-owned API
  modules instead of page-local clients.
- Keep the module operator page in parity with Leptos for owner-projected
  federated-registry freshness. The shared GraphQL client exposes logical
  registry ID, status, last success, and consecutive failures only.
- Keep typed `snake_case` URL query keys aligned with the Leptos admin host.
- Keep starter-only routes (`billing`, `exclusive`, `workspaces`,
  `workspaces/team`) returning `notFound()`.
- Prepare the atomic
  [Richtext cutover](../../../docs/modules/rich-text-implementation-plan.md):
  the shared framed vanilla Tiptap runtime is now wired through
  `@rustok/richtext`; `/richtext/frame` and its hashed assets are auth-exempt,
  immutable capability routes, and the Blog/Forum editor adapters consume
  host `next-intl` messages. Owner-selected content locale, derived direction,
  spellcheck and dynamic read-only state now update without remounting the
  iframe; the Blog edit form takes that locale from the post's requested or
  effective translation instead of overwriting it with the host UI locale.
  Read-only Blog detail uses the editor-free `@rustok/richtext/view` boundary
  over the typed server projection, so Tiptap is not imported for display.
  The Forum owner package now provides matching topic create/edit and reply
  composition routes over its GraphQL transport; the topic editor preserves
  the requested/effective content locale and keeps route/category operations
  separate from translation editing. Remaining work is complete mounted owner
  parity without adding a second Markdown/raw-JSON mode.

## Open Improvement Areas

- Add focused package boundary checks for `packages/*` entrypoints and route
  imports.
- Expand contract tests for API mapping and typed client validation.
- Align loading, empty, error and permission-gated states with `apps/admin`.
- Add client telemetry events and correlation-id propagation for critical admin
  flows.
- Strengthen route/action RBAC guard coverage.
- Add Next/Leptos parity, CSP-frame, accessibility, lazy-bundle, and
  save/reload coverage for the shared richtext editor and server-rendered view.

The Chromium frame spike currently covers opaque-origin isolation, private
channel sequencing, CSP headers, canonical document changes, owner content
locale/direction/spellcheck updates and dynamic read-only/editable transitions.
The Leptos Trunk/SSR host copies the same immutable assets, and Blog article plus
Forum topic forms mount the shared frame with owner locale and busy state. Next
admin now mounts Forum topic create/edit and reply creation through the owner
package, and the Leptos owner package now provides matching reply creation over
native SSR/hydrate and GraphQL CSR paths. Firefox/WebKit coverage, comment
authoring, dirty-locale-switch policy and mounted save/reload evidence remain
open.

## Verification

For Next admin host/package changes, run:

```powershell
npm.cmd run typecheck
npm.cmd run lint
npm.cmd run verify:i18n:ui
npm.cmd run verify:i18n:contract
git diff --check
```

When touching package-owned module surfaces, also verify the matching backend or
Leptos parity contract where applicable.
