# Implementation plan for `rustok-seo-render`

Status: renderer crate is stable as the canonical Rust-side SSR adapter for `SeoPageContext`. The next wave is parity hardening with host integrations within SEO Phase D (`D7`, `D8`, `D9`).

## Execution checkpoint

- Current phase: `phase_d7_renderer_parity_alignment`
- Last checkpoint: Added snapshot batch D7.1: deterministic primary snapshot assertion + nondeterministic-token normalization comparison test for parity tooling.
- Next step: Close D7.2 — expand cross-host fixture matrix Rust renderer vs Next metadata adapter.
- Open blockers:
  - This VM does not have `cargo` in `PATH`, local checks were not run.
  - Cross-host parity requires a stable REST/GraphQL `SeoPageContext` contract after SEO Batch D4.
- Hand-off notes for next agent:
  - Do not move SEO business logic into the renderer crate.
  - Any renderer changes must remain pure serialization over backend-provided `SeoPageContext`.
  - Maintain parity evidence between Rust storefront renderer and Next metadata adapter.
- Last updated at (UTC): 2026-06-07T17:45:00Z

## Scope of work

- keep a single Rust-side renderer over canonical `rustok-seo::SeoPageContext`;
- do not allow host applications to duplicate robots/meta/link/JSON-LD serialization;
- leave all SEO business logic in `rustok-seo`, not in the adapter crate.

## Current state

- crate already publishes `render_head_html` and `robots_directives`;
- `apps/storefront` uses this crate instead of local `build_seo_head`;
- renderer covers canonical, hreflang, typed robots, Open Graph, Twitter, verification, pagination, generic meta/link tags and JSON-LD blocks.

## Phase D backlog (renderer-side)

- [x] **D7.1 — Parity snapshots**
  - [ ] Add snapshot/unit tests for combinations: canonical + alternates + noindex + verification tags + multi-block JSON-LD.
  - [ ] Lock deterministic ordering for meta/link/script tags.

- [ ] **D7.2 — Cross-host contract parity**
  - [ ] Add contract tests comparing Rust renderer output and Next metadata adapter behavior on the same `SeoPageContext` fixture set.
  - [ ] Lock acceptable discrepancies (e.g., unsupported long-tail tags in Next API).

- [ ] **D8 — Verification matrix**
  - [ ] Integration smoke with `apps/storefront` SSR path and `storefront/seo-page-context` server function.
  - [ ] Regression tests on `SeoStructuredDataBlock` serialization (`schema_kind`, `schema_type`, `source`, payload).

- [ ] **D9 — Docs/DoD sync**
  - [ ] Update README/docs on parity rules and renderer/non-renderer boundary.
  - [ ] Add mini-runbook for drift between Rust renderer and Next metadata adapter.

## Update rules

1. Canonical SEO contract changes are first locked in `rustok-seo`.
2. Then the renderer crate and Rust-host consumers are synchronized.
3. If renderer ownership or public API changes, update `README.md`, `docs/README.md` and central registry docs.

## Verification

- `cargo check -p rustok-seo-render --tests --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Quality backlog

- [ ] Add snapshot coverage for parity-critical tag combinations.
- [ ] Maintain contract fixtures for Rust/Next parity.
- [ ] Update execution checkpoint after each D7/D8 increment.
