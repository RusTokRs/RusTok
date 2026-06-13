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
- `rustok-module.toml` — декларация slug/entry type/ui-classification;
- `contracts/page-builder-fba-registry.json` — machine-readable registry provider/consumer versions, minimum supported consumer version and fallback profile names for anti-drift gates.

## Интеграция

- `apps/server` подключает модуль через feature-флаг `mod-page-builder` и module registry codegen;
- `rustok-pages` и другие layout/content модули используют builder как consumer по contract-first path;
- host-реализации (Next/Leptos/Flutter) синхронизируются через capability contract, а не через UI 1:1.

## Provider health and SLO baseline

Machine-readable provider metadata now includes the health states `ready/degraded/unavailable`, degradation reasons (`capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`) and pilot SLO thresholds: `preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`. The registry and Wave evidence packet gates must keep these thresholds synchronized before Wave 1 promotion.

## Fallback matrix

Runtime provider-а фиксирует baseline fallback-профили в `src/rollout.rs`; consumer-модули и host adapters обязаны держать те же имена outcome.

| Профиль | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

## Проверка

- `cargo test -p rustok-page-builder --lib` — базовая проверка runtime metadata/contract surface;
- `cargo xtask module validate page_builder` — проверка publish-readiness и manifest/docs contracts;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-registry.mjs pages` — anti-drift проверка machine-readable registry против provider/consumer manifests, включая provider health states и degradation reasons.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-wave-evidence-packet.mjs` — проверка Wave 0 evidence packet, включая SLO thresholds/evaluation и correlation trace samples.

## Связанные документы

- `docs/modules/tiptap-page-builder-implementation-plan.md` — платформенный rollout-план builder-first FBA;
- `docs/modules/manifest.md` — контракт `modules.toml` / `rustok-module.toml`;
- `crates/rustok-pages/docs/implementation-plan.md` — consumer-интеграция `pages` с reference builder-модулем.
