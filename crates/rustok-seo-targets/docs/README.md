# `rustok-seo-targets`

`rustok-seo-targets` — support crate для runtime-регистрации SEO targets без создания отдельного tenant-aware модуля.

## Что фиксирует crate

- канонический extensible contract для target kind через `SeoTargetSlug`, а не через hardcoded enum;
- registry/provider pattern для owner backend-модулей;
- registry entry metadata (`display_name`, `owner_module_slug`) для shared operator/admin surfaces;
- capability flags `authoring`, `routing`, `bulk`, `sitemaps`;
- typed backend records для route match, loaded target, bulk summary и sitemap candidate;
- минимальные JSON-LD builders для built-in rich-snippet shapes, чтобы owner providers не собирали schema.org payload как raw `json!` blobs;
- runtime wiring через `ModuleRuntimeExtensions`, а не через manifest-магии.

## Что crate не делает

- не является модулем из `modules.toml`;
- не хранит tenant settings и не делает SEO persistence сам;
- не владеет GraphQL, Leptos UI или storefront rendering;
- не подменяет `rustok-seo`, а только даёт ему extensibility seam.

## Runtime pattern

1. Host строит единый `ModuleRuntimeExtensions`.
2. Owner modules в `register_runtime_extensions(...)` регистрируют свои SEO providers.
3. `rustok-seo` достаёт общий `Arc<SeoTargetRegistry>` из runtime context и использует его во всех entrypoints.
4. Добавление нового SEO-capable backend-модуля больше не требует hardcoded ветки в `rustok-seo`.

## Поля для SEO-шаблонов

`SeoLoadedTargetRecord.template_fields` — единственный допустимый канал данных для template-generated SEO. Provider обязан отдавать только SEO-safe значения:

- `title`;
- `description`;
- `locale`;
- `route`;
- slug/handle/id поля, которые нужны для шаблонов (`slug`, `handle`, `category_id`, `topic_id`).

Owner module не должен отдавать сырой HTML, произвольный JSON или внутренние DTO в template runtime. Шаблоны рендерит только `rustok-seo`; provider отвечает только за typed target loading и безопасный field map.

## JSON-LD builders

`rustok-seo-targets::schema` даёт небольшие typed builders для текущих owner providers:

- `web_page`;
- `collection_page`;
- `product`;
- `blog_posting`;
- `discussion_forum_posting`.
- `offer`;
- `review`;
- `breadcrumb_list`;
- `faq_page`.

Для `offer` helper действует минимальная нормализация: `price` пишется только для finite значений, `priceCurrency` — только для валидного трёхбуквенного alphabetic кода (кроме `XXX`), `availability` — только для `http(s)://schema.org/<OfferAvailability>` из поддерживаемого набора (`InStock`, `OutOfStock`, `PreOrder` и т.д.).

Эти helpers не являются полноценным schema editor. Они фиксируют безопасный baseline для fallback/generated rich snippets: обязательный `@context`, корректный `@type`, пропуск пустых optional полей и единый shape для `pages/product/blog/forum`. Более богатые Product Offer/Review, FAQ/HowTo, BreadcrumbList, ItemList и Organization/LocalBusiness должны наращиваться через этот же typed слой, а не через host-local schema.org classifier.
