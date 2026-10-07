# rustok-seo-render

## Purpose

`rustok-seo-render` is a compatibility re-export crate for `rustok-seo-storefront`. Canonical head rendering, transport adapters, and SEO storefront logic now reside in `crates/modules/rustok-seo/storefront` (`rustok-seo-storefront`). Existing Rust hosts continue to compile through this crate without breaking changes.

## Responsibilities

- render `SeoPageContext` into HTML head tags for SSR hosts
- serialize typed robots directives into canonical meta-tag content
- keep Rust-side SEO rendering aligned with the canonical `rustok-seo` contract
- prepare parity snapshots and cross-host contract checks for the SEO Phase D integration wave

## Entry points

- `rustok_seo_render::render_head_html`
- `rustok_seo_render::robots_directives`

## Interactions

- consumes the canonical SEO contract from `rustok-seo`
- uses escaping helpers from `rustok-core`
- is consumed by Rust hosts such as `apps/storefront`

## Current execution wave (Phase D)

The renderer backlog is focused on:

- deterministic snapshot coverage for complex metadata combinations
- parity fixtures between Rust SSR output and Next metadata adapters
- contract-only rendering boundaries (no SEO business logic drift)

See `docs/implementation-plan.md` for active milestones.
