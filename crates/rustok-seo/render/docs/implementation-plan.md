# План реализации `rustok-seo-render`

Статус: базовый renderer уже выделен из `apps/storefront` и используется как shared Rust-side SEO adapter.

## Область работы

- держать единый Rust-side renderer поверх canonical `rustok-seo::SeoPageContext`;
- не позволять host-приложениям дублировать robots/meta/link/JSON-LD serialization;
- оставлять всю SEO business logic в `rustok-seo`, а не переносить её в adapter crate.

## Текущее состояние

- crate уже публикует `render_head_html` и `robots_directives`;
- `apps/storefront` уже использует этот crate вместо локального `build_seo_head`;
- renderer покрывает canonical, hreflang, typed robots, Open Graph, Twitter, verification, pagination, generic meta/link tags и JSON-LD blocks.

## Следующий scope

- добавить snapshot/unit tests на parity сложных tag combinations;
- при появлении второго Rust storefront host переиспользовать тот же renderer без нового локального helper layer;
- при необходимости вынести дополнительные Rust-side helpers для structured data rendering, не превращая crate в SEO runtime.

## Правила обновления

1. Изменения canonical SEO contract сначала фиксируются в `rustok-seo`.
2. Затем синхронизируется renderer crate и Rust-host потребители.
3. Если меняется ownership или public API renderer-а, обновляются `README.md`, `docs/README.md` и центральные registry docs.
