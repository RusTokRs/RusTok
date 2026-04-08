# RusTok: план закрытия module-system

> **Дата**: 2026-03-19  
> **Актуализировано**: 2026-04-07  
> **Назначение**: зафиксировать текущее состояние модульной платформы, разделение `Registry V1` и `Registry V2`, а также остаток работ до production-ready контура.

Легенда:
- `✅` реализовано
- `⚠️` частично реализовано
- `⬜` не начато

## Короткий вывод

- `Registry V1` уже существует как постоянный read-only catalog API.
- `Registry V2` уже существует как рабочий write/governance контур с persisted async validation jobs/stages, server-driven moderation policy и local-host runner; в остатке remote/sandboxed execution и более богатая модель moderation/release-management.
- `DeploymentProfile` и `RuntimeHostMode` уже разделены в коде и должны оставаться независимыми осями.
- Manifest-wired Leptos UI уже работает не только для канонических dual-surface модулей, но и для расширенного набора admin-only core/optional surfaces; live catalog/GraphQL/Admin contract теперь ещё и явно публикует `hasAdminUi` / `hasStorefrontUi` / `uiClassification`, так что surface-аудит больше не живёт только в prose-таблицах.
- Волна базовой module-contract унификации фактически закрыта: на `2026-04-07` `cargo xtask module validate` проходит по всем path-модулям из `modules.toml` и уже проверяет manifest/docs/UI/runtime contracts fail-fast.

## Волны унификации модулей

База для этой волны стандартизации вынесена в отдельное исследование: [единый стандарт модулей](../research/deep-research-modules.md).

### Волна 0. Encoding и policy

- В репозитории зафиксированы `.editorconfig` и `.gitattributes` для `UTF-8` и воспроизводимых line endings.
- Central docs и обязательные module docs приводятся к единому формату без отдельных локальных исключений.
- Windows verification path документируется как first-class локальный сценарий, а не как fallback после Bash.

### Волна 1. Contract completeness

- Каждый path-модуль из `modules.toml` обязан иметь `rustok-module.toml`.
- Каждый path-модуль обязан иметь `README.md`, `docs/README.md` и `docs/implementation-plan.md`, причём root `README.md` обязан содержать `## Interactions`.
- `docs/modules/_index.md` остаётся каноническим индексом ссылок на локальные module docs.
- No-UI modules фиксируют явную `ui_classification`, а UI-модули проходят wiring validation по фактическому наличию subcrate.
- `modules.toml.depends_on`, `[dependencies]` в `rustok-module.toml` и `RusToKModule::dependencies()` в runtime не должны расходиться.

### Волна 1.5. Исполнимый audit path

- `cargo xtask module validate` больше не имеет skip-path для path-модулей без `rustok-module.toml`.
- Validation включает docs minimum, root README contract, broken module-doc links, dependency drift, runtime dependency drift и manifest/UI wiring checks.
- Обязательный Windows-native path состоит из `cargo xtask module validate`, `cargo xtask module test <slug>`, Node verification scripts и PowerShell-wrapper для architecture guard.

### Волна 2. Полный scoped-аудит

- ✅ На `2026-04-07` все path-модули из `modules.toml` проходят `cargo xtask module validate`.
- ⚠️ Полный регулярный прогон `cargo xtask module test <slug>` по всему graph остаётся частично ручным и всё ещё упирается в параллельную разработку соседних crate-ов.
- ⚠️ Общие quality gates (`verify:i18n:*`, storefront route audit, architecture guard, deployment-profile smoke при необходимости) уже считаются post-remediation контуром, но ещё не сведены в один стабильный зелёный pipeline для всего workspace.

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
- ✅ `cargo xtask module validate` теперь fail-fast проверяет не только сам manifest, но и обязательные docs/README contracts, ссылки из `docs/modules/_index.md`, runtime dependency drift и manifest/UI wiring.
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
| `capability-only` | `alloy`, `auth`, `cache`, `cart`, `content`, `customer`, `email`, `fulfillment`, `inventory`, `order`, `payment`, `pricing`, `product`, `profiles`, `region`, `taxonomy` | path-модули без собственного UI, явно классифицированные через `ui_classification = "capability_only"` |

Дополнительно:

- ✅ Наличие `admin/Cargo.toml` или `storefront/Cargo.toml` без соответствующего `[provides.*_ui].leptos_crate` теперь считается wiring error.
- ✅ Package-owned i18n contract уже живёт в manifest-driven UI:
  - admin surfaces `workflow`, `rbac`, `tenant`, `index`, `outbox`, `pages`, `comments`, `channel`, `forum`, `search`, `commerce`
  - storefront surfaces `blog`, `pages`, `commerce`, `forum`, `search`
- ✅ Manifest-derived surface classification теперь публикуется машинно-читаемо через catalog/GraphQL/Admin: `hasAdminUi`, `hasStorefrontUi`, `uiClassification`, где UI-wired модули идут как `dual_surface | admin_only | storefront_only`, а no-UI пакеты могут явно фиксировать `no_ui | capability_only | future_ui`.
- ✅ Для текущих no-UI path entries явная business-классификация тоже уже зафиксирована в `rustok-module.toml`: `ui_classification = capability_only`, так что остаток здесь больше не в ручном аудите существующих модулей, а только в дисциплине для новых пакетов.

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
- ✅ `POST /v2/catalog/publish/{request_id}/stages`
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
- ✅ `reject`, `owner-transfer` и `yank` требуют обязательную governance reason, а live `reject`/`owner-transfer`/`yank` теперь ещё и structured `reason_code`
- ✅ финальный `approve` теперь тоже стал policy-aware: если follow-up validation stages ещё не `passed`, live approval требует explicit override `reason + reason_code`, а audit trail пишет отдельный `publish_approval_override`
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
- ✅ `xtask module publish --auto-approve` теперь умеет и explicit approval override через `--approve-reason` / `--approve-reason-code`, если follow-up stages ещё не закрыты
- ✅ `xtask module stage-run` теперь закрывает первый execution-path для follow-up stages: `compile_smoke` и `targeted_tests` можно прогнать локально и сразу записать их lifecycle в registry без ручного `running/passed/failed` шага, а `security_policy_review` теперь тоже идёт через operator-assisted local preflight + explicit `--confirm-manual-review`
- ✅ `xtask module publish --auto-approve` теперь тоже стал stage-aware orchestration path, а не только approve caller: он сам прогоняет pending local stages (`compile_smoke`, `targeted_tests`, опционально `security_policy_review` при `--confirm-manual-review`) перед финальным approve/override решением
- ✅ `xtask module owner-transfer` теперь уже умеет live V2 endpoint и structured `--reason-code` для machine-readable ownership handoff policy
- ✅ `xtask module yank` уже умеет live V2 endpoint и structured `--reason-code` для exceptional yank policy
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
- ✅ `validationStages` теперь несут и canonical execution metadata (`executionMode`, `runnable`, `requiresManualConfirmation`) плюс canonical stage-aware `reason_code` contract (`allowedTerminalReasonCodes`, `suggestedPassReasonCode`, `suggestedFailureReasonCode`, `suggestedBlockedReasonCode`), так что thin clients больше не обязаны держать stage-key heuristics ни для local runner/operator-assisted follow-up paths, ни для terminal stage `reason_code`.
- ✅ Publish/governance policy тоже стал machine-readable contract: dry-run/live `POST /v2/catalog/publish` и `GET /v2/catalog/publish/{request_id}` теперь несут `moderationPolicy` (`first_party_live` / `third_party_manual_only` + restriction reason/code), а thin clients больше не обязаны вычислять live publish readiness только по `ownership == first_party`.
- ✅ Тот же status/lifecycle contract теперь также публикует canonical `governanceActions[]` (`validate`, `approve`, `reject`, `stage_report`, `owner_transfer`, `yank`) с `enabled`, `reason` и `supportedReasonCodes`, так что `/modules` и `xtask` больше не обязаны выводить live governance availability только из request-status эвристик.
- ✅ Для ручной фиксации stage results появился live write-path `POST /v2/catalog/publish/{request_id}/stages` и matching operator command `cargo xtask module stage <request-id> <stage> <status> ...`.
- ✅ Для `compile_smoke` и `targeted_tests` теперь есть реальный local executor поверх operator tooling: `cargo xtask module stage-run <slug> <request-id> <stage> --registry-url ...` прогоняет локальный smoke/test plan и сам пишет `running -> passed/failed` обратно в registry.
- ✅ `security_policy_review` больше не сводится к сырому manual `stage passed/failed`: `cargo xtask module stage-run ... security_policy_review --confirm-manual-review` теперь делает local policy preflight и пишет structured stage trail после явного operator confirmation.
- ✅ В server runtime теперь есть и opt-in background runner для local workspace host: `rustok.registry.validation_runner.enabled=true` поднимает worker, который сам подбирает queued follow-up stages и исполняет тот же local command plan для `compile_smoke` / `targeted_tests`, а при `auto_confirm_manual_review=true` — и для `security_policy_review`.
- ✅ Этот background runner теперь виден и в runtime observability: `/health/runtime` и runtime guardrail metrics показывают его config/effective activity (`configured_enabled`, `active`, `worker_attached`, `supported_stages`, `auto_confirm_manual_review`), так что follow-up automation больше не является скрытым worker'ом.
- ✅ Lifecycle стал богаче без redesign: теперь есть non-terminal состояния `changes_requested` и `on_hold`, live endpoints `POST /v2/catalog/publish/{request_id}/request-changes|hold|resume`, persisted metadata для этих решений прямо в `registry_publish_requests` и server-driven `governanceActions[]` для thin clients.
- ✅ Re-entry path после moderation feedback теперь формализован: `PUT /v2/catalog/publish/{request_id}/artifact` разрешён из `changes_requested`, очищает prior approval/request-changes state и старые validation jobs/stages, затем возвращает request в `submitted`.
- ✅ Generic remote executor contract теперь встроен в server-side governance context: full hosts экспонируют internal `POST /v2/catalog/runner/claim|{claim_id}/heartbeat|complete|fail`, а `registry_validation_stages` хранят lease metadata (`claim_id`, `claimed_by`, `claim_expires_at`, `last_heartbeat_at`, `runner_kind`).
- ✅ Lease expiry больше не живёт только на claim path: full-host runtime теперь поднимает periodic remote-executor reaper по `rustok.registry.remote_executor.requeue_scan_interval_ms`, который возвращает просроченные claims в queue.
- ✅ Runtime observability покрывает и remote executor path: `/health/runtime` и Prometheus теперь публикуют `remote_executor` snapshot (`configured_enabled`, `token_configured`, `reaper_attached`, `lease_ttl_ms`, `requeue_scan_interval_ms`) рядом с `validation_runner`.
- ⚠️ Полноценный внешний sandbox/worker deployment всё ещё остаётся operator/infra задачей: server-side API, lease model и reaper уже есть, но отдельный production-grade remote executor client/runtime вне `apps/server` ещё не поставляется как готовый managed компонент.
- ⚠️ Базовая persistence-модель для ownership и audit trail уже есть (`registry_module_owners`, `registry_governance_events`), и role split для review vs release-management уже начал ужесточаться: approve/reject/stage-review доступны owner + review actors (`registry:*`, `governance:moderator`, `moderator:*`), а owner-transfer/yank уже больше не приравниваются ко всем generic moderators.
- ✅ Для exceptional yank-path теперь есть более формальный policy contract: live `POST /v2/catalog/yank` требует не только human-readable `reason`, но и structured `reason_code` (`security|legal|malware|critical_regression|rollback|other`), а audit trail сохраняет оба значения.
- ✅ Для manual governance reject-path теперь тоже есть structured policy contract: live `POST /v2/catalog/publish/{request_id}/reject` требует не только human-readable `reason`, но и structured `reason_code` (`policy_mismatch|quality_gate_failed|ownership_mismatch|security_risk|legal|other`), а `request_rejected` audit event сохраняет оба значения.
- ✅ Для explicit owner-transfer-path теперь тоже есть structured policy contract: live `POST /v2/catalog/owner-transfer` требует не только human-readable `reason`, но и structured `reason_code` (`maintenance_handoff|team_restructure|publisher_rotation|security_emergency|governance_override|other`), а `owner_transferred` audit event сохраняет оба значения.
- ✅ Для финального publish-approval override теперь тоже есть structured policy contract: если follow-up validation stages ещё не закрыты, live `POST /v2/catalog/publish/{request_id}/approve` требует explicit `reason` + `reason_code` (`manual_review_complete|trusted_first_party|expedited_release|governance_override|other`), а audit trail сохраняет отдельный `publish_approval_override`.
- ✅ Тонкие operator clients теперь тоже видят этот policy contract заранее: `GET /v2/catalog/publish/{request_id}` отдаёт `followUpGates`, canonical `validationStages`, `approvalOverrideRequired` и `approvalOverrideReasonCodes`, так что preflight для approve больше не зависит только от live error path.
- ✅ Follow-up stage trail тоже стал более структурированным: `POST /v2/catalog/publish/{request_id}/stages` и `cargo xtask module stage ...` теперь умеют structured `reason_code`, а для live terminal stage updates (`passed` / `failed` / `blocked`) он уже обязателен. Более того, terminal stage reports теперь валидируются по stage-aware subset reason codes, а не только по общему global allowlist. `stage-run` и live `module stage` берут canonical suggested codes из `validationStages` (`local_runner_passed`, `build_failure`, `test_failure`, `manual_review_complete`, `policy_preflight_failed`, ...), так что moderation history по stage updates больше не живёт только как prose-detail и локальные CLI heuristics.
- ⚠️ В stricter policy layer осталось добить:
  - richer moderation decisions beyond approve/reject/owner-transfer/yank
  - аналогично формализовать exceptional governance/release-management решения beyond current approve/reject/owner-transfer/yank contracts

### Блок B. Отдельный deployment для `modules.rustok.dev`

- ⚠️ `registry_only` runtime уже есть, но отдельный внешний deployment ещё не оформлен до production-ready состояния.
- ⚠️ Базовый rollout/runbook для dedicated catalog host уже зафиксирован в `apps/server/docs/health.md`: canonical build profile = `headless-api`, runtime host mode = `registry_only`, acceptance = V1 list/detail + cache headers + reduced OpenAPI + negative write-path checks.
- ✅ Для внешнего dedicated host теперь есть и automation-path: `scripts/verify/verify-deployment-profiles.sh` и PowerShell-вариант поддерживают optional external smoke через `RUSTOK_REGISTRY_BASE_URL` (+ optional `RUSTOK_REGISTRY_SMOKE_SLUG`) и optional artifact capture через `RUSTOK_REGISTRY_EVIDENCE_DIR`; они проверяют public `health/runtime`, `health/modules`, V1 list/detail, cache headers, reduced OpenAPI (JSON + YAML) и negative write-path contract уже на deployed instance.
- ✅ `apps/server/docs/health.md` теперь также фиксирует provider-agnostic hand-off contract для внешнего host: edge/cache/TLS invariants, запрет на path rewrites / HTML error-page substitution, и minimal evidence package после rollout.
- ⚠️ По `modules.rustok.dev` незакрытым остаётся уже не platform contract, а только фактическое выполнение provider-specific infra части: traffic switch, TLS/CDN/WAF policy и release-hand-off в конкретной production среде.

### Блок C. Покрытие UI и operator polish

- ✅ Manifest-wired UI coverage уже не только документируется вручную: live contract включает расширенный набор admin-only core/optional surfaces и публикует machine-readable `uiClassification` + `hasAdminUi` / `hasStorefrontUi` в catalog/GraphQL/Admin.
- ✅ Existing no-UI path modules больше не висят как prose-only хвост: их `capability_only` статус уже зафиксирован прямо в manifest contract и доходит до `/modules`.
- ✅ `/modules` уже умеет не только показывать lifecycle, но и запускать интерактивные governance-действия, показывать policy hints, copyable `xtask`/HTTP/curl snippets, headers/body hints и operator commands.
- ✅ `/modules` уже различает automatic validation failure и manual governance reject в `Validation summary`, а также показывает отдельный `Ready for review` сигнал для validated request, который ещё не опубликован.
- ✅ `/modules` теперь также показывает отдельный `Follow-up gates` summary для `compile_smoke` / `targeted_tests` / `security_policy_review`, чтобы внешние async/manual gates были видны отдельно от базовой artifact/manifest validation.
- ✅ `/modules` уже показывает явные authority-линии для review / owner-transfer / yank и выравнивает live API hints под фактическую server-policy, а не под legacy generic `governance`.
- ✅ `/modules` уже показывает отдельный moderation history layer поверх audit trail: ключевые review/yank/owner-transfer/stage-report решения вынесены в более быстрый operator-facing timeline.
- ✅ `/modules` теперь показывает richer per-check async validation feedback: automated checks и validation job trace выводятся отдельно от high-level summary и общего audit trail.
- ✅ `/modules` теперь ещё и заранее подсвечивает stage-aware approve override в interactive actions: оператор видит, какие follow-up stages ещё не passed и какие `reason_code` допустимы, до live approve-запроса.
- ✅ `/modules` теперь также подсказывает `cargo xtask module stage-run ...` для всех follow-up stages; для `security_policy_review` hint явно требует `--confirm-manual-review`, а HTTP body hint для stage update уже показывает structured `reason_code`.
- ✅ `/modules`, `cargo xtask module publish --auto-approve`, live `cargo xtask module stage` и `cargo xtask module stage-run` теперь опираются на server-driven execution metadata и stage-aware `reason_code` hints у `validationStages`, а не на локальные `compile_smoke`/`targeted_tests`/`security_policy_review` heuristics при выборе auto-run/operator-assisted path или terminal reason code.
- ✅ `/modules` и native/GraphQL `marketplaceModule` теперь получают `registryLifecycle.moderationPolicy` даже для модулей без persisted publish/release history, а `cargo xtask module publish` использует тот же server-driven publish preflight вместо локального `first_party` gate.
- ✅ `/modules` теперь также показывает machine-readable surface classification из manifest/catalog contract: `UI class`, отдельные `Admin UI` / `Storefront UI` признаки и unified badges в catalog cards/detail panel.

### Блок D. Тесты

- ⚠️ Точечные `cargo check` по релевантным пакетам уже проходят для большинства последних шагов.
- ✅ `xtask` уже получил targeted unit coverage для V2 operator paths: publish/stage/owner-transfer/yank dry-run/live payload contracts, `requeue=true` contract, explicit `detail = null` stability, базовые argument-count guards, early CLI guards для empty request/stage ids, invalid semver и unknown slug, live-mode guards для missing `--registry-url` / missing `--reason`, `registry_url` env fallback/CLI precedence, `--reason`/`--detail` trimming, empty owner-transfer actor guard и loopback/no-proxy guardrails (включая IPv6 `::1`).
- ✅ `cargo xtask module publish --auto-approve` теперь использует stage-aware status preflight и останавливается раньше live approve, если request требует override, а `--approve-reason` / `--approve-reason-code` не были переданы.
- ✅ `apps/server` уже имеет targeted coverage для ключевых V2 lifecycle-переходов и reduced host: requeue/retry semantics validation jobs, materialization persisted `validation_stages`, stage requeue attempt numbering, `reason_code` contracts для reject/yank/owner-transfer/approve override и `registry_only` negative-surface smoke.
- ✅ `apps/server` теперь также покрывает targeted integration tests для `request_changes`, `hold/resume`, remote runner happy-path (`claim -> heartbeat -> complete`), invariant `approved != projected into V1 until explicit publish` и отсутствие `request-changes|hold|resume|runner/*` на `registry_only` host.
- ✅ `apps/server::modules::manifest` уже покрывает manifest-driven UI/i18n guardrails: `ui_classification`, locale normalization, `default_locale ∈ supported_locales` и обязательность bundle files.
- ⚠️ Полный workspace/test graph регулярно блокируется незавершённой параллельной разработкой в соседних crate-ах.
- ⬜ Нужны более устойчивые targeted tests для:
  - projection V2 → V1
  - full-host background validation runner и его observability/worker-attachment contract
  - end-to-end contract между native/GraphQL consumers и server-driven `validationStages` / `governanceActions` / `moderationPolicy`

## Приоритет выполнения

1. Довести ownership/governance policy layer до richer moderation и release-management решений beyond current approve/reject/owner-transfer/yank.
2. Закрыть provider-specific production deployment path для `modules.rustok.dev`.
3. Довести внешний remote/sandboxed executor deployment и worker runtime поверх уже существующего server-side claim/lease API.
4. Уплотнить targeted test coverage вокруг V2 → V1 projection, full-host validation runner и server-driven lifecycle contracts.
5. Сохранить дисциплину manifest/docs/UI/i18n contracts для новых path-модулей и не допускать регрессий в wave 1/1.5.

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

