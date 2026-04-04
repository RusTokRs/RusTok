# RusTok: план закрытия module-system

> **Дата**: 2026-03-19  
> **Актуализировано**: 2026-04-03  
> **Назначение**: зафиксировать текущее состояние модульной платформы, разделение `Registry V1` и `Registry V2`, а также остаток работ до production-ready контура.

Легенда:
- `✅` реализовано
- `⚠️` частично реализовано
- `⬜` не начато

## Короткий вывод

- `Registry V1` уже существует как постоянный read-only catalog API.
- `Registry V2` уже существует как первый рабочий write/governance контур, но пока без полноценного async validation pipeline и богатой модели ownership/moderation.
- `DeploymentProfile` и `RuntimeHostMode` уже разделены в коде и должны оставаться независимыми осями.
- Manifest-wired Leptos UI уже работает для набора канонических dual-surface модулей; основной остаток здесь в покрытии и дисциплине validation/tooling.

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
- ✅ Базовая local validation для marketplace metadata уже включена.

### 2. Build/codegen и host wiring

- ✅ `apps/server/build.rs` генерирует optional module registry, schema fragments и routes из `modules.toml`.
- ✅ `apps/admin/build.rs` уже строит generic module root pages/nav/dashboard и nested route metadata из `[[provides.admin_ui.pages]]`.
- ✅ `apps/storefront/build.rs` поддерживает multi-slot storefront sections и generic route `/modules/{route_segment}`.
- ✅ Build/release pipeline исполняет manifest-derived сборку `server`, `admin` и Leptos `storefront`.
- ✅ `ManifestManager` валидирует semver-диапазоны зависимостей, runtime/product conflicts и schema-driven settings contract.

### 3. Admin `/modules`

- ✅ `updateModuleSettings` закрыт end-to-end.
- ✅ `/modules` умеет:
  - tenant-level toggle
  - platform install/uninstall
  - schema-driven settings editing
  - build progress через GraphQL WS с polling fallback
  - registry readiness summary
  - visual marketplace preview
  - release trail summary
  - publish lifecycle block для `Registry V2`
- ✅ Последние operator-facing строки в module UI выровнены под locale-aware rendering, чтобы модульный UX не разваливался на смешение русского и английского.

  - policy-aware moderation summary
  - actionable next-step hints
  - copyable operator commands РґР»СЏ `xtask`
  - state-aware live API actions СЃ authority / body / header hints
  - Windows-friendly `curl.exe` snippets
  - live-mode `xtask` snippets РґР»СЏ publish / owner-transfer / yank

### 4. Manifest-wired UI coverage

Текущее состояние path-модулей:

| Модуль | Статус UI wiring | Комментарий |
|---|---|---|
| `blog` | ✅ dual-surface | admin + storefront |
| `commerce` | ✅ dual-surface | admin + storefront |
| `forum` | ✅ dual-surface | admin + storefront |
| `pages` | ✅ dual-surface | admin + storefront |
| `search` | ✅ dual-surface | admin + storefront |
| `workflow` | ✅ admin-only | осознанный admin-only slice |
| `channel` | ✅ admin-only | осознанный admin-only slice |

Дополнительно:

- ✅ Наличие `admin/Cargo.toml` или `storefront/Cargo.toml` без соответствующего `[provides.*_ui].leptos_crate` теперь считается wiring error.
- ✅ `cargo xtask module validate`, `apps/admin/build.rs` и `apps/storefront/build.rs` используют manifest wiring как источник правды, а не наличие sub-crate на диске.

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
- ✅ `POST /v2/catalog/yank`

Текущее lifecycle-состояние:

- ✅ request создаётся отдельно от artifact upload
- ✅ upload теперь только сохраняет artifact и ставит request в `submitted`
- ✅ validation вынесена в отдельный lifecycle-step `/validate`
- ✅ `/validate` валидирует bundle against request
- ✅ `/validate` теперь работает как queue boundary и переводит request в background `validating`, а не держит bundle-check inline в HTTP вызове
- ✅ request проходит через `artifact_uploaded -> submitted -> validating -> approved` или `rejected`
- ✅ публикация release происходит отдельным governance action
- ✅ `yank` требует обязательную причину
- ✅ published releases уже проецируются обратно в `V1`

Governance first cut:

- ✅ header-driven actor contract уже работает
- ✅ persisted slug owner binding уже сохраняется отдельно от governance actor; release publisher identity и `/modules` lifecycle UX теперь читают этот binding, а `x-rustok-publisher` отделён от audit actor `x-rustok-actor`
- ✅ requested publisher identity теперь также сохраняется прямо в `registry_publish_requests`, и approve/reject policy больше не опирается только на `requested_by`
- ✅ persisted audit trail теперь сохраняется в `registry_governance_events` и уже отражается в `/modules` как recent governance events для publish/upload/validate/approve/reject/yank/owner-binding переходов
- ✅ есть first-cut policy для governance actors и slug-scoped publishers
- ✅ ownership transfer теперь вынесен в явный `POST /v2/catalog/owner-transfer` с обязательной причиной, persisted owner rebind и отдельным audit event `owner_transferred`
- ✅ approve/reject больше не считаются self-review шагом через `publisher_identity`: review path теперь требует governance actor или текущего persisted owner
- ✅ `xtask module publish` уже умеет live orchestration и по умолчанию останавливается на `approved` / `review-ready`; финальный governance `approve` теперь делается только по явному `--auto-approve`
- ✅ `xtask module owner-transfer` теперь уже умеет live V2 endpoint
- ✅ `xtask module yank` уже умеет live V2 endpoint
- ✅ dry-run режим сохранён

## Что остаётся сделать

### Блок A. Registry V2 async validation и governance

- ⚠️ Validation pipeline уже отделён от upload path и вынесен за пределы inline HTTP upload/validate round-trip, но compile/test/security/policy checks всё ещё остаются в одном лёгком background validator без отдельного job orchestration слоя.
- ✅ Текущий background validator теперь явно маркирует свой scope: `approved` означает, что artifact/manifest contract checks пройдены и запрос готов к review, а compile/test/security/policy остаются внешними follow-up gates и surfaced в warnings/audit trail.
- ✅ `POST /v2/catalog/publish/{request_id}/validate` теперь умеет requeue request после автоматического `validation_failed`, но не resurrect-ит manual governance reject-path.
- ✅ Для транзиентных сбоев загрузки artifact внутри background validator уже есть минимальный retry/backoff path с audit events `validation_retry_scheduled` / `validation_retry_exhausted`.
- ⬜ Нужно вынести compile/test/security/policy checks в асинхронный контур.
- ⬜ Нужен явный request lifecycle для background validation jobs и повторной обработки.
- ⚠️ Базовая persistence-модель для ownership и audit trail уже есть (`registry_module_owners`, `registry_governance_events`), и явный owner transfer уже поднят, но richer moderation decisions всё ещё не доведены до полного production policy.
- ⬜ Нужен более строгий policy layer для:
  - moderator/admin approve-reject capabilities
  - unpublish/yank governance rules

### Блок B. Отдельный deployment для `modules.rustok.dev`

- ⚠️ `registry_only` runtime уже есть, но отдельный внешний deployment ещё не оформлен до production-ready состояния.
- ⚠️ Базовый rollout/runbook для dedicated catalog host уже зафиксирован в `apps/server/docs/health.md`: canonical build profile = `headless-api`, runtime host mode = `registry_only`, acceptance = V1 list/detail + cache headers + reduced OpenAPI + negative write-path checks.
- ⚠️ Локальный acceptance-smoke для `registry_only` уже есть; незакрытым остаётся именно внешний deployment/runbook путь для `modules.rustok.dev`.

### Блок C. Покрытие UI и operator polish

- ⚠️ Канонические dual-surface proof points уже есть, но покрытие не всех модулей доведено до финального состояния.
- ⬜ Нужно продолжить аудит path-модулей на предмет честной классификации `dual-surface` / `admin-only` / `storefront-only` / `no-ui`.
- ⚠️ `/modules` уже умеет не только показывать lifecycle, но и запускать интерактивные governance-действия, показывать policy hints, copyable `xtask`/HTTP/curl snippets, headers/body hints и operator commands.
- ✅ `/modules` уже различает automatic validation failure и manual governance reject в `Validation summary`, а также показывает отдельный `Ready for review` сигнал для validated request, который ещё не опубликован.
- ✅ `/modules` теперь также показывает отдельный `Follow-up gates` summary для `compile_smoke` / `targeted_tests` / `security_policy_review`, чтобы внешние async/manual gates были видны отдельно от базовой artifact/manifest validation.
- ⬜ В operator UX осталось добить:
  - более богатый вывод async validation feedback и moderation decisions вне текущего summary/callout уровня
  - финальный polish вокруг owner-transfer/review authority

### Блок D. Тесты

- ⚠️ Точечные `cargo check` по релевантным пакетам уже проходят для большинства последних шагов.
- ⚠️ Полный workspace/test graph регулярно блокируется незавершённой параллельной разработкой в соседних crate-ах.
- ⬜ Нужны более устойчивые targeted tests для:
  - V2 lifecycle transitions
  - projection V2 → V1
  - `registry_only` reduced surface
  - manifest-wired UI guardrails

## Приоритет выполнения

1. Вынести compile/test/security/policy checks в отдельный асинхронный orchestration-контур поверх уже существующего `validate`.
2. Довести ownership/governance persistence и policy model до richer moderation capabilities.
3. Закрыть production deployment path для `modules.rustok.dev`.
4. Доработать moderation UX в `/modules` вокруг richer validation feedback и moderation decisions.
5. Продолжить аудит и доводку manifest-wired UI coverage.
6. Уплотнить targeted test coverage вокруг V1/V2 и reduced host.

## Критерии завершения

План можно считать закрытым, когда одновременно выполняются условия:

- `V1` стабилен как постоянный read-only catalog API.
- `V2` имеет полноценный async publish/governance lifecycle.
- `registry_only` развёртывается как отдельный catalog host без monolith surface.
- publish/yank flow работает и в `Monolith + full`, и в `HeadlessApi + full`.
- `/modules` показывает operator-friendly lifecycle, validation и governance state, а также умеет запускать базовые V2 governance-действия.
- UI wiring всех path-модулей честно классифицирован и проверяется tooling-guardrails.

## Связанные документы

- [Контракт manifest-файла](./manifest.md)
- [Реестр модулей и владельцев](./registry.md)
- [Обзор модульной платформы](./overview.md)
- [Архитектура модулей](../architecture/modules.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Server docs](../../apps/server/docs/README.md)
- [Admin docs](../../apps/admin/docs/README.md)

## Актуализация `/modules` на 2026-04-03

- `lifecycle-aware` operator UX в `/modules` уже заметно продвинут дальше, чем описано в базовом плане выше.
- Detail panel уже показывает:
  - policy-aware moderation summary
  - actionable next-step hints
  - copyable operator commands для `xtask`
  - state-aware live API actions
  - authority hints по allowed actor/current owner
  - `read-only` vs `write-path` маркировку
  - минимальные request-body hints для mutating actions
  - header hints для `x-rustok-actor` / `x-rustok-publisher`
  - Windows-friendly `curl.exe` snippets
  - live-режим `xtask` snippets там, где уже есть CLI-эквивалент
- Таким образом, в operator UX остался уже не общий lifecycle/governance слой, а хвост по richer validation feedback и общему moderation polish.
## Актуализация `/modules` на 2026-04-04

- Operator UX в `apps/admin` продвинут дальше lifecycle summary: `/modules` detail panel теперь умеет запускать интерактивные V2 governance-действия поверх уже существующих registry endpoints.
- Для `validate`, `approve`, `reject`, `owner-transfer` и `yank` добавлены локальные формы/кнопки с `dry-run` toggle, полями `actor` / `publisher` / `reason` / `new owner actor`, принудительным обновлением detail state и отображением `warnings` / `errors` из `RegistryMutationResponse`.
- Admin transport для этих действий идёт через узкие `#[server]` wrappers в `features/modules/api.rs`, которые ходят в существующий `/v2/catalog/*` HTTP contract и не вводят отдельный параллельный API.
- Дополнительно detail panel уже показывает copyable operator commands, `xtask` live-mode snippets, Windows-friendly `curl.exe` snippets, authority hints и минимальные body/header шаблоны для live governance-вызовов.
- Дополнительно live destructive actions (`reject`, `owner-transfer`, `yank`) теперь идут через явный confirm-step в detail panel, так что оператор не отправляет такие governance-вызовы одним кликом.
- Таким образом, в `/modules` уже закрыт базовый interactive governance слой и confirm-path для destructive actions; в хвосте остались в основном richer validation feedback и moderation polish.

