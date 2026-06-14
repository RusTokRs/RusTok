# План реализации `rustok-page-builder` (FBA reference module)

## Контекст

`rustok-page-builder` создаётся как самостоятельный FBA reference-module.
Первый этап — стабилизировать capability contracts и runtime seams,
после чего модуль подключается как consumer-dependency в `rustok-pages`.

## Этапы

- [x] Фаза 0 — bootstrap module contract (`Cargo.toml`, `rustok-module.toml`, `RusToKModule`).
- [ ] Фаза 1 — capability API baseline (`preview/tree/properties/publish`) без vendor lock-in.
- [~] Фаза 2 — observability и module health contract.
- [ ] Фаза 3 — integration contract для `pages` как consumer.
- [ ] Фаза 4 — rollout controls (feature flags / tenant gates / pilot).

## Текущее состояние

- runtime module scaffold завершён;
- module manifest и docs contracts заведены;
- machine-readable FBA registry (`contracts/page-builder-fba-registry.json`) фиксирует provider version, `consumer_min_version`, consumer contract versions, fallback profile set, provider health states, degradation reasons и pilot SLO thresholds для anti-drift gate;
- server feature wiring (`mod-page-builder`) подключён;
- capability handlers пока в статусе planned (Phase 1).


## FFA/FBA status

- FFA status: `not_started` (у reference provider пока нет module-owned UI)
- FBA status: `in_progress`
- Structural shape: `no_ui_boundary`
- Evidence:
  - модуль существует как самостоятельный reference provider для `preview/tree/properties/publish`;
  - machine-readable registry фиксирует provider/consumer versions, fallback profiles, health states, degradation reasons и SLO thresholds;
  - baseline verification gates покрывают provider/consumer anti-drift, Wave evidence template и synthetic Wave 0 packet;
  - первый migration slice перевёл `PageBuilderCapabilityService` на явный `PortContext` и enforce write semantics для `publish` без изменения DTO contract.
  - server-side handler seam добавил permission map `preview/tree -> pages:read`, `properties -> pages:update`, `publish -> pages:publish` с `pages:manage` override и registry/manifest anti-drift проверкой.
- Last verified at (UTC): 2026-06-14T00:00:00Z
- Owner: `rustok-page-builder` module team

## Ближайшие шаги

1. Довести transport-neutral DTO/contract package и `PortContext`-based service port для builder capabilities до publish-ready evidence.
2. Подключить server-side handler seam к реальным transport adapters после выбора GraphQL/server-function entrypoints.
3. Удерживать `verify-page-builder-contract-registry.mjs`, `verify-page-builder-wave-evidence-packet.mjs` и aggregate `verify-page-builder-fba-baseline.mjs` в baseline gate для provider/consumer anti-drift, health/SLO threshold sync, permission-map sync и Wave evidence формы.
4. Описать sunset path для legacy block-driven compatibility.

## Область работ

- runtime capability contract (`preview/tree/properties/publish`);
- permission/RBAC enforcement для builder lifecycle действий;
- observability и health контракты для control-plane rollout;
- consumer-integration protocol для `rustok-pages` и других модулей.

## Проверка

- `cargo xtask module validate page_builder`
- `cargo test -p rustok-page-builder --lib`

## Правила обновления

- при изменении capability contracts обновлять одновременно `docs/README.md` и этот план;
- при изменении rollout/ownership синхронизировать `docs/modules/tiptap-page-builder-implementation-plan.md`;
- не фиксировать исторический changelog: поддерживать только актуальное состояние этапов и ближайших работ.

## Связанные документы

- `docs/modules/tiptap-page-builder-implementation-plan.md`
- `docs/modules/manifest.md`
- `crates/rustok-page-builder/docs/README.md`
- `crates/rustok-pages/docs/implementation-plan.md`
