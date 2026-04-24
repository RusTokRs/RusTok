# rustok-seo-targets

## Purpose

`rustok-seo-targets` provides the backend capability contract for registry-backed SEO targets in RusToK.

## Responsibilities

- Define the canonical validated `SeoTargetSlug` contract used by SEO runtime, GraphQL, Leptos server functions, and bulk tooling.
- Expose the runtime target registry and provider interfaces used by owner modules to register SEO-capable entities.
- Carry owner-aware registry metadata (`display_name`, `owner_module_slug`) so shared operator surfaces do not hardcode target labels.
- Keep target extensibility module-owned without introducing a second tenant-aware SEO module.
- Provide typed backend records for route resolution, loaded target state, bulk summaries, and sitemap candidates.

## Interactions

- Registered by owner backend modules such as `rustok-pages`, `rustok-product`, `rustok-blog`, and `rustok-forum`.
- Consumed by `rustok-seo` to resolve metadata, routing, bulk operations, redirects, and sitemap generation through one shared runtime registry.
- Wired into host runtimes through `rustok-core::ModuleRuntimeExtensions`.
- Runtime consumers are expected to fail fast when the shared registry is missing instead of silently falling back to hardcoded built-ins.
- Not listed in `modules.toml` and not tenant-toggled directly.

## Entry points

- `SeoTargetSlug`
- `SeoTargetProvider`
- `SeoTargetRegistry`
- `register_seo_target_provider`
- `seo_target_registry_from_extensions`
