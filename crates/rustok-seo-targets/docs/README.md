# `rustok-seo-targets`

`rustok-seo-targets` — support crate for runtime registration of SEO targets without creating a separate tenant-aware module.

## What the crate defines

- a canonical extensible contract for target kind via `SeoTargetSlug`, not via a hardcoded enum;
- a registry/provider pattern for owner backend modules;
- registry entry metadata (`display_name`, `owner_module_slug`) for shared operator/admin surfaces;
- capability flags `authoring`, `routing`, `bulk`, `sitemaps`;
- typed backend records for route match, loaded target, bulk summary and sitemap candidate;
- an independent image boundary DTO `SeoTargetImageRecord` for OG/Twitter/schema fallback without a dependency on `rustok-media`; it carries an optional owner-provided `media_asset_id` for consumers that need the canonical media descriptor;
- minimal JSON-LD builders for built-in rich-snippet shapes, so owner providers do not construct schema.org payloads as raw `json!` blobs;
- helper `populate_image_template_fields` for image-aware SEO templates;
- runtime wiring through `ModuleRuntimeExtensions`, not through manifest magic.

## What the crate does not do

- it is not a module from `modules.toml`;
- it does not store tenant settings and does not do SEO persistence itself;
- it does not own GraphQL, Leptos UI or storefront rendering;
- it does not replace `rustok-seo`, but only provides it with an extensibility seam.

## Runtime pattern

1. Host builds unified `ModuleRuntimeExtensions`.
2. Owner modules in `register_runtime_extensions(...)` register their SEO providers.
3. `rustok-seo` retrieves shared `Arc<SeoTargetRegistry>` from runtime context and uses it in all entrypoints.
4. Adding new SEO-capable backend-module no longer requires hardcoded branch in `rustok-seo`.

## Fields for SEO templates

`SeoLoadedTargetRecord.template_fields` — the only allowed data channel for template-generated SEO. The provider must only return SEO-safe values:

- `title`;
- `description`;
- `locale`;
- `route`;
- slug/handle/id fields needed for templates (`slug`, `handle`, `category_id`, `topic_id`);
- image-aware template keys populated via `SeoTargetImageRecord` (`image_url`, `image_alt`, `image_width`, `image_height`, `image_mime`, `image_extension`, `image_pixel_count`, `image_aspect_ratio`, `image_has_alt`, `image_has_size`, `image_count`).

The owner module must not pass raw HTML, arbitrary JSON or internal DTOs to the template runtime. Templates are rendered only by `rustok-seo`; the provider is only responsible for typed target loading and a safe field map.

## JSON-LD builders

`rustok-seo-targets::schema` provides small typed builders for current owner providers:

- `web_page` / `web_page_with_image`;
- `collection_page` / `collection_page_with_image`;
- `product` / `product_with_image`;
- `blog_posting` / `blog_posting_with_image`;
- `discussion_forum_posting` / `discussion_forum_posting_with_image`;
- `offer`;
- `review`;
- `breadcrumb_list`;
- `faq_page`.

For the `offer` helper, minimal normalization applies: `price` is written only for finite values, `priceCurrency` — only for a valid three-letter alphabetic code (except `XXX`), `availability` — only for `http(s)://schema.org/<OfferAvailability>` from the supported set (`InStock`, `OutOfStock`, `PreOrder` etc.).

These helpers are not a full schema editor. They establish a safe baseline for fallback/generated rich snippets: mandatory `@context`, correct `@type`, omission of empty optional fields and a unified shape for `pages/product/blog/forum`. Richer Product Offer/Review, FAQ/HowTo, BreadcrumbList, ItemList and Organization/LocalBusiness should be built on this same typed layer, not via a host-local schema.org classifier.
