# RusTok: план закрытия module-system

> **Дата**: 2026-03-19  
> **Актуализировано**: 2026-04-10  
> **Назначение**: зафиксировать текущее состояние модульной платформы, разделение `Registry V1` и `Registry V2`, а также остаток работ до production-ready контура.

Легенда:
- `✅` реализовано
- `⚠️` частично реализовано
- `⬜` не начато

## Короткий вывод

- `Registry V1` уже существует как постоянный read-only catalog API.
- `Registry V2` уже существует как рабочий write/governance контур с текущим action set, раздельными automated validation jobs, follow-up stages и thin remote-runner path через `runner/*` + `xtask`.
- `DeploymentProfile` и `RuntimeHostMode` уже разделены в коде и должны оставаться независимыми осями.
- Manifest-wired Leptos UI уже сведён к явной repo-side классификации, а основной открытый repo-хвост сейчас не в новых UI-классах, а в targeted verification и поддержании docs/tooling discipline вокруг уже собранного контракта.

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

Это закрывает основную repo-side цель трека: оператор может устанавливать, удалять, обновлять и деплоить модули из Admin UI, видя lifecycle/progress и не выходя в ручной backend workflow.
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
| `dual-surface` | `blog`, `commerce`, `forum`, `pages`, `pricing`, `product`, `region`, `search` | admin + storefront |
| `admin-only` | `channel`, `comments`, `customer`, `fulfillment`, `index`, `inventory`, `media`, `order`, `outbox`, `rbac`, `tenant`, `workflow` | осознанные module-owned admin slices |
| `storefront-only` | `cart` | отдельный storefront-owned slice без admin UI |
| `no-ui / capability-only` | `alloy`, `auth`, `cache`, `content`, `email`, `payment`, `profiles`, `taxonomy` | path entries без manifest-declared Leptos UI |

Дополнительно:

- ✅ Наличие `admin/Cargo.toml` или `storefront/Cargo.toml` без соответствующего `[provides.*_ui].leptos_crate` теперь считается wiring error.
- ✅ Package-owned i18n contract уже живёт в manifest-driven UI:
  - admin surfaces `workflow`, `rbac`, `tenant`, `index`, `outbox`, `pages`, `comments`, `channel`, `forum`, `search`, `commerce`
  - storefront surfaces `blog`, `pages`, `commerce`, `forum`, `search`
- ✅ Базовая классификация path entries теперь зафиксирована явно через manifest audit, а не выводится по умолчанию из отсутствия sub-crate; отдельным хвостом остаётся только периодическая сверка этой таблицы с живыми `rustok-module.toml`.

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
- ✅ `POST /v2/catalog/publish/{request_id}/request-changes`
- ✅ `POST /v2/catalog/publish/{request_id}/hold`
- ✅ `POST /v2/catalog/publish/{request_id}/resume`
- ✅ `POST /v2/catalog/owner-transfer`
- ✅ `POST /v2/catalog/yank`
- ✅ internal full-host-only runner API:
  - `POST /v2/catalog/runner/claim`
  - `POST /v2/catalog/runner/{claim_id}/heartbeat`
  - `POST /v2/catalog/runner/{claim_id}/complete`
  - `POST /v2/catalog/runner/{claim_id}/fail`

Текущее lifecycle-состояние:

- ✅ `POST /v2/catalog/publish` делает sync metadata validation и создаёт request отдельно от artifact upload
- ✅ upload теперь только сохраняет artifact и ставит request в `submitted`
- ✅ validation вынесена в отдельный lifecycle-step `/validate`
- ✅ `/validate` валидирует bundle against request
- ✅ `/validate` теперь работает как queue boundary и переводит request в background `validating`, а не держит bundle-check inline в HTTP вызове
- ✅ У validation теперь есть отдельный persisted job-layer `registry_validation_jobs` с попытками и статусами `queued` / `running` / `succeeded` / `failed`.
- ✅ request проходит через `artifact_uploaded -> submitted -> validating -> approved` или `rejected`
- ✅ `approved` теперь означает `review-ready` после automated artifact/manifest validation, а не уже опубликованный release
- ✅ появились дополнительные non-terminal governance states:
  - `changes_requested`
  - `on_hold`
- ✅ публикация release происходит отдельным governance action
- ✅ `reject`, `owner-transfer` и `yank` требуют обязательную governance reason, а live `reject`/`owner-transfer`/`yank` теперь ещё и structured `reason_code`
- ✅ `request-changes`, `hold` и `resume` теперь тоже живут как machine-readable governance actions с persisted metadata, `reason_code` и audit trail
- ✅ финальный `approve` теперь тоже стал policy-aware: если follow-up validation stages ещё не `passed`, live approval требует explicit override `reason + reason_code`, а audit trail пишет отдельный `publish_approval_override`
- ✅ published releases уже проецируются обратно в `V1`

Governance first cut:

- ✅ live authority больше не берётся из `x-rustok-actor` / `x-rustok-publisher`; user bearer auth остаётся единственным live source для registry write-path
- ✅ persisted slug owner binding уже сохраняется отдельно от governance actor; release publisher identity и `/modules` lifecycle UX теперь читают этот binding, а `x-rustok-publisher` отделён от audit actor `x-rustok-actor`
- ✅ requested publisher identity теперь также сохраняется прямо в `registry_publish_requests`, и approve/reject policy больше не опирается только на `requested_by`
- ✅ persisted audit trail теперь сохраняется в `registry_governance_events` и уже отражается в `/modules` как recent governance events для publish/upload/validate/approve/reject/owner-transfer/yank/owner-binding переходов
- ✅ publish/release metadata теперь выровнены под platform multilingual storage contract: `module name` / `description` вынесены в `registry_*_translations`, а base rows держат только language-agnostic state + `default_locale`
- ✅ есть first-cut policy для governance actors и slug-scoped publishers
- ✅ ownership transfer теперь вынесен в явный `POST /v2/catalog/owner-transfer` с обязательной причиной, persisted owner rebind и отдельным audit event `owner_transferred`
- ✅ approve/reject больше не считаются self-review шагом через `publisher_identity`: review path теперь требует governance actor или текущего persisted owner
- ✅ lifecycle consumers уже получают derived `follow_up_gates` и structured `automated_checks` через GraphQL/Admin read-path
- ✅ `xtask module publish` уже умеет live orchestration и по умолчанию останавливается на `approved` / `review-ready`; финальный governance `approve` теперь делается только по явному `--auto-approve`
- ✅ `xtask module publish --auto-approve` теперь умеет и explicit approval override через `--approve-reason` / `--approve-reason-code`, если follow-up stages ещё не закрыты
- ✅ `xtask module stage-run` теперь закрывает первый execution-path для follow-up stages: `compile_smoke` и `targeted_tests` можно прогнать локально и сразу записать их lifecycle в registry без ручного `running/passed/failed` шага, а `security_policy_review` теперь тоже идёт через operator-assisted local preflight + explicit `--confirm-manual-review`
- ✅ `xtask module publish --auto-approve` теперь тоже стал stage-aware orchestration path, а не только approve caller: он сам прогоняет pending local stages (`compile_smoke`, `targeted_tests`, опционально `security_policy_review` при `--confirm-manual-review`) перед финальным approve/override решением
- ✅ Live operator flows в `xtask` теперь больше не полагаются на скрытый synthetic review actor: `module publish`, `module stage`, `module stage-run`, `module owner-transfer` и `module yank` требуют bearer auth через `--auth-token` / `RUSTOK_MODULE_AUTH_TOKEN`, а live preflight/mutation path использует тот же session-backed user principal.
- ✅ lease-based remote executor API уже живёт в `apps/server`: stage claims/heartbeats/completion/failure идут через `runner/*`, есть persisted lease metadata, periodic reaper и observability в `/health/runtime` + Prometheus
- ✅ `cargo xtask module runner <runner-id> ...` теперь даёт thin remote worker поверх этого server-driven contract: он claim-ит runnable stages, шлёт heartbeat и завершает `compile_smoke` / `targeted_tests`, а `security_policy_review` остаётся opt-in через `--confirm-manual-review`
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
- ✅ Для ручной фиксации stage results появился live write-path `POST /v2/catalog/publish/{request_id}/stages` и matching operator command `cargo xtask module stage <request-id> <stage> <status> ...`.
- ✅ Для `compile_smoke` и `targeted_tests` теперь есть реальный local executor поверх operator tooling: `cargo xtask module stage-run <slug> <request-id> <stage> --registry-url ...` прогоняет локальный smoke/test plan и сам пишет `running -> passed/failed` обратно в registry.
- ✅ `security_policy_review` больше не сводится к сырому manual `stage passed/failed`: `cargo xtask module stage-run ... security_policy_review --confirm-manual-review` теперь делает local policy preflight и пишет structured stage trail после явного operator confirmation.
- ✅ Для целей платформы достаточно server-driven runner contract (`runner/*`, lease/reaper, health/metrics). Внешняя автоматизация поверх этого contract может быть добавлена deployment-средой, но managed worker/fleet не считается открытым repo-side scope `module-system`.
- ⚠️ Базовая persistence-модель для ownership и audit trail уже есть (`registry_module_owners`, `registry_governance_events`), и role split для review vs release-management уже начал ужесточаться: approve/reject/stage-review доступны owner + review actors (`registry:*`, `governance:moderator`, `moderator:*`), а owner-transfer/yank уже больше не приравниваются ко всем generic moderators.
- ✅ Для exceptional yank-path теперь есть более формальный policy contract: live `POST /v2/catalog/yank` требует не только human-readable `reason`, но и structured `reason_code` (`security|legal|malware|critical_regression|rollback|other`), а audit trail сохраняет оба значения.
- ✅ Для manual governance reject-path теперь тоже есть structured policy contract: live `POST /v2/catalog/publish/{request_id}/reject` требует не только human-readable `reason`, но и structured `reason_code` (`policy_mismatch|quality_gate_failed|ownership_mismatch|security_risk|legal|other`), а `request_rejected` audit event сохраняет оба значения.
- ✅ Для explicit owner-transfer-path теперь тоже есть structured policy contract: live `POST /v2/catalog/owner-transfer` требует не только human-readable `reason`, но и structured `reason_code` (`maintenance_handoff|team_restructure|publisher_rotation|security_emergency|governance_override|other`), а `owner_transferred` audit event сохраняет оба значения.
- ✅ Для финального publish-approval override теперь тоже есть structured policy contract: если follow-up validation stages ещё не закрыты, live `POST /v2/catalog/publish/{request_id}/approve` требует explicit `reason` + `reason_code` (`manual_review_complete|trusted_first_party|expedited_release|governance_override|other`), а audit trail сохраняет отдельный `publish_approval_override`.
- ✅ Тонкие operator clients теперь тоже видят этот policy contract заранее: `GET /v2/catalog/publish/{request_id}` отдаёт `followUpGates`, canonical `validationStages`, `approvalOverrideRequired`, `approvalOverrideReasonCodes` и machine-readable `governanceActions`, так что preflight для approve и action availability больше не зависят только от live error path или локальных status-эвристик.
- ✅ Для status-preflight этот contract теперь auth-aware: если thin client передаёт session-backed bearer auth, `governanceActions` режутся до реально разрешённых request-level действий для этого user principal, а не только до status-driven superset.
- ✅ `/modules` теперь использует `registryLifecycle` только как summary/read-model: actor-agnostic `governanceActions` в нём сведены к release-management hints (`owner_transfer`, `yank`), а authoritative request-level operator contract читается отдельным bearer-auth fetch к `GET /v2/catalog/publish/{request_id}`.
- ✅ `xtask` теперь тоже покрывает явные governance follow-up actions поверх того же status contract: `cargo xtask module request-changes|hold|resume <request-id> --auth-token <token> --reason <text> --reason-code <code> --registry-url ...` сначала смотрит `governanceActions`, а потом уже вызывает live route.
- ✅ Follow-up stage trail тоже стал более структурированным: `POST /v2/catalog/publish/{request_id}/stages` и `cargo xtask module stage ...` теперь умеют structured `reason_code`, а для live terminal stage updates (`passed` / `failed` / `blocked`) он уже обязателен. `stage-run` проставляет его автоматически (`local_runner_passed`, `build_failure`, `test_failure`, `manual_review_complete`, `policy_preflight_failed`, ...), так что moderation history по stage updates больше не живёт только как prose-detail.
- ✅ Stricter policy layer для текущего action set считается закрытым для repo-side целей: authority и `reason` / `reason_code` enforcement живут на сервере, а thin clients читают server-driven preflight без локального request-level policy.

### Блок B. Отдельный deployment для `modules.rustok.dev`

- ⚠️ `registry_only` runtime уже есть, но отдельный внешний deployment ещё не оформлен до production-ready состояния.
- ⚠️ Базовый rollout/runbook для dedicated catalog host уже зафиксирован в `apps/server/docs/health.md`: canonical build profile = `headless-api`, runtime host mode = `registry_only`, acceptance = V1 list/detail + cache headers + reduced OpenAPI + negative write-path checks.
- ✅ Для внешнего dedicated host теперь есть и automation-path: `scripts/verify/verify-deployment-profiles.sh` и PowerShell-вариант поддерживают optional external smoke через `RUSTOK_REGISTRY_BASE_URL` (+ optional `RUSTOK_REGISTRY_SMOKE_SLUG`) и optional artifact capture через `RUSTOK_REGISTRY_EVIDENCE_DIR`; они проверяют public `health/runtime`, `health/modules`, V1 list/detail, cache headers, reduced OpenAPI (JSON + YAML) и negative write-path contract уже на deployed instance.
- ✅ Negative smoke для dedicated host теперь также покрывает более новый V2 surface: `request-changes`, `hold`, `resume` и `runner/*` не должны быть доступны на `registry_only`.
- ✅ `apps/server/docs/health.md` теперь также фиксирует provider-agnostic hand-off contract для внешнего host: edge/cache/TLS invariants, запрет на path rewrites / HTML error-page substitution, и minimal evidence package после rollout.
- ⚠️ По `modules.rustok.dev` незакрытым остаётся уже не platform contract, а только фактическое выполнение provider-specific infra части: traffic switch, TLS/CDN/WAF policy и release-hand-off в конкретной production среде.

### Блок C. Покрытие UI и operator polish

- ✅ Manifest-wired UI coverage уже сведён к явной repo-side классификации `dual-surface` / `admin-only` / `storefront-only` / `no-ui`.
- ⚠️ Дальше здесь нужен не redesign, а только периодический audit новых path entries и сверка manifest table с `modules.toml`.
- ✅ `/modules` уже умеет не только показывать lifecycle, но и запускать интерактивные governance-действия, показывать policy hints, copyable `xtask`/HTTP/curl snippets, headers/body hints и operator commands.
- ✅ `/modules` уже различает automatic validation failure и manual governance reject в `Validation summary`, а также показывает отдельный `Ready for review` сигнал для validated request, который ещё не опубликован.
- ✅ `/modules` теперь также показывает отдельный `Follow-up gates` summary для `compile_smoke` / `targeted_tests` / `security_policy_review`, чтобы внешние async/manual gates были видны отдельно от базовой artifact/manifest validation.
- ✅ `/modules` уже показывает явные authority-линии для review / owner-transfer / yank и выравнивает live API hints под фактическую server-policy, а не под legacy generic `governance`.
- ✅ `/modules` уже показывает отдельный moderation history layer поверх audit trail: ключевые review/yank/owner-transfer/stage-report решения вынесены в более быстрый operator-facing timeline.
- ✅ `/modules` теперь показывает richer per-check async validation feedback: automated checks и validation job trace выводятся отдельно от high-level summary и общего audit trail.
- ✅ `/modules` теперь ещё и заранее подсвечивает stage-aware approve override в interactive actions: оператор видит, какие follow-up stages ещё не passed и какие `reason_code` допустимы, до live approve-запроса.
- ✅ `/modules` теперь также подсказывает `cargo xtask module stage-run ...` для всех follow-up stages; для `security_policy_review` hint явно требует `--confirm-manual-review`, а HTTP body hint для stage update уже показывает structured `reason_code`.

### Блок D. Тесты

- ⚠️ Точечные `cargo check` по релевантным пакетам уже проходят для большинства последних шагов.
- ✅ `xtask` уже получил targeted unit coverage для V2 operator paths: publish/stage/owner-transfer/yank dry-run/live payload contracts, `requeue=true` contract, explicit `detail = null` stability, базовые argument-count guards, early CLI guards для empty request/stage ids, invalid semver и unknown slug, live-mode guards для missing `--registry-url` / missing `--reason`, `registry_url` env fallback/CLI precedence, `--reason`/`--detail` trimming, empty owner-transfer actor guard и loopback/no-proxy guardrails (включая IPv6 `::1`).
- ✅ `xtask module runner` уже получил targeted unit coverage для runner-id/token/stage-set/interval argument guards.
- ✅ `cargo xtask module publish --auto-approve` теперь использует stage-aware status preflight и останавливается раньше live approve, если request требует override, а `--approve-reason` / `--approve-reason-code` не были переданы.
- ⚠️ Полный workspace/test graph регулярно блокируется незавершённой параллельной разработкой в соседних crate-ах, поэтому acceptance по `module-system` держится на targeted checks.
- ✅ Repo-side verification baseline для `module-system` теперь сводится к поддерживаемому targeted-набору, а не к открытому feature backlog:
  - auth-aware `GET /v2/catalog/publish/{request_id}` и текущий governance action set
  - V2 lifecycle transitions, включая retry/requeue semantics и `reason` / `reason_code` enforcement
  - projection V2 → V1
  - thin runner contract: claim/heartbeat/complete/fail, lease expiry и stale/duplicate claim rejection
  - `registry_only` reduced surface

## Приоритет выполнения

1. Repo-side цель `module-system` считается закрытой для сценария Admin-driven install/uninstall/upgrade/deploy с progress feedback.
2. Закрыть production deployment path для `modules.rustok.dev` как отдельную infra-задачу.
3. Поддерживать manifest-wired UI / i18n audit как периодическую сверку, а не как неразмеченный open-ended backlog.
4. Внешняя автоматизация поверх `runner/*` допускается как deployment/ops pattern, но не входит в platform backlog.

## Критерии завершения

План можно считать закрытым, когда одновременно выполняются условия:

- `V1` стабилен как постоянный read-only catalog API.
- `V2` имеет полноценный async publish/governance lifecycle.
- thin remote runner path работает по server-driven lease contract и наблюдается через runtime health/metrics.
- `registry_only` развёртывается как отдельный catalog host без monolith surface.
- publish/yank flow работает и в `Monolith + full`, и в `HeadlessApi + full`.
- `/modules` показывает operator-friendly lifecycle, validation и governance state, а также умеет запускать базовые V2 governance-действия.
- UI wiring всех path-модулей честно классифицирован, а manifest / i18n guardrails проверяются tooling-слоем.
- Центральный `docs/modules/registry.md` при этом остаётся ownership-картой, а не вторым источником policy/UI-contract правды.
- В открытом backlog остаются только provider-specific rollout `modules.rustok.dev` и периодический audit новых path-модулей; managed worker/fleet больше не считается частью repo-side `module-system` scope.

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
