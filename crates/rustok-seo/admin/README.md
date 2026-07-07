# rustok-seo-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI for `rustok-seo`.

## Purpose

This package ships the module-owned admin surface for cross-cutting SEO infrastructure.
Entity-specific SEO authoring now lives in owner-module admin packages.

## Responsibilities

- run the central SEO bulk editor for one target kind and one locale per job
- derive available bulk target kinds plus owner/display metadata from the shared SEO target registry instead of a hardcoded enum
- queue bulk apply, CSV export, and CSV import jobs without taking over entity editors
- enforce explicit bulk remediation modes: preview only, apply missing only, overwrite generated only, or force overwrite explicit
- expose bulk job history and tenant-scoped artifact downloads
- manage manual redirects exposed by `rustok-seo`
- edit tenant-scoped SEO defaults through the shared module settings contract
- manage tenant-scoped SEO template defaults and per-target template overrides
- preview tenant-level `robots.txt` and linked public URLs
- trigger sitemap generation and show the latest sitemap status
- surface cross-cutting SEO diagnostics, readiness scoring, issue aggregates, canonical redirect issues, hreflang gaps, and explicit/generated/fallback source counts without taking over entity editors
- keep the control-plane route state URL-owned through the typed `tab` query key
- stay out of page/product/blog/forum entity editors

## Entry points

- root export and composition boundary: `admin/src/lib.rs`
- framework-agnostic form/view-model/request policy layer: `admin/src/core.rs`
- module-owned native/server-function transport facade: `admin/src/transport.rs`
- explicit Leptos render adapter and route/query shell: `admin/src/ui/leptos.rs`
- section components: `admin/src/sections/`
- locale copy helper: `admin/src/i18n.rs`

## Interactions

- depends on `rustok-seo` for the service and DTO contracts
- runs inside `apps/admin` through manifest-driven module discovery
- keeps the UI package split into `core/transport/ui/leptos/sections/i18n`, so the module-owned admin surface has an FFA boundary while staying route-driven and avoiding a monolithic file
- now owns the full infrastructure control-plane surface for bulk jobs, bulk remediation modes, redirects, sitemaps, robots preview, tenant defaults, SEO templates, and diagnostics
