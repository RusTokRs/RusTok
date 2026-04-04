# RusTok: план закрытия module-system

> **Дата**: 2026-03-19  
> **Актуализировано**: 2026-04-04  
> **Назначение**: зафиксировать текущее состояние модульной платформы, разделение `Registry V1` и `Registry V2`, а также остаток работ до production-ready контура.

Легенда:
- `✅` реализовано
- `⚠️` частично реализовано
- `⬜` не начато

## Короткий вывод

- `Registry V1` уже существует как постоянный read-only catalog API.
- `Registry V2` уже существует как первый рабочий write/governance контур, но пока без полноценного async validation pipeline и богатой модели ownership/moderation.
- `DeploymentProfile` и `RuntimeHostMode` уже разделены в коде и должны оставаться независимыми осями.
- Manifest-wired Leptos UI уже работает не только для канонических dual-surface модулей, но и для расширенного набора admin-only core/optional surfaces; основной остаток здесь в полном аудите `no-ui`/`admin-only` coverage и дисциплине validation/tooling.

## Архитектурные инварианты

### V1 и V2

- `Registry V1` — это версия read-only API каталога, а не временный этап.
- `Registry V2` — это версия write/governance API, которая не заменяет `V1`, а публикует в него release-проекцию.
- Внешние discovery consumers и operator UI читают каталог через `GET /v1/catalog*`.
- Write-path, publish lifecycle и governance идут через `V2`.

### Deployment profile и runtime host mode

- `DeploymentProfile` описывает build/deploy surface:
  - `monolith`
  - `server-with-admin`
  - `server-with-storefront`
  - `headless-api`
- `RuntimeHostMode` описывает runtime-exposed surface:
  - `full`
  - `registry_only`
- Эти оси независимы.
- `registry_only` остаётся строго read-only режимом поверх `HeadlessApi`.
- `V2` должен быть API-first и одинаково доступным в `Monolith + full` и `HeadlessApi + full`.

## Что уже закрыто

### 1. Manifest contract и module metadata

- ✅ `rustok-module.toml` является каноническим manifest-контрактом для path-модулей.
- ✅ Парсятся и валидируются:
  - `[module]`
  - `[compatibility]`
  - `[dependencies]`
  - `[conflicts]`
  - `[crate]`
  - `[provides]`
  - `[settings]`
  - `[locales]`
  - `[marketplace]`
- ✅ `[settings]` уже поддерживает schema-driven scalar controls, `options`, `min`/`max`, nested `properties`/`items`, inline editor для nested `object`/`array` и schema-aware child editors.
- ✅ `[marketplace]` уже протянут через manifest → catalog → GraphQL → Admin UI:
  - `category`
  - `tags`
  - `icon`
  - `banner`
  - `screenshots`
- ✅ Базовая local validation для marketplace metadata уже включена:
  - `description >= 20`
  - `icon` = absolute `http(s)` `.svg`
  - `banner` / `screenshots` = absolute `http(s)` image URLs
- ✅ `[provides.*_ui.i18n]` уже поддерживает manifest-level bundle contract для package-owned переводов:
  - `default_locale`
  - `supported_locales`
  - явные пути к `locales/*.json` / `messages/*.json`
- ✅ `ManifestManager` валидирует locale tags, membership для `default_locale` и наличие каждого объявленного bundle file.

### 2. Build/codegen и host wiring

- ✅ `apps/server/build.rs` генерирует optional module registry, schema fragments и routes из `modules.toml`.
- ✅ `apps/admin/build.rs` уже строит generic module root pages/nav/dashboard и nested route metadata из `[[provides.admin_ui.pages]]`.
- ✅ `apps/storefront/build.rs` поддерживает multi-slot storefront sections и generic route `/modules/{route_segment}`.
- ✅ Build/release pipeline исполняет manifest-derived сборку `server`, `admin` и Leptos `storefront`.
- ✅ `ManifestManager` валидирует semver-диапазоны зависимостей, runtime/product conflicts и schema-driven settings contract.
- ✅ Host build/codegen и `cargo xtask module validate` используют manifest wiring как источник правды, а не просто наличие sub-crate на диске.

### 3. Admin `/modules`

- ✅ `updateModuleSettings` закрыт end-to-end.
- ✅ `/modules` умеет:
  - tenant-level toggle
  - platform install/uninstall/upgrade
  - schema-driven settings editing
  - build progress через GraphQL WS с polling fallback
  - registry readiness summary и visual marketplace preview
  - release trail summary
  - publish lifecycle block для `Registry V2`
  - `Validation summary` с разделением automatic `validation_failed` и manual governance `reject`
  - `Follow-up gates` summary для `compile_smoke` / `targeted_tests` / `security_policy_review`
  - recent governance audit trail
  - interactive V2 governance actions `validate` / `approve` / `reject` / `owner-transfer` / `yank`
- ✅ Последние operator-facing строки в module UI выровнены под locale-aware rendering, чтобы модульный UX не разваливался на смешение русского и английского.
- ✅ Detail panel уже показывает:
  - policy-aware moderation summary
  - actionable next-step hints
  - copyable operator commands для `xtask`
  - live-mode `xtask` snippets для publish / owner-transfer / yank
  - state-aware live API actions с authority / body / header hints
  - Windows-friendly `curl.exe` snippets
  - `read-only` vs `write-path` маркировку
  - persisted owner binding и recent governance events
- ✅ Live destructive actions (`reject`, `owner-transfer`, `yank`) идут через явный confirm-step, а `dry-run` остаётся быстрым preview path.

### 4. Manifest-wired UI coverage

Текущее покрытие manifest-wired Leptos UI среди path-модулей:

| Класс | Модули | Комментарий |
|---|---|---|
| `dual-surface` | `blog`, `commerce`, `forum`, `pages`, `search` | admin + storefront |
| `admin-only core` | `channel`, `index`, `outbox`, `rbac`, `tenant` | осознанные module-owned admin slices |
| `admin-only optional` | `comments`, `media`, `workflow` | осознанные module-owned admin slices |

Дополнительно:

- ✅ Наличие `admin/Cargo.toml` или `storefront/Cargo.toml` без соответствующего `[provides.*_ui].leptos_crate` теперь считается wiring error.
- ✅ Package-owned i18n contract уже живёт в manifest-driven UI:
  - admin surfaces `workflow`, `rbac`, `tenant`, `index`, `outbox`, `pages`, `comments`, `channel`, `forum`, `search`, `commerce`
  - storefront surfaces `blog`, `pages`, `commerce`, `forum`, `search`
- ⚠️ Для остальных path entries в `modules.toml` финальная явная классификация как `no-ui` / `capability-only` / `future UI` всё ещё должна быть зафиксирована отдельным аудитом, а не выводиться по умолчанию из отсутствия sub-crate.

### 5. Registry V1

`Registry V1` уже реализован как read-model:

- ✅ `GET /v1/catalog`
- ✅ `GET /v1/catalog/{slug}`
- ✅ legacy aliases `GET /catalog` и `GET /catalog/{slug}` сохранены как backward-compatible fallback

Поддерживается:

- ✅ schema-versioned payload
- ✅ filters: `search`, `category`, `tag`
- ✅ paging: `limit`, `offset`
- ✅ `X-Total-Count`
- ✅ stable ordering для paging
- ✅ `ETag`
- ✅ `Cache-Control`
- ✅ `If-None-Match`
- ✅ `304 Not Modified`
- ✅ OpenAPI coverage
- ✅ metadata policy для visual fields и `description`

Каталожные данные уже включают:

- ✅ `name`
- ✅ `description`
- ✅ `category`
- ✅ `tags`
- ✅ `icon`
- ✅ `banner`
- ✅ `screenshots`
- ✅ release trail normalization
- ✅ checksum/publisher normalization

Consumer path:

- ✅ `RegistryMarketplaceProvider` умеет list/detail-aware lookup
- ✅ GraphQL `marketplace` и `marketplaceModule` используют V1-aware provider path
- ✅ `/modules` уже использует `tag` filter и registry metadata

### 6. `registry_only` host

- ✅ `apps/server` уже умеет работать в `settings.rustok.runtime.host_mode = "registry_only"`.
- ✅ В этом режиме остаются только:
  - `health`
  - `metrics`
  - `swagger`
  - V1 catalog routes
- ✅ GraphQL, auth, MCP и embedded UI surfaces не поднимаются.
- ✅ `OpenAPI` в этом режиме режется до реального reduced surface.
- ✅ `/health/ready`, `/health/runtime` и runtime metrics выровнены под reduced host.
- ✅ `RUSTOK_RUNTIME_HOST_MODE=registry_only` поддерживается через env override.
- ✅ Локальный smoke path уже зафиксирован в `apps/server/docs/health.md`, `scripts/verify/verify-deployment-profiles.sh` и PowerShell-варианте `scripts/verify/verify-deployment-profiles.ps1`.
- ✅ Локальный acceptance-smoke для `registry_only` уже включает reduced-surface negative write checks, `GET /v1/catalog/{slug}` detail contract и cache-path через `ETag` / `If-None-Match`.

### 7. Registry V2

Первый рабочий V2 lifecycle уже есть:

- ✅ `POST /v2/catalog/publish`
- ✅ `GET /v2/catalog/publish/{request_id}`
- ✅ `PUT /v2/catalog/publish/{request_id}/artifact`
- ✅ `POST /v2/catalog/publish/{request_id}/validate`
- ✅ `POST /v2/catalog/publish/{request_id}/approve`
- ✅ `POST /v2/catalog/publish/{request_id}/reject`
- ✅ `POST /v2/catalog/owner-transfer`
- ✅ `POST /v2/catalog/yank`

Текущее lifecycle-состояние:

- ✅ `POST /v2/catalog/publish` делает sync metadata validation и создаёт request отдельно от artifact upload
- ✅ upload теперь только сохраняет artifact и ставит request в `submitted`
- ✅ validation вынесена в отдельный lifecycle-step `/validate`
- ✅ `/validate` валидирует bundle against request
- ✅ `/validate` теперь работает как queue boundary и переводит request в background `validating`, а не держит bundle-check inline в HTTP вызове
- ✅ У validation теперь есть отдельный persisted job-layer `registry_validation_jobs` с попытками и статусами `queued` / `running` / `succeeded` / `failed`.
- ✅ request проходит через `artifact_uploaded -> submitted -> validating -> approved` или `rejected`
- ✅ `approved` теперь означает `review-ready` после automated artifact/manifest validation, а не уже опубликованный release
- ✅ публикация release происходит отдельным governance action
- ✅ `reject`, `owner-transfer` и `yank` требуют обязательную governance reason
- ✅ published releases уже проецируются обратно в `V1`

Governance first cut:

- ✅ header-driven actor contract уже работает
- ✅ persisted slug owner binding уже сохраняется отдельно от governance actor; release publisher identity и `/modules` lifecycle UX теперь читают этот binding, а `x-rustok-publisher` отделён от audit actor `x-rustok-actor`
- ✅ requested publisher identity теперь также сохраняется прямо в `registry_publish_requests`, и approve/reject policy больше не опирается только на `requested_by`
- ✅ persisted audit trail теперь сохраняется в `registry_governance_events` и уже отражается в `/modules` как recent governance events для publish/upload/validate/approve/reject/owner-transfer/yank/owner-binding переходов
- ✅ есть first-cut policy для governance actors и slug-scoped publishers
- ✅ ownership transfer теперь вынесен в явный `POST /v2/catalog/owner-transfer` с обязательной причиной, persisted owner rebind и отдельным audit event `owner_transferred`
- ✅ approve/reject больше не считаются self-review шагом через `publisher_identity`: review path теперь требует governance actor или текущего persisted owner
- ✅ lifecycle consumers уже получают derived `follow_up_gates` и structured `automated_checks` через GraphQL/Admin read-path
- ✅ `xtask module publish` уже умеет live orchestration и по умолчанию останавливается на `approved` / `review-ready`; финальный governance `approve` теперь делается только по явному `--auto-approve`
- ✅ `xtask module owner-transfer` теперь уже умеет live V2 endpoint
- ✅ `xtask module yank` уже умеет live V2 endpoint
- ✅ dry-run режим сохранён

## Что остаётся сделать

### Блок A. Registry V2 async validation и governance

- ✅ Validation pipeline уже отделён от upload path и вынесен за пределы inline HTTP upload/validate round-trip; базовый artifact/manifest validator и follow-up stage orchestration теперь разделены.
- ✅ Текущий background validator теперь явно маркирует свой scope: `approved` означает, что artifact/manifest contract checks пройдены и запрос готов к review, а compile/test/security/policy остаются внешними follow-up gates и surfaced в warnings/audit trail.
- ✅ Lifecycle snapshots и governance events уже несут structured `automated_checks` / `follow_up_gates`, так что текущее ограничение validator scope видно не только в docs, но и в live operator UX.
- ✅ `POST /v2/catalog/publish/{request_id}/validate` теперь умеет requeue request после автоматического `validation_failed`, но не resurrect-ит manual governance reject-path.
- ✅ Для транзиентных сбоев загрузки artifact внутри background validator уже есть минимальный retry/backoff path с audit events `validation_retry_scheduled` / `validation_retry_exhausted`.
- ✅ Явный persisted lifecycle для базовых validation jobs уже есть через `registry_validation_jobs` и audit events `validation_job_*`.
- ✅ Для compile/test/security/policy checks теперь есть отдельный persisted orchestration-layer `registry_validation_stages` со статусами `queued` / `running` / `passed` / `failed` / `blocked`, attempt number, detail и timestamps.
- ✅ `registryLifecycle` теперь отдаёт не только compatibility summary `followUpGates`, но и canonical `validationStages`, так что `/modules` показывает per-stage state отдельно от базовой validator summary.
- ✅ Для ручной фиксации stage results появился live write-path `POST /v2/catalog/publish/{request_id}/stages` и matching operator command `cargo xtask module stage <request-id> <stage> <status> ...`.
- ⚠️ Реальный local/remote runner для compile/test/security stages в этот шаг не встроен; orchestration и operator recording уже persisted, но исполнение команд всё ещё внешнее/manual.
- ⚠️ Базовая persistence-модель для ownership и audit trail уже есть (`registry_module_owners`, `registry_governance_events`), и явный owner transfer уже поднят, но richer moderation decisions всё ещё не доведены до полного production policy.
- ⬜ Нужен более строгий policy layer для:
  - moderator/admin approve-reject capabilities
  - unpublish/yank governance rules

### Блок B. Отдельный deployment для `modules.rustok.dev`

- ⚠️ `registry_only` runtime уже есть, но отдельный внешний deployment ещё не оформлен до production-ready состояния.
- ⚠️ Базовый rollout/runbook для dedicated catalog host уже зафиксирован в `apps/server/docs/health.md`: canonical build profile = `headless-api`, runtime host mode = `registry_only`, acceptance = V1 list/detail + cache headers + reduced OpenAPI + negative write-path checks.
- ⚠️ Локальный acceptance-smoke для `registry_only` уже есть; незакрытым остаётся именно внешний deployment/runbook путь для `modules.rustok.dev`.

### Блок C. Покрытие UI и operator polish

- ⚠️ Manifest-wired UI coverage уже заметно шире исходных dual-surface proof points: live contract включает расширенный набор admin-only core/optional surfaces, но полный аудит остальных path entries ещё не доведён до финальной классификации.
- ⬜ Нужно продолжить аудит path-модулей на предмет честной классификации `dual-surface` / `admin-only` / `storefront-only` / `no-ui`.
- ✅ `/modules` уже умеет не только показывать lifecycle, но и запускать интерактивные governance-действия, показывать policy hints, copyable `xtask`/HTTP/curl snippets, headers/body hints и operator commands.
- ✅ `/modules` уже различает automatic validation failure и manual governance reject в `Validation summary`, а также показывает отдельный `Ready for review` сигнал для validated request, который ещё не опубликован.
- ✅ `/modules` теперь также показывает отдельный `Follow-up gates` summary для `compile_smoke` / `targeted_tests` / `security_policy_review`, чтобы внешние async/manual gates были видны отдельно от базовой artifact/manifest validation.
- ⬜ В operator UX осталось добить:
  - более богатый per-check вывод async validation feedback и moderation decisions вне текущего summary/callout уровня
  - финальный polish вокруг owner-transfer/review authority и richer moderation history

### Блок D. Тесты

- ⚠️ Точечные `cargo check` по релевантным пакетам уже проходят для большинства последних шагов.
- ⚠️ Полный workspace/test graph регулярно блокируется незавершённой параллельной разработкой в соседних crate-ах.
- ⬜ Нужны более устойчивые targeted tests для:
  - V2 lifecycle transitions, включая requeue/retry semantics
  - projection V2 → V1
  - `registry_only` reduced surface
  - manifest-wired UI и `[provides.*_ui.i18n]` guardrails

## Приоритет выполнения

1. Вынести compile/test/security/policy checks в отдельный асинхронный orchestration-контур поверх уже существующего `validate`.
2. Довести ownership/governance persistence и policy model до richer moderation capabilities.
3. Закрыть production deployment path для `modules.rustok.dev`.
4. Доработать moderation UX в `/modules` вокруг richer validation feedback и moderation decisions.
5. Продолжить аудит и доводку manifest-wired UI / i18n coverage.
6. Уплотнить targeted test coverage вокруг V1/V2 и reduced host.

## Критерии завершения

План можно считать закрытым, когда одновременно выполняются условия:

- `V1` стабилен как постоянный read-only catalog API.
- `V2` имеет полноценный async publish/governance lifecycle.
- `registry_only` развёртывается как отдельный catalog host без monolith surface.
- publish/yank flow работает и в `Monolith + full`, и в `HeadlessApi + full`.
- `/modules` показывает operator-friendly lifecycle, validation и governance state, а также умеет запускать базовые V2 governance-действия.
- UI wiring всех path-модулей честно классифицирован, а manifest / i18n guardrails проверяются tooling-слоем.

## Связанные документы

- [Контракт manifest-файла](./manifest.md)
- [Реестр модулей и владельцев](./registry.md)
- [Обзор модульной платформы](./overview.md)
- [Архитектура модулей](../architecture/modules.md)
- [Архитектура i18n](../architecture/i18n.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Server docs](../../apps/server/docs/README.md)
- [Health / registry_only runbook](../../apps/server/docs/health.md)
- [Admin docs](../../apps/admin/docs/README.md)

