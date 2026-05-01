# Документация `rustok-seo-render`

`rustok-seo-render` — support crate для Rust-host адаптеров SEO. Он не владеет SEO runtime, а отвечает только за последнюю милю: превращает canonical `SeoPageContext` в SSR head HTML.

## Назначение

- убрать дублирование Rust-side SEO head rendering между host-приложениями;
- держать один renderer для canonical, robots, hreflang, Open Graph, Twitter, verification tags, pagination links, generic meta/link tags и typed JSON-LD schema blocks;
- не создавать второй source of truth поверх `rustok-seo`.

## Зона ответственности

- pure rendering helpers без доступа к storage, redirect runtime и tenant policy;
- сериализация typed `SeoRobots` в строку directives для `<meta name="robots">`;
- сериализация `SeoStructuredDataBlock.payload` в `<script type="application/ld+json">` без повторной классификации schema.org типа;
- HTML escaping и сборка SSR head string для Rust-host приложений.

## Что не входит

- canonical/redirect resolution;
- locale fallback;
- metadata precedence;
- sitemap/robots runtime orchestration;
- frontend-specific Next.js mapping.

## Интеграция

- `apps/storefront` использует crate как shared Rust-side renderer вместо локальной сборки head tags;
- `apps/next-frontend` остаётся на TypeScript adapter слое поверх built-in Next Metadata API;
- canonical SEO contract и дальше живёт в `rustok-seo`.

## Проверка

- `cargo check -p rustok-seo-render`
- `cargo check -p rustok-storefront`

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Документация `rustok-seo`](../../docs/README.md)
