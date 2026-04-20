# rustok-seo

## Purpose

`rustok-seo` provides a tenant-aware SEO runtime for RusToK. It owns explicit SEO metadata overrides, manual redirects, sitemap generation, robots.txt rendering, and a storefront-facing page-context contract for SSR metadata generation across GraphQL, REST, and Leptos server-function surfaces.

## Responsibilities

- resolve SEO page context for `page`, `product`, `blog_post`, `forum_category`, and `forum_topic`
- keep public forum topic SEO resolution channel-aware when a topic is restricted by forum channel access
- merge explicit SEO overrides with existing domain metadata fallbacks
- manage manual redirects and canonical overrides
- generate sitemap files and serve `robots.txt`
- expose a headless REST read path for `SeoPageContext` at `/api/seo/page-context`, reusing canonical request locale/channel context
- provide shared SEO capability contracts that owner modules can embed into their own admin UI
- expose admin and storefront read/write surfaces through GraphQL, HTTP, and Leptos server functions

## Entry points

- runtime module: `rustok_seo::SeoModule`
- GraphQL: `rustok_seo::graphql::{SeoQuery, SeoMutation}`
- HTTP routes: `rustok_seo::controllers::routes`
- cross-cutting admin UI: `crates/rustok-seo/admin`
- Rust renderer support: `crates/rustok-seo/render`

## Interactions

- reads canonical routing substrate from `rustok-content`
- reads page/blog/product/forum content from `rustok-pages`, `rustok-blog`, `rustok-product`, and `rustok-forum`
- consumes tenant/module settings from `rustok-tenant`
- is mounted by `apps/server`, consumed by `apps/storefront`, and shared with `apps/next-frontend`
- reuses host-provided `RequestContext.channel_slug` on REST/GraphQL/Leptos SSR paths so restricted forum topics only resolve SEO in the matching public channel
- pairs with `rustok-seo-render` for Rust-host SSR head rendering without moving SEO resolution out of the module
- is expected to integrate with owner-module admin surfaces in `rustok-pages`, `rustok-product`,
  `rustok-blog`, and `rustok-forum`; `rustok-seo/admin` is reserved for cross-cutting SEO
  infrastructure rather than long-term ownership of entity editors
