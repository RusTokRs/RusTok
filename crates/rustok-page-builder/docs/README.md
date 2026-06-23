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
- `src/dto.rs` — transport-neutral DTO, `PageBuilderContractMetadata::BASELINE` и typed error catalog (`validation/sanitize/runtime/feature-disabled`) для contract package без привязки к transport adapters;
- `src/service.rs` — transport-neutral `PageBuilderCapabilityService`, `ReferencePageBuilderService` для compile-free provider baseline, feature-flag guard и server-side handler seam с RBAC permission checks;
- `src/transport.rs` — canonical transport bridge для GraphQL, Leptos `#[server]` и future mobile adapters поверх `AuthorizedPageBuilderHandlers::handle`;
- `src/health.rs` — типизированные provider health states, degradation reasons, `ProviderHealthEvidence` и evaluator pilot SLO thresholds для release-gate evidence;
- `rustok-module.toml` — декларация slug/entry type/ui-classification;
- `contracts/page-builder-fba-registry.json` — machine-readable registry provider/consumer versions, minimum supported consumer version and fallback profile names for anti-drift gates.
- `contracts/page-builder-flutter-wave-handoff.json` — machine-readable Flutter Wave hand-off contract for device/runtime evidence without duplicating FBA registry thresholds or control-plane toggle semantics in mobile.

## Интеграция

- `apps/server` подключает модуль через feature-флаг `mod-page-builder` и module registry codegen;
- `rustok-pages` и другие layout/content модули используют builder как consumer по contract-first path;
- host-реализации (Next/Leptos/Flutter) синхронизируются через capability contract, а не через UI 1:1.


## Transport-neutral contract package

Baseline DTO package теперь содержит `PageBuilderContractMetadata::BASELINE` с canonical provider slug `page_builder`, contract `grapesjs_v1`, `builder_contract_version = 1.0`, `consumer_min_version = 1.0` и capability set `preview/tree/properties/publish`. Это минимальный publish-ready marker для adapters: GraphQL, Leptos server functions и future mobile codegen должны брать имена capability из contract metadata/registry, а не вводить transport-local aliases.

`PageBuilderCapabilityRequest` и `PageBuilderCapabilityResponse` задают tagged-envelope для transport adapters: GraphQL resolvers, Leptos `#[server]` functions и future mobile bridge могут принимать один canonical request envelope и dispatch через `AuthorizedPageBuilderHandlers::handle`. Такой seam удерживает RBAC, rollout guard и write-semantics enforcement в одном месте и не позволяет transport layer повторно изобретать имена capability или локальные error envelopes.

Первый transport bridge slice добавил `PageBuilderTransportKind`, `PageBuilderTransportSuccess`, `PageBuilderTransportError`, `dispatch_transport_envelope`, `dispatch_graphql_envelope` и `dispatch_leptos_server_function_envelope`. GraphQL/server-function adapters должны вызывать эти dispatch helpers, а затем маппить success/error envelope в свой framework-specific result; `PageBuilderTransportError` берёт `kind` и `stable_code` из `PageBuilderServiceError::kind()` / `stable_code()`, поэтому transport не владеет отдельным error catalog.

## Reference provider baseline

`ReferencePageBuilderService` закрывает минимальный capability API baseline без vendor lock-in и без persistence side effects. Provider принимает только `grapesjs_v1`, валидирует `page_id`, `revision_id`, object-shaped `project_data` / `properties`, возвращает typed `validation` errors для contract violations и typed `sanitize` errors для forbidden preview HTML (`<script`). `preview` формирует deterministic HTML wrapper `data-rustok-page-builder="grapesjs_v1"`, `properties` echo-возвращает canonical node properties, а `publish` возвращает typed `PublishPageBuilderResult` только после contract validation. Реальная persistence/rendering adapter-реализация может заменить reference provider за тем же `PageBuilderCapabilityService`, не меняя DTO, RBAC, rollout или transport bridge.

## Provider health and SLO baseline

Machine-readable provider metadata включает health states `ready/degraded/unavailable`, degradation reasons (`capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`) и pilot SLO thresholds: `preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`. Runtime-код exposes тот же baseline через `ProviderHealthState`, `ProviderDegradationReason`, `ProviderSloThresholds::PILOT`, `ProviderHealthSnapshot::evaluate` и `ProviderHealthEvidence::from_observations`, чтобы Wave evidence можно было формировать без transport-specific adapters. Registry и Wave evidence packet gates должны держать эти thresholds синхронизированными до Wave 1 promotion.

Правила health evaluation намеренно консервативны: breach preview p95 или runtime error-rate помечает provider как `provider_unhealthy`, breach sanitize threshold помечает `sanitize_backpressure`, breach publish p95 помечает `publish_backlog`, а runtime error-rate выше двойного pilot threshold переводит state в `unavailable`; иначе непустой набор degradation reasons даёт `degraded`.

## Типизированный каталог ошибок

Runtime provider-а exposes те же error-семантики, которые объявлены в `rustok-module.toml` и `contracts/page-builder-fba-registry.json`: `PageBuilderErrorKind::ALL` покрывает `validation`, `sanitize`, `runtime` и `feature-disabled`, а `PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE` закрепляет стабильный degraded-mode code `FEATURE_DISABLED`. `PageBuilderServiceError::kind()` и `PageBuilderServiceError::stable_code()` являются transport-neutral bridge для GraphQL, Leptos server functions и future mobile codegen adapters, поэтому adapters должны маппить provider errors из этих typed markers вместо локальных имён ошибок.

## Карта permission для capability

Server-side capability handlers enforce стабильную page permission map перед делегированием в provider service. `pages:manage` остаётся effective override для всех builder capabilities.

| Capability | Required permission | Notes |
|---|---|---|
| `preview` | `pages:read` | Read-only preview generation path. |
| `tree` | `pages:read` | Read-only node tree inspection path. |
| `properties` | `pages:update` | Editor-side property update path. |
| `publish` | `pages:publish` | Publish path; still requires `PortContext` write semantics (`idempotency_key` + deadline). |

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
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-transport-bridge.mjs` — no-compile guardrail для canonical GraphQL/server-function transport bridge markers.
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-flutter-handoff.mjs` — no-compile guardrail для Flutter Wave hand-off evidence contract и mobile app-core typed error parity markers.

## Связанные документы

- `docs/modules/tiptap-page-builder-implementation-plan.md` — платформенный rollout-план builder-first FBA;
- `docs/modules/manifest.md` — контракт `modules.toml` / `rustok-module.toml`;
- `crates/rustok-pages/docs/implementation-plan.md` — consumer-интеграция `pages` с reference builder-модулем.
