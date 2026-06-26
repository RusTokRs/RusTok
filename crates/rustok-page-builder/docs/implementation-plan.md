# План реализации `rustok-page-builder` (FBA reference module)

## Контекст

`rustok-page-builder` создаётся как самостоятельный FBA reference-module.
Первый этап — стабилизировать capability contracts и runtime seams,
после чего модуль подключается как consumer-dependency в `rustok-pages`.

## Этапы

- [x] Фаза 0 — bootstrap module contract (`Cargo.toml`, `rustok-module.toml`, `RusToKModule`).
- [x] Фаза 1 — capability API baseline (`preview/tree/properties/publish`) без vendor lock-in.
- [x] Фаза 2 — observability и module health contract baseline.
- [ ] Фаза 3 — integration contract для `pages` как consumer.
- [ ] Фаза 4 — rollout controls (feature flags / tenant gates / pilot).

## Текущее состояние

- runtime module scaffold завершён;
- module manifest и docs contracts заведены;
- machine-readable FBA registry (`contracts/page-builder-fba-registry.json`) фиксирует provider version, `consumer_min_version`, consumer contract versions, fallback profile set, provider health states, degradation reasons и pilot SLO thresholds для anti-drift gate;
- server feature wiring (`mod-page-builder`) подключён;
- typed provider health/SLO evaluator добавлен в runtime baseline для Wave evidence;
- transport-neutral DTO metadata (`PageBuilderContractMetadata::BASELINE`), typed provider error catalog (`PageBuilderErrorKind`, `PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE`) и typed Wave health evidence (`ProviderHealthEvidence`) заведены как publish-ready contract markers;
- transport-neutral tagged request/response envelope и `AuthorizedPageBuilderHandlers::handle` добавлены как entrypoint seam для будущих GraphQL/server-function adapters;
- transport bridge slice добавил `src/transport.rs` с `dispatch_graphql_envelope` / `dispatch_leptos_server_function_envelope` и canonical success/error envelope поверх `AuthorizedPageBuilderHandlers::handle`;
- endpoint adapter seam добавил `src/adapters.rs` с GraphQL/Leptos payload wrappers и host-facing handler-функциями поверх canonical dispatch helpers;
- machine-readable correlation contract `contracts/page-builder-correlation-contract.json` фиксирует evidence chain `builder write -> pages publish -> storefront read` и source markers для no-compile gate;
- capability handlers имеют reference-provider baseline (`ReferencePageBuilderService`) для `preview/tree/properties/publish` с contract validation, sanitize guard и deterministic typed responses;
- persistence/rendering extension slice заведён через `PageBuilderProjectStore`, `PageBuilderRenderingAdapter`, `ReferencePageBuilderRenderingAdapter` и `AdapterBackedPageBuilderService`, поэтому host adapters могут подключать storage/rendering без изменения DTO, `PageBuilderCapabilityService`, `AuthorizedPageBuilderHandlers::handle` или GraphQL/Leptos endpoint wrappers;
- Control-plane dry run evidence закреплён в `contracts/page-builder-control-plane-dry-run.json`: атомарный change-set для `builder.enabled` и дочерних flags, обязательные профили `all_on/publish_off/preview_off/builder_off`, before/after snapshots, waiver policy и read-surface guarantees.


## FFA/FBA status

- FFA status: `not_started` (у reference provider пока нет module-owned UI)
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence:
  - модуль существует как самостоятельный reference provider для `preview/tree/properties/publish`;
  - machine-readable registry фиксирует provider/consumer versions, fallback profiles, health states, degradation reasons и SLO thresholds;
  - baseline verification gates покрывают provider/consumer anti-drift, Wave evidence template, synthetic Wave 0 packet, Wave 1 readiness draft и correlation evidence `builder write -> pages publish -> storefront read`;
  - runtime health contract фиксирует `ready/degraded/unavailable`, degradation reasons, pilot SLO thresholds и typed SLO evaluation evidence в коде;
  - migration slice перевёл `PageBuilderCapabilityService` на явный `PortContext` и shared `PortCallPolicy::write()` для `publish` без изменения DTO contract.
  - server-side handler seam добавил permission map `preview/tree -> pages:read`, `properties -> pages:update`, `publish -> pages:publish` с `pages:manage` override и registry/manifest anti-drift проверкой.
  - provider runtime теперь exposes typed error catalog `validation/sanitize/runtime/feature-disabled` и стабильный degraded-mode code `FEATURE_DISABLED` для transport adapters.
  - transport bridge slice фиксирует canonical dispatch helpers для GraphQL и Leptos server-function adapters; no-compile guardrail `verify-page-builder-transport-bridge.mjs` проверяет, что adapters не обходят `AuthorizedPageBuilderHandlers::handle` и typed error mapping.
  - endpoint adapter seam фиксирует framework-neutral GraphQL/Leptos endpoint payloads и `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint`; no-compile guardrail `verify-page-builder-endpoint-adapters.mjs` удерживает endpoint wrappers на canonical request/response envelopes.
  - capability API baseline закрыт reference provider-ом без persistence side effects: `preview` рендерит deterministic wrapper, `properties` возвращает canonical node properties, `publish` возвращает typed publish result после `grapesjs_v1` validation, а forbidden preview HTML маппится в typed `sanitize` error.
  - Control-plane dry run evidence contract и runtime `BuilderControlPlaneChangeSet::dry_run` фиксируют атомарный toggle change-set, обязательные profile snapshots, rollback decision marker и waiver policy; aggregate no-compile baseline включает `verify-page-builder-control-plane-dry-run.mjs`.
  - adapter seam contract `contracts/page-builder-adapter-seams.json` и runtime traits `PageBuilderProjectStore` / `PageBuilderRenderingAdapter` фиксируют extension-point для persistence/rendering без transport-local capability aliases, transport-local error kind aliases, pages-local visual builder ownership или vendor-specific required project payloads.
- Last verified at (UTC): 2026-06-21T00:00:00Z
- Owner: `rustok-page-builder` module team

## Ближайшие шаги

1. Подключить host GraphQL resolver-ы и Leptos `#[server]` wrappers к `handle_page_builder_graphql_endpoint` / `handle_page_builder_leptos_server_function_endpoint`, сохраняя `PageBuilderCapabilityRequest/Response`, `PageBuilderServiceError::kind()` и `stable_code()` как canonical transport bridge без transport-local capability/error aliases.
2. Заменить draft dry-run snapshots фактическим tenant evidence packet без waivers перед Wave 1 promotion.
3. Удерживать `verify-page-builder-transport-bridge.mjs`, `verify-page-builder-endpoint-adapters.mjs`, `verify-page-builder-control-plane-dry-run.mjs`, `verify-page-builder-contract-registry.mjs`, `verify-page-builder-wave-evidence-packet.mjs`, `verify-page-builder-wave1-readiness-draft.mjs`, `verify-page-builder-correlation-evidence.mjs`, `verify-page-builder-adapter-seams.mjs` и aggregate `verify-page-builder-fba-baseline.mjs` в baseline gate для provider/consumer anti-drift, health/SLO threshold sync, permission-map sync, Wave evidence формы и correlation chain `builder write -> pages publish -> storefront read`.
4. Подключить конкретный host persistence/rendering adapter к `AdapterBackedPageBuilderService` в server/consumer wiring, сохраняя `CapabilityGuardedService` для rollout flags и `PortCallPolicy::write()` enforcement.
5. Описать sunset path для legacy block-driven compatibility.

## Область работ

- runtime capability contract (`preview/tree/properties/publish`);
- permission/RBAC enforcement для builder lifecycle действий;
- observability и health контракты для control-plane rollout;
- consumer-integration protocol для `rustok-pages` и других модулей.

## Проверка

- `cargo xtask module validate page_builder`
- `cargo test -p rustok-page-builder --lib`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs pages` (no-compile baseline gate for contract/evidence/fallback source markers; does not replace Cargo checks when compilations are allowed)

## Правила обновления

- при изменении capability contracts обновлять одновременно `docs/README.md` и этот план;
- при изменении rollout/ownership синхронизировать `docs/modules/tiptap-page-builder-implementation-plan.md`;
- не фиксировать исторический changelog: поддерживать только актуальное состояние этапов и ближайших работ.

## Связанные документы

- `docs/modules/tiptap-page-builder-implementation-plan.md`
- `docs/modules/manifest.md`
- `crates/rustok-page-builder/docs/README.md`
- `crates/rustok-pages/docs/implementation-plan.md`
