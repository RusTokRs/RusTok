---
id: doc://docs/modules/admin-parity.md
kind: architecture_reference
language: en
status: active
---

# Admin parity: Rust FFA admin vs classic Next admin

Both admins consume the **same GraphQL**; each module keeps its own UI. On the
Rust side that UI is the module-owned FFA package (`crates/modules/*/admin`, one
Leptos connector, composed by `apps/admin` via `/modules/:module_slug`). On the
Next side it is the module-owned package `apps/next-admin/packages/*`
(`@rustok/<module>-admin`, registered in `apps/next-admin/src/modules/index.ts`).
This matrix is the parity source of truth; remaining work is tracked in
`docs/modules/page-builder-backlog.md` (platform-wide items) — update this file
when a surface lands, do not fork a second matrix.

Legend: ✅ module-owned surface exists · ❌ missing · ◐ exists under a different
owner/shape (noted).

| Module / surface | Rust FFA | Next | Notes |
|---|---|---|---|
| AI | ✅ | ✅ | |
| Auth (users/roles UI) | ✅ host features | ✅ host features | Shell-level, both |
| Billing | — | ✅ host route | No Rust module crate; platform surface |
| Blog | ✅ | ✅ | Both implement post CRUD, lifecycle, channel scoping, image/SEO, scheduling and comment moderation. Category CRUD is currently Next-only; preview/pin and localization parity still require work. |
| Brand | ✅ | ❌ | |
| Cache | ✅ | ✅ | |
| Channel | ✅ | ❌ | |
| Comments | ✅ | ❌ | |
| Commerce (orders/returns/shipping) | ✅ | ✅ `@rustok/commerce-admin` | Covers order/fulfillment/pricing surfaces in Next |
| Customer | ✅ | ❌ | |
| Design system | ✅ host page | ✅ host route | Shell-level, both |
| Email | ✅ | ✅ | |
| Events | ✅ (`events-module`) | ✅ | |
| Exclusive / workspaces | — | ✅ host routes | No Rust counterpart yet |
| Forms | ✅ | ✅ `@rustok/forms-admin` (2026-10-14) | Submission inbox + triage states |
| Forum | ✅ | ✅ | |
| Groups | ✅ | ❌ | |
| Iggy connector | ✅ | ✅ | |
| Index | ✅ | ❌ | Largely internal |
| Inventory | ✅ | ◐ via commerce package | |
| Marketplace (listing/seller) | ✅ | ❌ | |
| MCP | ✅ | ✅ | |
| Media | ✅ | ✅ `@rustok/media-admin` (2026-10-14) | Library list/usage/delete |
| Modules registry | ✅ host page | ✅ host route | Shell-level, both |
| Notifications | ✅ | ❌ | |
| Outbox | ✅ | ❌ | Largely internal |
| Page builder + Pages | ✅ | ✅ `@rustok/pages-admin` (2026-10-14) | Next: manage + deep-link to the Rust builder; no second editor |
| Pricing / product bundles / relations | ✅ | ◐ via commerce/product packages | |
| Product | ✅ | ✅ | |
| RBAC | ✅ | ✅ | |
| Region | ✅ | ❌ | |
| Search | ✅ | ✅ | |
| SEO | ✅ | ◐ host feature (`src/features/seo`) | Functional, but host-owned instead of module-owned in Next |
| Tenant | ✅ | ◐ via workspaces/org switcher | Partial |
| Translation | ✅ | ✅ | |
| Workflow | ✅ | ✅ | |

## Consistency rules

- New module capability ships in **both** module-owned UIs or is explicitly
  recorded here as one-sided with a reason (internal-only, shell-level, or
  deep-linked — e.g. the visual builder is Rust-FFA-only by design and Next
  deep-links to it).
- Both surfaces use the same GraphQL operations and the same permission model;
  a capability that needs a new operation adds it to the module's GraphQL
  first, never to one admin's private endpoint.
- Host shells (`apps/admin`, `apps/next-admin` app routes) only compose module
  packages and pass session/tenant context; they do not grow business UI.

## Blog feature parity (current implementation)

The two admin packages are independently implemented but must obey the Blog
owner's lifecycle, locale, channel, SEO, taxonomy, and optimistic revision
contracts. A checkmark in the table means that the UI surface exists, **not**
that every feature has equivalent controls.

| Capability | Leptos admin | Next admin | Owner rule |
| --- | --- | --- | --- |
| Post CRUD / publish / unpublish | Yes | Yes | Preserve revisions and honor rejected mutations |
| Archive / restore | Yes | Yes | Archived posts must be restored before publishing |
| Post categories | Post selector | Category CRUD and post selector | Taxonomy remains canonical; category management UI is still Next-only |
| Channel visibility | Editable | Editable | Empty allowlist is unrestricted across public channels |
| Scheduled publication | RFC3339 editor for create/draft | Local-time editor for create/draft | Backend owns publish timing and state transitions |
| Featured image | Editable URL | Editable URL | Absolute HTTP(S) or safe root-relative URLs |
| SEO | Blog-localized fields plus explicit owner SEO panel | Blog-localized SEO fields | Effective SEO composes base copy and explicit overrides |
| Comments moderation | Yes | Yes | Comments owner gates permissions, availability, retention |
| Preview tokens / pinned posts | No direct controls | No direct controls | Canonical Blog owner operations exist |

The remaining gaps should not be concealed by parallel client-specific
defaults. Changes must exercise the same canonical owner API under both the
native server-function and GraphQL transport profiles, and verify that saving
in one admin does not destroy fields set by the other.
