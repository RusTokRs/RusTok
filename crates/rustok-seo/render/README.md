# rustok-seo-render

## Purpose

`rustok-seo-render` is a Rust-host support crate for rendering `rustok-seo` metadata into SSR HTML head tags. It keeps Rust storefront hosts on a shared renderer instead of duplicating tag serialization logic per app.

## Responsibilities

- render `SeoPageContext` into HTML head tags for SSR hosts
- serialize typed robots directives into canonical meta-tag content
- keep Rust-side SEO rendering aligned with the canonical `rustok-seo` contract

## Entry points

- `rustok_seo_render::render_head_html`
- `rustok_seo_render::robots_directives`

## Interactions

- consumes the canonical SEO contract from `rustok-seo`
- uses escaping helpers from `rustok-core`
- is consumed by Rust hosts such as `apps/storefront`
