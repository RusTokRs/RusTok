# ModuleRuntimeExtensions для runtime-capabilities

- Date: 2026-04-20
- Status: Accepted

## Context

Платформа уже использует support/capability crates рядом с tenant-aware модулями, но до этого не было
одного канонического runtime-паттерна, через который owner-модули могли бы регистрировать backend
capabilities без правки central core-модуля под каждый новый target/provider.

SEO показал этот разрыв особенно явно:

- `rustok-seo` оставался единственным tenant-aware SEO module;
- persisted storage уже использовал string `target_kind`, но Rust/runtime contract держался на закрытом enum;
- добавление нового SEO-capable backend-модуля требовало hardcoded dispatch внутри `rustok-seo`;
- host runtime уже имел общий `ModuleRegistry`, но не имел общего typed extension-registry, который
  могли бы наполнять модули во время bootstrap.

Нужен был общий platform pattern, а не ad-hoc исключение только для SEO.

## Decision

Платформа вводит module-owned runtime capability registration через `rustok-core::ModuleRuntimeExtensions`.

Приняты следующие правила:

- `RusToKModule` получает hook `register_runtime_extensions(&mut ModuleRuntimeExtensions)`;
- host (`apps/server`) строит один общий `ModuleRuntimeExtensions` после `build_registry()` и вызывает
  этот hook у всех зарегистрированных модулей;
- итоговый `ModuleRuntimeExtensions` кладётся в shared runtime store и в GraphQL schema data;
- support/capability crates публикуют typed registries поверх этого механизма, но не становятся от
  этого tenant-aware модулями сами по себе.

Для SEO это зафиксировано так:

- `rustok-seo-targets` становится support/capability crate;
- канонический public contract target kind = validated string `SeoTargetSlug`;
- owner backend-модули (`pages`, `product`, `blog`, `forum`) сами регистрируют свои
  `SeoTargetProvider` в runtime registry;
- `rustok-seo` получает один shared `Arc<SeoTargetRegistry>` из runtime context и использует его во
  всех entrypoints: GraphQL, HTTP, Leptos `#[server]`, storefront SSR helpers и background workers.

Manifest schema при этом не расширяется отдельной секцией runtime-capabilities. Source of truth для
таких registration seams остаётся Rust-side hook.

## Consequences

- Добавление нового SEO-capable backend-модуля больше не требует правки `rustok-seo` core.
- Появляется повторно используемый platform pattern для других runtime-capabilities, а не только для SEO.
- Host runtime обязан инициализировать `ModuleRuntimeExtensions` один раз и прокидывать его во все
  shared entrypoints.
- Support/capability crate по-прежнему не считается platform module только из-за участия в runtime wiring.
- Module authors теперь должны документировать не только manifest wiring, но и runtime capability
  registration, если модуль публикует provider seam через `ModuleRuntimeExtensions`.
