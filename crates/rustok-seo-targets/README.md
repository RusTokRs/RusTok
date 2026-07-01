# rustok-seo-targets

## Purpose

`rustok-seo-targets` provides the backend capability contract for registry-backed SEO targets in RusToK.

## Responsibilities

- Define the canonical validated `SeoTargetSlug` contract used by SEO runtime, GraphQL, Leptos server functions, and bulk tooling.
- Expose the runtime target registry and provider interfaces used by owner modules to register SEO-capable entities.
- Carry owner-aware registry metadata (`display_name`, `owner_module_slug`) so shared operator surfaces do not hardcode target labels.
- Keep target extensibility module-owned without introducing a second tenant-aware SEO module.
- Provide typed backend records for route resolution, loaded target state, bulk summaries, and sitemap candidates.
- Own the independent `SeoTargetImageRecord` DTO (`url/alt/size/mime` + derived helpers); owner modules map media/domain records at their SEO provider boundaries.
- Provide small JSON-LD schema builders for owner providers so built-in targets do not hand-roll raw schema blobs.

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
- `schema::{web_page, web_page_with_image, collection_page, collection_page_with_image, product, product_with_image, blog_posting, blog_posting_with_image, discussion_forum_posting, discussion_forum_posting_with_image}`
- `populate_image_template_fields`
- `register_seo_target_provider`
- `seo_target_registry_from_extensions`
