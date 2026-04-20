# rustok-seo-admin

Leptos admin UI for `rustok-seo`.

## Purpose

This package ships the module-owned admin surface for cross-cutting SEO infrastructure.
Entity-specific SEO authoring now lives in owner-module admin packages.

## Responsibilities

- manage manual redirects exposed by `rustok-seo`
- edit tenant-scoped SEO defaults through the shared module settings contract
- preview tenant-level `robots.txt` and linked public URLs
- trigger sitemap generation and show the latest sitemap status
- surface cross-cutting SEO diagnostics without taking over entity editors
- keep the control-plane route state URL-owned through the typed `tab` query key
- stay out of page/product/blog/forum entity editors

## Entry points

- root export: `admin/src/lib.rs`
- route/query shell: `admin/src/component.rs`
- section components: `admin/src/sections/`
- form/view-model layer: `admin/src/model.rs`
- native server functions: `admin/src/api.rs`
- locale copy helper: `admin/src/i18n.rs`

## Interactions

- depends on `rustok-seo` for the service and DTO contracts
- runs inside `apps/admin` through manifest-driven module discovery
- keeps the UI package split into `lib/component/model/api/i18n/sections`, so the module-owned admin surface stays route-driven without collapsing back into one monolithic file
- now owns the full infrastructure control-plane surface for redirects, sitemaps, robots preview, tenant defaults, and diagnostics
