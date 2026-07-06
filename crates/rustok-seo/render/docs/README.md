# `rustok-seo-render` Documentation

`rustok-seo-render` — support crate for Rust-host SEO adapters. It does not own the SEO runtime, but handles only the last mile: converting canonical `SeoPageContext` into SSR head HTML.

## Purpose

- eliminate duplication of Rust-side SEO head rendering between host applications;
- maintain a single renderer for canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links, generic meta/link tags and typed JSON-LD schema blocks;
- not create a second source of truth over `rustok-seo`.

## Scope

- pure rendering helpers without access to storage, redirect runtime and tenant policy;
- serialization of typed `SeoRobots` into a directives string for `<meta name="robots">`;
- serialization of `SeoStructuredDataBlock.payload` into `<script type="application/ld+json">` without re-classifying the schema.org type;
- HTML escaping and assembly of an SSR head string for Rust-host applications.

## Out of scope

- canonical/redirect resolution;
- locale fallback;
- metadata precedence;
- sitemap/robots runtime orchestration;
- frontend-specific Next.js mapping.

## Integration

- `apps/storefront` uses the crate as a shared Rust-side renderer instead of locally assembling head tags;
- `apps/next-frontend` remains on a TypeScript adapter layer over the built-in Next Metadata API;
- the canonical SEO contract continues to live in `rustok-seo`.

## Phase D alignment

`rustok-seo-render` participates in SEO Phase D as a parity/hardening layer:

- snapshot coverage for complex head tag combinations;
- contract fixtures for Rust renderer vs Next metadata adapter parity;
- drift guardrails to keep business logic inside `rustok-seo`.

## Verification

- `cargo check -p rustok-seo-render`
- `cargo check -p rustok-storefront`

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [`rustok-seo` documentation](../../docs/README.md)
