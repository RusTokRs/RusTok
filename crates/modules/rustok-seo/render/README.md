# rustok-seo-render

## Purpose

`rustok-seo-render` is a Rust-host support crate for rendering `rustok-seo` metadata into SSR HTML head tags. It keeps Rust storefront hosts on a shared renderer instead of duplicating tag serialization logic per app.

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
