# Implementation plan for `rustok-seo-admin-support`

Status: support crate is stabilized as a reusable owner-side SEO UI layer. D6.1/D6.3 are closed; execution wave is synchronized with the remaining track (`D6.2`, `D8`, `D9`).

## Execution checkpoint

- Current phase: `phase_d6_transport_parity_followup`
- Last checkpoint: Legacy `src/api.rs` file removed; shared SEO GraphQL helpers now live in `src/transport.rs`, and `SeoEntityPanel` consumes the internal transport module directly while keeping owner-module API unchanged.
- Next step: Close D6.2 — transport helpers parity (REST primary + GraphQL secondary path) for diagnostics/sitemap/bulk control-plane read surfaces.
- Open blockers:
  - For D6.2, the minimum REST surface in the support crate needs to be agreed upon without duplicating heavy client code from Next admin.
  - For D8 integration coverage, a coordinated fixture set of permission/validation errors for Leptos owner panels is needed.
- Hand-off notes for next agent:
  - Do not move ownership entity screens into the central SEO hub.
  - Maintain host-locale contract without package-local fallback chains.
  - All new UI widgets must work identically for `pages/product/blog/forum`.
- Last updated at (UTC): 2026-06-07T22:40:00Z

## Goal

- do not duplicate SEO panel logic in `pages`, `product`, `blog`, `forum` and future content modules;
- do not turn `rustok-seo-admin` into a universal entity editor;
- keep the reusable UI/tooling layer separate from SEO runtime and from owner-module screen ownership.

## Completed

- [x] created support crate with root README and local docs;
- [x] extracted shared GraphQL helpers for `seoMeta`, `upsertSeoMeta`, `publishSeoRevision`;
- [x] legacy `src/api.rs` file removed; shared SEO GraphQL helpers now live in `src/transport.rs`, and `SeoEntityPanel` consumes the internal transport module directly;
- [x] implemented `SeoEntityPanel` for owner-side entity editors;
- [x] implemented `SeoCapabilityNotice` for capability-slot scenarios;
- [x] embedded owner-side SEO panels in `rustok-pages/admin`, `rustok-product/admin`, `rustok-blog/admin`, `rustok-forum/admin`;
- [x] removed package-local locale override: support crate reads the host effective locale, canonicalizes it and does not hold an editable locale field;
- [x] extracted reusable snippet preview/recommendation/summary widgets;
- [x] raw `structured_data` textarea replaced with typed schema input contract preserving GraphQL write parity.

## Phase D backlog (SEO integration parity)

- [x] **D6.1 — Observability/remediation widgets**
  - [x] Add reusable cards for event delivery status (pending/sent/retry/failed/dead_letter) without tight coupling to a specific owner module layout.
  - [x] Add remediation hints for diagnostics issue codes with explicit action mapping (`open_entity_editor`, `open_bulk_job`, `run_reindex`).

- [ ] **D6.2 — Transport helpers parity**
  - [ ] Extend shared transport layer with REST parity endpoints from SEO Batch D4 (diagnostics summary, bulk job detail/status, sitemap job detail).
  - [ ] Keep fallback to the current GraphQL contract while rollout flag REST parity is disabled.

- [x] **D6.3 — UX consistency gates**
  - [x] Define a unified visual/state contract for loading/error/permission/empty states.
  - [x] Tie permission hints to the canonical SEO permission model (`seo:read`, `seo:manage`).

- [ ] **D8 — Verification matrix**
  - [x] Unit tests for scoring/remediation mapping and locale wiring.
  - [ ] Integration tests for transport fallback (GraphQL/REST) and error envelope mapping.
  - [ ] Snapshot/smoke tests for reusable cards in owner layouts.

- [ ] **D9 — Docs/DoD sync**
  - [x] Update crate README/docs with operational guidance for owner modules.
  - [ ] Lock Definition of Done for reusable widget additions.

## Verification

- `cargo check -p rustok-seo-admin-support --tests --config profile.dev.debug=0`
- `cargo check -p rustok-pages-admin --config profile.dev.debug=0`
- `cargo check -p rustok-product-admin --config profile.dev.debug=0`
- `cargo check -p rustok-blog-admin --config profile.dev.debug=0`
- `cargo check -p rustok-forum-admin --config profile.dev.debug=0`
- `npm --prefix apps/next-admin run lint && npm --prefix apps/next-admin run typecheck`

## Quality backlog

- [ ] Extend integration/snapshot coverage for new observability/remediation widgets.
- [ ] Maintain transport compatibility matrix GraphQL/REST.
- [ ] Synchronize docs after each D6/D8 increment.
