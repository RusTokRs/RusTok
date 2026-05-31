# rustok-page-builder: runtime-контракт

`rustok-page-builder` — референсный FBA-модуль визуального билдера.

## Назначение

Модуль вводит самостоятельный capability-контур билдера до интеграции в `pages`.
Это позволяет закрепить FBA-first delivery и контрактную совместимость между host-реализациями.

## Зона ответственности

- самостоятельный FBA reference-контур visual builder до интеграции в доменные consumer-модули;
- владение vendor-neutral payload contract (`grapesjs_v1`) и capability boundaries `preview/tree/properties/publish`;
- lifecycle/health/observability seams для rollout и безопасного tenant-by-tenant включения.

## Ответственности

- owner контракта visual builder payload (`grapesjs_v1`) на модульном уровне;
- lifecycle-рамка для rollout/health/observability в терминах FBA;
- совместимость с consumer-модулями по contract-first интеграции.

## Точки входа

- `src/lib.rs` — runtime metadata и permission surface;
- `rustok-module.toml` — декларация slug/entry type/ui-classification.

## Интеграция

- `apps/server` подключает модуль через feature-флаг `mod-page-builder` и module registry codegen;
- `rustok-pages` и другие layout/content модули используют builder как consumer по contract-first path;
- host-реализации (Next/Leptos/Flutter) синхронизируются через capability contract, а не через UI 1:1.

## Fallback matrix

Runtime provider-а фиксирует baseline fallback-профили в `src/rollout.rs`; consumer-модули и host adapters обязаны держать те же имена outcome.

| Профиль | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Expected typed error |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `builder` |

## Проверка

- `cargo test -p rustok-page-builder --lib` — базовая проверка runtime metadata/contract surface;
- `cargo xtask module validate page_builder` — проверка publish-readiness и manifest/docs contracts.

## Связанные документы

- `docs/modules/tiptap-page-builder-implementation-plan.md` — платформенный rollout-план builder-first FBA;
- `docs/modules/manifest.md` — контракт `modules.toml` / `rustok-module.toml`;
- `crates/rustok-pages/docs/implementation-plan.md` — consumer-интеграция `pages` с reference builder-модулем.
