# План внедрения rich-text (Tiptap) и GrapesJS Page Builder

Этот документ фиксирует **отдельный план реализации** для двух связанных, но разных контуров:
- `Tiptap`/`rt_json_v1` для rich-text сценариев blog/forum;
- `GrapesJS`/`grapesjs_v1` для визуального Page Builder как **отдельного FBA-модуля-референса** с последующей интеграцией в `pages`.

Важно: в рамках перехода на FBA сначала создаётся **самостоятельный reference-модуль builder-а**, и только затем `pages` выступает как consumer этого модуля. Детали интеграции `pages` остаются в `crates/rustok-pages/docs/implementation-plan.md` (раздел `Dedicated page-builder track`), а этот документ фиксирует платформенный порядок внедрения и release-gate.

В контексте перехода на FBA этот трек должен использоваться как **референсный “ideal FBA module”**: все новые шаги по `PageBuilder` проектируются в FBA-модели (capability contracts, composable lifecycle, tenant/module feature controls, observability-first), без возврата к legacy module-схеме, кроме явно помеченных compatibility-слоёв.

## 1. Цель и критерии готовности

Цель: безопасно перевести rich-text admin UX blog/forum на `rt_json_v1`, отдельно собрать референсный FBA-модуль visual builder-а, и только после этого подключить его к `pages` без деградации RBAC, publish-пайплайна, индексации и storefront-rendering.

Критерии завершения:
- `rt_json_v1` используется как основной rich-text формат ввода в admin для blog/forum;
- pages редактируются через `GrapesJS`-builder с каноническим body-форматом `grapesjs_v1`, а не через Tiptap-rich-text flow;
- миграция legacy markdown проведена tenant-by-tenant с подтверждённым rollback-сценарием;
- интеграционные/e2e проверки и observability release-gate пройдены;
- feature flag переведён в `default-on` после стабилизации.
- FBA reference-модуль builder-а прошёл pilot как независимый модульный контур до широкого включения в `pages`.

### Технологический baseline Page Builder (обязательное ограничение)

- Базовый production-path для визуального builder в RusTok — **open-source GrapesJS** (self-hosted).
- Контракт `grapesjs_v1` остаётся vendor-neutral: backend/runtime не должен требовать vendor-specific payload или proprietary API.
- Для Leptos/Flutter в baseline достаточно contract-safe surfaces (preview/tree/properties/publish) поверх общего backend-контракта; 1:1 визуальный клон Next.js builder не является обязательным критерием текущего rollout.

## 2. Статус фаз

- [x] **Фаза 0 — Контракт и backend-baseline зафиксированы**
- [~] **Фаза 1 — Выделение FBA reference-модуля builder-а**
- [~] **Фаза 2 — Интеграция consumer-ов (в т.ч. `pages`) с reference-модулем**
- [~] **Фаза 3 — Feature flags и стратегия rollout**
- [ ] **Фаза 4 — Миграция legacy markdown → rt_json_v1**
- [ ] **Фаза 5 — Release-gate: тесты, RBAC, observability**
- [ ] **Фаза 6 — Pre-production smoke и pilot rollout**
- [ ] **Фаза 7 — Default-on и пост-релизная стабилизация**

## 3. Фазы реализации

### Фаза 0 — Контракт и backend-baseline (выполнено)

**Статус:** [x] Done

- [x] Единый контракт rich-text/page-builder в backend: `markdown` + `rt_json_v1` + `grapesjs_v1`.
- [x] Серверная sanitize/validation для `rt_json_v1` и schema-check для `grapesjs_v1` включены в write-path.
- [x] Blog/Forum/Pages read-path возвращает `*_format` и `content_json` для rich payload.
- [x] Доступен migration job `migrate_legacy_richtext` для tenant-scoped запуска.

**Выход артефакта:** контракт готов к consumer-интеграции.

### Фаза 1 — Выделение FBA reference-модуля builder-а

**Статус:** [~] In progress

- [x] Зафиксировать самостоятельный FBA reference-контур builder-а на уровне центральной документации и правил rollout (без возврата к pages-owned реализации).
- [x] Зафиксировать capability-contracts (`preview/tree/properties/publish`) как минимально обязательный consumer surface для reference-модуля.
- [~] Подготовить module health contract + observability baseline для reference-модуля (метрики и release-gate определены, автоматизация в CI остаётся в Phase 5).
- [~] Определить compatibility-периметр legacy payload-ов как временный слой и зафиксировать sunset criteria (критерии заданы, tenant-график отключения фиксируется в rollout runbook).
- [ ] Выровнять contract parity для Next/Leptos/Flutter как consumer-ов reference-модуля на уровне production-readiness.

**DoD фазы:** reference-модуль способен жить и катиться независимо, а `pages` и другие контуры подключаются к нему как consumer-ы по стабильному FBA-контракту.

### Фаза 2 — Интеграция consumer-ов (в т.ч. `pages`) с reference-модулем

**Статус:** [~] In progress

- [x] Подключить `RtJsonEditor` в production CRUD-flow blog.
- [x] Подключить `ForumReplyEditor` в production CRUD-flow forum.
- [x] Подключить `PageBuilder` surfaces в `pages`-flow как consumer-контур.
- [x] Зафиксировать parity-план для `apps/next-admin` и `apps/admin` на уровне capability-contract.
- [x] Выровнять UX-обработку validation/sanitize ошибок в формах.
- [x] Синхронизировать dependency с Flutter registry/codegen планом (`docs/research/flutter.md`, anti-drift guardrail).
- [~] Зафиксировать FBA migration contract для `rustok-pages`: pages остаётся владельцем page/menu runtime, но визуальный builder-домен потребляется как внешний reference-capability слой.
- [x] Вынести в отдельный runbook процедуру включения/отключения builder-capabilities tenant-by-tenant без отката всего pages runtime (см. `crates/rustok-pages/docs/implementation-plan.md`, разделы `Tenant switch procedure` + `FBA execution backlog`).
- [~] Свести capability readiness к единому FBA execution backlog для `pages` (metadata/provider contract, fallback semantics, observability correlation, CI fallback-gate).

**DoD фазы:** `pages` и соседние контуры не владеют builder-доменом напрямую, а используют reference-модуль через стабильный контракт.

### Cross-plan dependency note (обязательно для hand-off)

- До завершения backend/parity шагов этой дорожной карты Flutter-команда может делать только contract-safe registry scaffolding.
- Любые изменения mobile module contracts для page-builder обязаны содержать явное уведомление о зависимостях и блокерах между:
  - `docs/research/flutter.md`;
  - текущим документом;
  - `crates/rustok-pages/docs/implementation-plan.md`.

### Фаза 3 — Feature flags и стратегия rollout

**Статус:** [~] In progress

- [~] Ввести флаги уровня tenant/module/form (baseline-профиль и naming зафиксированы, rollout automation остаётся в бэклоге).
- [x] Определить стратегию включения: internal → pilot → broad rollout.
- [x] Подготовить матрицу включения/исключения по tenant и модулю (см. Phase 3.2).
- [~] Согласовать операционный runbook переключений (процедура и rollback-условия зафиксированы, требуется owner sign-off в execution log).
- [x] Зафиксировать baseline-only rollout: OSS GrapesJS + vendor-neutral `grapesjs_v1` contract без расширения platform-контракта под вендор-специфику (см. Phase 3.5).
- [~] Зафиксировать FBA governance-профиль для `rustok-pages` как reference-модуля: capability boundaries, control-plane hooks, module health contract, ownership SLA (профиль и SLA baseline заданы в Phase 3.6; acceptance в Phase 5).

**DoD фазы:** controlled rollout возможен без redeploy.


### Фаза 3.1 — Минимальный профиль feature flags (FBA baseline)

Обязательный baseline-профиль до pilot-wave:

- `builder.enabled` — глобальный tenant-level флаг доступа к visual builder контру.
- `builder.preview.enabled` — разрешение preview capability.
- `builder.properties.enabled` — разрешение редактирования properties/tree.
- `builder.publish.enabled` — разрешение publish через builder path.
- `builder.legacy_bridge_readonly` — принудительный read-only режим legacy block bridge.

Правила:

1. Выключение `builder.publish.enabled` **не** должно ломать page read-path и direct publish для legacy payload.
2. Выключение `builder.enabled` переводит UI в fallback-поведение (read-only + диагностическое сообщение), без 5xx на storefront/admin list views.
3. Для pilot-tenants запрещено включать `builder.publish.enabled=true`, если `builder.preview.enabled=false`.

### Фаза 3.1.1 — Fallback matrix для capability-профилей

Единая матрица fallback-поведения синхронизирована с runtime helper-ами `rustok-page-builder::rollout` и consumer manifest `rustok-pages`. Она задаёт минимальный ожидаемый outcome для Next/Leptos/Flutter adapters без требования 1:1 UI-клона.

| Профиль | Admin visual path | Preview | Properties/tree | Publish | Read/list/storefront paths | Disabled capabilities |
|---|---|---|---|---|---|---|
| `all_on` | `editable_builder` | `available` | `available` | `available` | `stable` | — |
| `publish_off` | `editable_builder_publish_disabled` | `available` | `available` | `typed_feature_disabled_error` | `stable` | `publish` |
| `preview_off` | `preview_hidden_properties_available` | `typed_feature_disabled_error` | `available` | `typed_feature_disabled_error` | `stable` | `preview`, `publish` |
| `builder_off` | `readonly_fallback` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `stable` | `preview`, `tree`, `properties`, `publish` |

Правила синхронизации:

1. При изменении профилей сначала обновляется runtime matrix в `crates/rustok-page-builder/src/rollout.rs`.
2. Затем синхронизируются consumer manifest/docs (`rustok-pages`) и этот центральный план.
3. Anti-drift gate `verify-page-builder-fallback-matrix-docs.mjs`, provider runtime gate `verify-page-builder-runtime-fallback-gate.mjs` и `rustok-pages` consumer gate `verify-page-builder-pages-fallback-gate.mjs` должны оставаться частью baseline-проверки до Wave 1.

### Фаза 3.2 — Матрица rollout по волнам

Ниже — минимально обязательная матрица включений для baseline rollout.

| Волна | Профиль tenant | `builder.enabled` | `preview` | `properties` | `publish` | `legacy_bridge_readonly` | Ключевые проверки |
|---|---|---:|---:|---:|---:|---:|---|
| Wave 0 (internal) | platform/synthetic | ✅ | ✅ | ✅ | ❌ | ✅ | parity payload, toggle audit trail, fallback на legacy-read |
| Wave 1 (pilot) | 1–3 low-traffic tenant | ✅ | ✅ | ✅ | ⚠️ по allowlist | ✅ | publish dry-run, RBAC parity, sanitize error-rate |
| Wave 2 (broad) | cohort tenants | ✅ | ✅ | ✅ | ✅ | ✅ (до sunset) | SLO/SLI стабильность, отсутствие regressions в routing/indexing |
| Wave 3 (stabilize) | default cohorts | ✅ (default-on) | ✅ | ✅ | ✅ | ❌ (после sunset) | post-rollout review, закрытие compatibility-debt |

Правила перехода между волнами:

1. Переход в следующую волну запрещён при незакрытых `P1` инцидентах по publish/sanitize/RBAC.
2. Для каждой волны обязателен signed-off owner list: platform on-call + pages owner + runtime owner (Next/Leptos).
3. Перед Wave 2 требуется подтверждённая regression-проверка storefront rendering для `grapesjs_v1` payload в `apps/storefront` и `apps/next-frontend`.

### Фаза 3.3 — Runbook переключений (tenant-by-tenant)

Процедура для каждого tenant выполняется как атомарная операция control-plane:

1. Снять pre-check snapshot: текущие flags, модульные permissions, состояние publish queue.
2. Включить/выключить `builder.enabled` и дочерние capability flags в одном change-set.
3. Выполнить smoke-проверки: `preview -> properties -> publish(dry)` на тестовой page.
4. Проверить observability probes: sanitize failures, publish latency, error-rate за последние 15 минут.
5. Зафиксировать post-check snapshot + решение (`keep` / `rollback`) в audit trail.

6. Получить owner sign-off по чеклисту: platform on-call, pages owner, runtime owner (Next/Leptos).

Артефакты выполнения (обязательно приложить к execution log):

- pre/post toggle snapshot;
- smoke-check протокол (`preview/properties/publish(dry)`);
- выдержка метрик за 15 минут до/после переключения;
- финальное решение `keep`/`rollback` с owner signatures.

Условия немедленного rollback:

- рост runtime error-rate выше agreed threshold;
- regression в RBAC (доступ editor/moderator/admin расходится с policy);
- publish pipeline queue backlog превышает baseline x2 в течение 10+ минут.

SLO-проверка после переключения:

- `preview` p95 < 1.5s;
- `publish` p95 < 3s;
- sanitize failures <= baseline + alert threshold.


### Фаза 3.4 — Шаблон execution log (обязательный минимум)

Чтобы owner sign-off из Phase 3.3 был проверяемым, для каждого tenant change-set фиксируется единый шаблон записи:

```text
Tenant: <tenant_id>
Wave: <0|1|2|3>
Change-set id: <control-plane operation id>
Requested by: <owner>
Approved by: <platform on-call, pages owner, runtime owner>

Flags before:
- builder.enabled=...
- builder.preview.enabled=...
- builder.properties.enabled=...
- builder.publish.enabled=...
- builder.legacy_bridge_readonly=...

Flags after:
- builder.enabled=...
- builder.preview.enabled=...
- builder.properties.enabled=...
- builder.publish.enabled=...
- builder.legacy_bridge_readonly=...

Smoke checks:
- preview: pass/fail + latency
- properties/tree: pass/fail
- publish(dry): pass/fail + duration

Observability window (15m pre/post):
- sanitize failure rate: ...
- publish p95: ...
- runtime error-rate: ...

Decision: keep|rollback
Rollback reference: <runbook link / operation id>
Notes: <known deviations or waivers>
```

Минимальные правила заполнения:

1. Запрещено оставлять пустыми `Flags before/after` и `Decision`.
2. Любой `waiver` требует явной ссылки на инцидент/тикет и срок действия waiver.
3. Для `Wave 1` и выше обязательна ссылка на storefront regression-check отчёт (`apps/storefront` + `apps/next-frontend`).

### Фаза 3.5 — Baseline-only rollout policy (freeze)

В рамках текущего трека фиксируется **strict baseline** без расширения platform-контракта:

- runtime/editor baseline: только OSS GrapesJS (self-hosted);
- transport/storage baseline: только vendor-neutral `grapesjs_v1`;
- integration policy: любые vendor-specific plugins допустимы только как локальные UI adapters без изменения backend contract;
- compatibility policy: legacy bridge остаётся read-only до sunset, без feature growth.

Запрещено в рамках Phase 3–5:

1. Добавлять новые обязательные поля в `grapesjs_v1` под конкретного вендора.
2. Вводить control-plane флаги, которые имеют смысл только для vendor-specific runtime.
3. Подменять fallback semantics для legacy-read path vendor-зависимыми правилами.

Критерий соответствия baseline-only policy:

- любой новый change-set в rollout содержит отметку `contract_impact: none|compatible`;
- при `contract_impact=compatible` приложен schema-diff и подтверждение backward-compatibility.

### Фаза 3.6 — FBA governance-профиль для `rustok-pages` (baseline)

Минимальный governance-профиль до broad rollout:

| Контур | Ответственный owner | SLA / реакция | Артефакт контроля |
|---|---|---|---|
| Control-plane toggles (`builder.*`) | Platform team | rollback decision ≤ 15 мин при P1 | execution log + audit trail |
| Page/menu runtime contract | Pages owners | hotfix triage ≤ 30 мин при publish regression | incident ticket + runbook link |
| Runtime adapters (Next/Leptos) | Runtime owners | parity regression ack ≤ 1 рабочий день | parity-check report |
| Observability & alerts | Platform + module owners | alert acknowledge ≤ 10 мин | alert timeline + postmortem |

Обязательные governance-правила:

1. Любой rollout change-set должен иметь назначенного `decision owner` и `rollback owner`.
2. Owner для control-plane и owner для runtime adapter не могут быть одним и тем же человеком в Wave 1/2.
3. Для спорных случаев приоритет решения — у platform on-call, с последующей фиксацией в post-incident review.

### Фаза 4 — Миграция legacy markdown → rt_json_v1

**Статус:** [ ] Todo

- [ ] Выполнить `--dry-run` миграции для каждого tenant.
- [ ] Сохранить отчёты `processed/succeeded/failed/skipped` по tenant.
- [ ] Подтвердить backup scope и rollback policy до production-wave.
- [ ] Провести поэтапный боевой запуск миграции по согласованному графику.

**DoD фазы:** целевые tenant-группы мигрированы, rollback протестирован процедурно.

### Фаза 5 — Release-gate: тесты, RBAC, observability

**Статус:** [ ] Todo

- [ ] Довести до CI-ready интеграционные/e2e сценарии blog/forum/pages (create/update/read/publish/moderation).
- [ ] Проверить RBAC enforcement для editor/moderator/admin на новых маршрутах и действиях.
- [ ] Зафиксировать мониторинг: sanitize-failures, error-rate, publish latency, migration metrics.
- [ ] Определить пороги алертов и регламент реакции на инциденты rollout.

**DoD фазы:** release-gate формализован и выполняется автоматически.

### Фаза 6 — Pre-production smoke и pilot rollout

**Статус:** [ ] Todo

- [ ] Smoke-проверки: create/update/read, preview/publish, index/reindex, canonical URL.
- [ ] Проверить rendering parity в storefront для migrated rich-контента.
- [ ] Запустить pilot-wave на ограниченном списке tenant.
- [ ] Зафиксировать результаты pilot и решения go/no-go.

**DoD фазы:** pilot подтверждает стабильность и прогнозируемое поведение.

### Фаза 7 — Default-on и пост-релизная стабилизация

**Статус:** [ ] Todo

- [ ] Перевести флаг в `default-on` по согласованным tenant-группам.
- [ ] Мониторить 24–72 часа ключевые SLI/SLO и ошибки санитизации.
- [ ] Выполнить post-rollout review (риски, инциденты, долги).
- [ ] Обновить связанные implementation-plan/docs по итогам rollout.

**DoD фазы:** функция включена по умолчанию, подтверждена эксплуатационная стабильность.

## 4. Зависимости и связанные документы

- `docs/modules/overview.md` — контекст по модульному составу и краткий статус готовности.
- `apps/next-admin/docs/implementation-plan.md` — интеграция admin runtime (Next.js).
- `apps/admin/docs/implementation-plan.md` — интеграция admin runtime (Leptos).
- `apps/storefront/docs/implementation-plan.md` и `apps/next-frontend/docs/implementation-plan.md` — rendering parity и rollout storefront.
- `docs/architecture/api.md` и `docs/standards/rt-json-v1.md` — контракт rich-text/page-builder payload.

## 5. Модульный фокус: почему Page Builder — центральный контур

- `blog/forum` в этом плане — rich-text consumers (`rt_json_v1`), а не владельцы визуального builder-домена.
- Визуальный builder-домен сначала живёт как отдельный FBA reference-модуль; `pages` подключается позже как consumer этого домена.
- Любой следующий phase-gate по builder (`feature flags`, `pilot`, `default-on`) считается незавершённым без явного статуса сначала по reference-модулю, затем по интеграции `pages`.

## 6. FBA reference-module policy для builder-модуля

Чтобы не продолжать реализацию “по старой схеме”, отдельный builder-модуль фиксируется как эталонный модуль перехода на FBA:

- **FBA-first delivery:** новые изменения в Page Builder сначала проектируются в терминах FBA contracts/capabilities и только затем отображаются в конкретные host/runtime реализации.
- **Explicit compatibility perimeter:** legacy (`markdown`, block-driven pages) поддерживается только как временный compatibility layer с явным sunset-планом и метриками снятия зависимости.
- **Control-plane alignment:** rollout, enable/disable, retry/compensation и health-check сценарии должны идти через стандартные lifecycle/mechanism-практики FBA, а не через ad-hoc module toggles.
- **Parity by contract, not by framework:** parity между Next/Leptos/Flutter контролируется через единый capability contract (`grapesjs_v1` + publish semantics), а не требованием UI 1:1.
- **Reference outcome:** после стабилизации этот модуль используется как шаблон для остальных FBA-миграций (content-подобные и layout-driven домены).

## 7. FBA migration blueprint на примере `rustok-pages`

Ниже фиксируется практический шаблон перевода существующего module-owned домена в FBA-модель на примере `pages` как первого consumer-а reference builder-модуля.

### 7.1 Целевая роль `rustok-pages` в FBA

- `rustok-pages` **не** владеет визуальным editor runtime как внутренней реализацией.
- `rustok-pages` владеет page/menu/visibility/publish runtime-contract и потребляет builder как capability provider.
- Вся логика включения builder-функций идёт через control-plane политики (`tenant/module/form level`) и module health signals.

### 7.2 Границы ответственности (ownership split)

- **Reference builder-модуль:** schema `grapesjs_v1`, capability endpoints (`preview/tree/properties/publish`), sanitize/validation contract, capability-health signals.
- **`rustok-pages`:** page lifecycle, publish pipeline, routing/canonical slug, channel visibility, storefront rendering guarantees.
- **Host runtimes (Next/Leptos/Flutter):** UI adapters, feature-toggle awareness, отображение ошибок contract-layer без vendor-specific forks.

### 7.3 Этапы перевода `pages` к FBA-consumer модели

1. Закрепить builder-capability boundary в `rustok-pages` docs/manifest и запретить возврат к pages-local builder ownership.
2. Добавить tenant-scoped capability toggles: `builder.preview`, `builder.publish`, `builder.properties` как часть rollout-профиля.
3. Синхронизировать observability: correlation между page publish latency и builder sanitize/validation failures.
4. Верифицировать dual-path admin integration (`native #[server]` + GraphQL fallback) без дрейфа payload contract.
5. Перевести compatibility-слой legacy blocks в sunset-режим: только read/bridge path, без расширения функционала.

### 7.4 FBA readiness checklist для `pages`

- [ ] `rustok-pages` runtime metadata явно описывает внешний builder capability-provider.
- [ ] Rollout runbook позволяет частично отключать builder-capabilities без деградации page read/publish.
- [ ] CI-gate содержит сценарии fallback на legacy-read path при недоступности capability-layer.
- [ ] Stores/admin UIs проходят parity-check по error semantics (`validation/sanitize/runtime`).
- [ ] Для legacy block-driven path утверждён tenant-by-tenant sunset график.
- [~] Для Wave 0 зафиксированы toggle snapshots (before/after) и audit trail в control-plane логах (template/rules зафиксированы, ждём фактические execution packets).


### 7.5 Ownership / approvals matrix

- **Platform team:** владеет control-plane toggles, lifecycle hooks, rollback decision.
- **Pages module owners:** владеют page/menu runtime contract и storefront read guarantees.
- **Builder reference owners:** владеют capability API/schema (`preview/tree/properties/publish`) и sanitize policy.
- **Frontend owners (Next/Leptos/Flutter):** владеют adapter parity и UX fallback semantics.

Перед переводом tenant в следующую волну требуется явное подтверждение от Platform + Pages owner.



### 7.6 FBA execution plan (next 3 iterations) для `pages` как референсного consumer-а

> Цель блока: продолжить практическую разработку `page builder` в FBA-модели и использовать `rustok-pages` как шаблон переноса legacy module-owned домена в capability-consumer режим.

**Итерация PB-FBA-1 (contract hardening + metadata parity)**

- [~] Зафиксировать в `rustok-pages` machine-readable матрицу capability fallback (`builder_off`, `preview_off`, `publish_off`) и связать её с runtime error catalog (toggle profiles + degraded modes + error-catalog binding зафиксированы в manifest/registry/runtime gate; cross-runtime parity evidence остаётся в Wave hand-off).
- [ ] Закрыть contract-parity по consumer adapters (Next/Leptos/Flutter) на уровне одинаковых error semantics, без требования UI 1:1.
- [ ] Зафиксировать anti-drift checks: `contract_version` между provider/consumer должен валидироваться в CI.

**Итерация PB-FBA-2 (operability + fallback verification)**

- [ ] Добавить обязательный fallback regression gate: отключение `builder.enabled` не ломает `list/read/menu` surfaces и не вызывает 5xx.
- [~] Привязать tenant-switch операции к control-plane audit trail (before/after snapshots + keep/rollback decision) — execution log template и обязательные артефакты уже зафиксированы в Phase 3.3/3.4, остаётся operational evidence из Wave 0.
- [ ] Вынести унифицированные SLO threshold checks в release-gate для wave-переходов (`preview p95`, `publish p95`, sanitize failure rate).

**Итерация PB-FBA-3 (pilot execution + sunset discipline)**

- [ ] Провести Wave 0 для `pages` как first consumer с evidence package по каждому toggle profile.
- [ ] Провести Wave 1 на 1–3 low-traffic tenant c формальным go/no-go протоколом.
- [ ] Зафиксировать tenant-by-tenant sunset график для legacy blocks bridge (только read/bridge, без расширения write-path).

**Definition of Done для блока 7.6:**

- `rustok-pages` подтверждён как воспроизводимый FBA migration blueprint (contract/ops/rollout), применимый для следующих content/layout модулей.
- Все wave-переходы и rollback решения подтверждены контрольными артефактами, а не только narrative-описанием.

## 8. FBA execution roadmap (продолжение разработки Page Builder)

Этот раздел фиксирует, как **продолжать разработку Page Builder уже в FBA-модели**, и как на его примере довести `pages` до production-ready consumer-профиля.

### 8.1 Builder reference module: ближайшие deliverables

1. **Capability runtime metadata**
   - зафиксировать в runtime metadata builder-модуля явный provider-profile:
     `preview/tree/properties/publish`, health probes, degradation modes;
   - добавить machine-readable версию capability-контракта для anti-drift проверок.
   - для `rustok-pages` consumer metadata baseline уже зафиксирован в `crates/rustok-pages/rustok-module.toml` (`dependencies.page_builder`, `fba.builder_consumer`, `degraded_modes`, `toggle_profiles`).
2. **Control-plane handshake**
   - закрепить единый change-set для `builder.enabled + дочерние flags` как атомарную операцию;
   - синхронизировать retry/compensation поведение lifecycle hooks с control-plane runbook.
3. **Observability-first baseline**
   - связать метрики capability-layer с page publish pipeline (`sanitize failures`, `publish latency`, `error_rate`);
   - добавить обязательный correlation-id между builder write-path и page publish events.
4. **Compatibility sunset**
   - держать legacy bridge только в read/readonly;
   - расширение legacy write-surface запрещено после Wave 0.

### 8.2 `rustok-pages` как эталон consumer-модуля (FBA)

`rustok-pages` доводится до FBA-consumer ready по четырём трекам:

1. **Provider contract explicitness**
   - pages runtime metadata явно указывает внешний builder provider;
   - docs/manifest запрещают pages-local re-ownership editor runtime.
2. **Fallback semantics**
   - при `builder.enabled=false` admin остаётся доступным в диагностическом read-only режиме;
   - storefront read-path не зависит от capability endpoint availability.
3. **Typed errors / publish gating**
   - при `builder.publish.enabled=false` publish-path возвращает typed runtime error, не 5xx;
   - list/read surfaces остаются стабильными при partial disable.
4. **Operational verification**
   - tenant switch выполняется по `before/after` snapshot + smoke + decision log;
   - rollback policy применяется без отката всего pages runtime.

### 8.3 FBA release gate для связки `builder -> pages`

Переход в Wave 1 разрешён только если одновременно выполнены условия:

- builder capability health probes стабильны и наблюдаемы;
- `pages` прошёл fallback сценарии (`builder.enabled=false`, `builder.publish.enabled=false`);
- CI содержит fallback regression checks для admin/storefront read paths;
- для pilot-tenant есть утверждённый owner on-call и rollback playbook.

### 8.4 Последовательность исполнения (Wave 0 → Wave 1)

Чтобы снять неоднозначность hand-off между командами, исполнение фиксируется как линейка обязательных шагов:

1. **Contract freeze**
   - freeze `grapesjs_v1` поля и typed error semantics;
   - зафиксировать contract version (`builder_contract_version`) в metadata provider-а и consumer-а (`pages`).
2. **Toggle semantics verification**
   - выполнить dry run по четырём профилям: `all_on`, `publish_off`, `preview_off`, `builder_off`;
   - для каждого профиля сохранить audit evidence (`before/after`, smoke output, rollback decision).
3. **Fallback CI gate**
   - включить автоматические checks fallback-поведения в CI;
   - запретить переход в Wave 1 при отсутствии свежего fallback evidence.
4. **Pilot readiness review**
   - совместный sign-off Platform + Pages + Builder owners;
   - согласование on-call и incident ownership до включения pilot-tenant.

### 8.6 Ближайший execution backlog (следующая итерация)

1. **PB-FBA-1a — contract/evidence sync**
   - обновить `docs/modules/implementation-plans-registry.md` после фиксации Wave 0 evidence packet;
   - приложить ссылки на `toggle snapshots` и `fallback gate` результаты для `all_on/publish_off/preview_off/builder_off`.
2. **PB-FBA-1b — error catalog binding**
   - [x] связать `degraded_modes` из `rustok-module.toml` с typed runtime error catalog в `rustok-pages`;
   - [x] добавить проверку anti-drift: каждый degraded mode должен иметь documented error code.
3. **PB-FBA-2a — CI fallback gate hardening**
   - довести до required-check сценарии `builder_off` и `publish_off` без 5xx на read/list surfaces;
   - зафиксировать waiver policy: временные исключения только с owner sign-off и expiry date.

Критерий завершения 8.6:

- есть machine-verifiable evidence, что toggle semantics и fallback behavior подтверждены не только документацией, но и CI + execution packet артефактами.

### 8.5 Артефакты, обязательные для Go/No-Go

Перед каждым переходом между волнами должны существовать артефакты:

- capability metadata snapshot (provider + consumer);
- rollout change-set с trace-id и audit trail;
- smoke report (`preview/properties/publish(dry)`);
- observability report (p95 preview/publish, sanitize failures, runtime error-rate);
- rollback confirmation note с указанием ответственного owner.


## 9. Практический backlog “дальше по плану” (Q2–Q3 2026)

Ниже — конкретизированный план продолжения, чтобы команды могли параллельно двигать **reference builder** и `rustok-pages` без размытых ownership-зон.

### 9.1 Итерация A — Capability stabilization (T+2 недели)

**Цель:** довести reference-модуль builder-а до стабильного provider-контракта.

- [ ] Зафиксировать `builder_contract_version` в provider metadata и добавить anti-drift проверку в CI.
- [x] Формализовать baseline typed error catalog для `preview/tree/properties/publish` (`validation/sanitize/runtime/feature-disabled`) в provider/consumer metadata и runtime gate; RBAC parity остаётся Wave evidence.
- [x] Довести health contract до machine-readable профиля (`ready/degraded/unavailable`) с причиной деградации (`capability_disabled`, `provider_unhealthy`, `sanitize_backpressure`, `publish_backlog`) и anti-drift проверкой registry ↔ manifest.
- [x] Подготовить SLO-baseline для capability endpoints по pilot-tenant классу нагрузки (`preview_p95_ms <= 1500`, `publish_p95_ms <= 3000`, `sanitize_failure_rate <= 0.01`, `runtime_error_rate <= 0.01`) и включить sync-check в synthetic Wave evidence gate; фактические tenant measurements остаются Wave hand-off evidence.

**Выход итерации:** builder-модуль имеет стабильный FBA-provider профиль, пригодный для массового consumer onboarding.

### 9.2 Итерация B — `pages` FBA-consumer hardening (T+2–4 недели)

**Цель:** на примере `pages` зафиксировать канонический migration path module-owned → FBA-consumer.

- [x] В `rustok-pages` metadata зафиксировать dependency profile на внешний builder provider (без локального ownership fallback).
- [x] Внедрить fallback-matrix для admin/storefront сценариев (`builder_off`, `publish_off`, `preview_off`) и подтвердить отсутствие 5xx в read/list.
- [x] Добавить publish gating contract: typed runtime error + UX guidance вместо аварийного падения publish flow.
- [ ] Свести observability correlation: один trace/correlation-id на путь `builder write -> pages publish -> storefront read`.

**Выход итерации:** `pages` подтверждён как эталонный FBA-consumer, пригодный как шаблон для `content`-подобных модулей.

### 9.3 Итерация C — Control-plane rollout readiness (T+4–6 недель)

**Цель:** обеспечить безопасный tenant-by-tenant rollout без redeploy.

- [ ] Автоматизировать atomic toggle change-set (`builder.enabled` + дочерние flags) через control-plane операции.
- [ ] Внедрить обязательный pre/post snapshot capture и audit trail attachment в runbook-процедуру.
- [ ] Добавить rollback trigger policy в автоматизированные проверки (error-rate, publish backlog, RBAC regression).
- [ ] Подготовить unified on-call ownership matrix для Platform / Pages / Builder owners.

**Выход итерации:** rollout-процедура воспроизводима и операционно готова к Wave 1.

### 9.4 Итерация D — Wave 1 pilot и подготовка broad rollout (T+6–8 недель)

**Цель:** валидировать production-поведение на ограниченном наборе tenant-ов.

- [ ] Провести pilot на 1–3 low-traffic tenant с полным журналом toggle evidence.
- [ ] Зафиксировать результаты parity-проверки между Next/Leptos/Flutter по capability semantics.
- [ ] Подтвердить, что legacy bridge остаётся read-only и не расширяется новыми write-path.
- [ ] Подготовить решение Go/No-Go для Wave 2 на основе SLO/SLI и incident review.

**Выход итерации:** решение о broad rollout принимается по объективным данным release-gate.

### 9.5 Definition of Ready для следующих FBA-миграций (после `pages`)

Модуль может идти по “pages-шаблону” только если:

- [ ] external capability-provider обозначен в runtime metadata;
- [ ] есть fallback contract при частичном/полном disable capability-layer;
- [ ] rollout toggle semantics поддерживают atomic change-set + rollback;
- [ ] observability связывает capability write-path и downstream runtime effects;
- [ ] legacy compatibility имеет sunset-график с tenant-level дедлайнами.

### 9.6 Ближайший execution-sprint (продолжаем по плану)

Чтобы продолжить без повторного пересогласования, ближайший спринт фиксируется как обязательный минимальный пакет:

1. **Builder provider contract freeze (Sprint checkpoint A1)**
   - [ ] зафиксировать `builder_contract_version` в provider/consumer metadata;
   - [ ] утвердить typed error semantics для `preview/properties/publish` в одном changelog-entry.
2. **Pages fallback hardening (Sprint checkpoint B1)**
   - [ ] подтвердить `builder_off` и `publish_off` сценарии без 5xx на admin list/read;
   - [ ] добавить явный UX-message для disabled publish capability.
3. **Control-plane dry run evidence (Sprint checkpoint C1)**
   - [ ] выполнить dry run профилей `all_on/publish_off/preview_off/builder_off`;
   - [ ] приложить `before/after` snapshots + rollback decision log.
4. **Wave 1 readiness packet (Sprint checkpoint D1)**
   - [ ] собрать минимальный пакет Go/No-Go (`metadata`, `smoke`, `observability`, `rollback note`);
   - [ ] провести joint review Platform + Pages + Builder owners.

**Exit criteria спринта:** есть единый evidence-пакет для перехода к pilot Wave 1 без расширения scope документа.


## 10. Критерии завершения трека (обновлённые)

Трек считается завершённым только при выполнении всех условий:

- [ ] reference builder-модуль стабилизирован как независимый FBA-provider;
- [ ] `rustok-pages` подтверждён как production-ready FBA-consumer reference;
- [ ] rollout проходит tenant-by-tenant через control-plane без redeploy и без критичных regression;
- [ ] fallback/rollback сценарии автоматизированы и покрыты CI + runbook evidence;
- [ ] шаблон migration path опубликован как обязательный baseline для следующих module migrations.


## 11. Программа приведения модулей к FBA (на базе шаблона `builder -> pages`)

Чтобы трек не ограничивался только `Page Builder`, этот документ фиксирует общий порядок перевода модулей RusTok к FBA-архитектуре с использованием `rustok-pages` как первого референсного consumer-кейса.

### 11.1 Целевой охват программы

В scope ближайшей волны входят:

- content-like модули (`blog`, `forum`, `pages`) — как приоритетная группа для capability-driven rollout;
- layout/navigation контуры (`pages/menu/routing`) — как проверка совместимости с publish/read pipeline;
- следующие домены после стабилизации Wave 1 — по readiness-критериям из раздела 9.5.

### 11.2 Единый migration pipeline для любого модуля

Каждый модуль переводится в FBA по одинаковой последовательности:

1. **Capability boundary freeze**
   - модуль фиксирует внешние provider/consumer границы в metadata/manifest;
   - запрещается скрытый возврат к module-local ownership для capability-domain.
2. **Control-plane onboarding**
   - включение capability-функций только через tenant-scoped toggle profile;
   - atomic change-set + обязательный rollback pathway.
3. **Fallback/compatibility hardening**
   - read/list пути обязаны переживать partial disable capability-layer;
   - compatibility path должен иметь sunset-срок и owner.
4. **Observability & SLO binding**
   - correlation capability write-path ↔ downstream runtime effects;
   - обязательные SLI/SLO и alert thresholds до pilot-wave.
5. **Pilot evidence & promotion**
   - модуль проходит Wave 0/Wave 1 с audit evidence;
   - переход в broad rollout только после sign-off owners.

### 11.3 Очередь модулей “дальше по плану” после `pages`

После завершения `pages` как FBA-consumer reference, следующая очередь фиксируется так:

- **Queue A (немедленно после pages):** `blog`, `forum` — доведение до полной FBA-consumer модели на том же capability/governance профиле.
- **Queue B (после Queue A):** layout-adjacent и content-index интеграции, завязанные на publish/read consistency.
- **Queue C (расширение):** остальные module-owned домены, где есть legacy toggle/ownership debt.

Для каждой очереди требуется отдельный Go/No-Go packet: metadata snapshot, fallback report, observability report, rollback note.

### 11.4 FBA governance checklist (обязателен для всех модулей)

Модуль считается “готов к FBA rollout”, только если:

- [ ] есть machine-readable runtime metadata с явным provider/consumer профилем;
- [ ] включение capability-функций выполняется только через control-plane toggle policy;
- [ ] fallback semantics документированы и покрыты CI-checks;
- [ ] rollback выполняется без полного отката соседних runtime-контуров;
- [ ] ownership матрица (Platform + Module + Frontend) утверждена до pilot;
- [ ] legacy compatibility имеет sunset milestone и tenant-level tracking.

### 11.5 Контроль актуальности документации и anti-drift

При каждом переводе модуля в FBA обязательно обновляются:

1. этот документ (статус программы и очередь модулей);
2. `docs/modules/registry.md` (актуальный maturity/state модуля);
3. implementation-plan конкретного модуля (локальные шаги и runbook);
4. release-gate evidence (CI + observability + rollback artifacts).

## 12. Ближайший execution-пакет (май–июль 2026): продолжение разработки `page builder` и перенос `pages` в FBA

Этот блок фиксирует конкретный пакет работ “что делать дальше” без повторного пересмотра всей дорожной карты.

### 12.1 Sprint 1 (до 2026-06-15): contract freeze и anti-drift

- [ ] Утвердить `builder_contract_version=v1` для reference builder provider и `rustok-pages` consumer metadata.
- [ ] Зафиксировать таблицу совместимости `provider_version -> consumer_min_version` и проверять её в CI как hard gate.
- [ ] Закрепить единый typed error contract (`validation`, `sanitize`, `rbac`, `runtime`) для `preview/tree/properties/publish`.

**Артефакты Sprint 1:**
- changelog entry по contract freeze;
- CI отчёт anti-drift check (baseline command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-contract-parity.mjs`);
- обновлённые metadata snapshots provider/consumer.

### 12.2 Sprint 2 (до 2026-06-30): `rustok-pages` fallback hardening

- [ ] Провести проверку профилей `builder_off`, `publish_off`, `preview_off` для `apps/admin` и `apps/next-admin`.
- [ ] Подтвердить, что `list/read/menu` surfaces в `pages` не дают 5xx при частичном/полном disable builder capabilities.
- [ ] Зафиксировать UX-semantic для disabled publish capability (typed error + operator guidance + trace-id).

**Артефакты Sprint 2:**
- fallback regression report (admin + storefront), включая baseline verify command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fallback-profiles.mjs`;
- incidents/alerts dry log по disable-сценариям;
- обновлённый runbook переключений tenant-by-tenant.

### 12.3 Sprint 3 (до 2026-07-15): Wave 0/Wave 1 readiness

- [ ] Автоматизировать control-plane dry-run change-set для профилей `all_on/publish_off/preview_off/builder_off`.
- [ ] Собрать обязательный Wave 1 readiness packet: metadata, smoke, observability, rollback note.
- [ ] Провести совместный Go/No-Go review: Platform + Builder owners + Pages owners.

**Артефакты Sprint 3:**
- audit trail с before/after snapshots;
- dry-run consistency verify report (baseline command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-toggle-profiles-consistency.mjs`);
- SLO отчёт (`preview p95`, `publish p95`, sanitize failure rate);
- подписанный протокол Go/No-Go для pilot tenants.
- unified baseline gate report (command: `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`).

### 12.4 Как масштабировать после `pages` (дальше по плану)

После завершения Sprint 3 модуль `pages` считается эталонным FBA-consumer кейсом, и pipeline из разделов 9–11 переносится без изменений на:

1. `blog` (Queue A) — приоритет на publish/read consistency и typed error parity.
2. `forum` (Queue A) — приоритет на moderation/publish lifecycle и fallback stability.
3. layout/index интеграции (Queue B) — приоритет на routing/canonical/index consistency при capability деградации.

Переход между модулями выполняется только при наличии полного evidence-пакета предыдущего шага (metadata + fallback + observability + rollback).

Без синхронного обновления этих артефактов модуль не переводится в следующую rollout-волну.

### 12.5 Матрица ответственности и hand-off (обязательный baseline для Sprint 1–3)

Чтобы исключить неявные блокеры между командами, для каждого sprint-checkpoint фиксируется owner-профиль:

| Checkpoint | Platform team | Builder reference owners | Pages owners | Frontend owners (Next/Leptos/Flutter) |
| --- | --- | --- | --- | --- |
| Sprint 1 / A1 | утверждают anti-drift gate и contract registry | публикуют `builder_contract_version` и typed error catalog | подтверждают `consumer_min_version` и dependency profile | подтверждают adapter mapping для typed errors |
| Sprint 2 / B1 | подтверждают toggle policy и rollback triggers | гарантируют стабильность capability health probes | верифицируют `list/read/menu` fallback без 5xx | подтверждают UX parity при `publish_off`/`builder_off` |
| Sprint 3 / C1-D1 | проводят Go/No-Go церемонию и фиксируют decision log | прикладывают provider health и SLO report | прикладывают publish/read smoke и rollback note | прикладывают parity evidence по capability semantics |

Правило hand-off: checkpoint не считается завершённым, если хотя бы один owner-блок в таблице не имеет подтверждённого артефакта в release packet.

### 12.6 Минимальный evidence packet template (для Wave 0/Wave 1)

Для унификации пакета между модулями после `pages` используется единая структура. Machine-readable baseline закреплён в `crates/rustok-page-builder/contracts/page-builder-wave-evidence-template.json` и проверяется `verify-page-builder-wave-evidence-template.mjs`; синтетический dry-run packet для `pages` лежит в `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json` и проверяется `verify-page-builder-wave-evidence-packet.mjs`, но не заменяет фактическое tenant evidence:

1. `metadata/`
   - provider snapshot (`builder_contract_version`, health profile, degraded modes);
   - consumer snapshot (`dependency profile`, fallback matrix, toggle profiles).
2. `fallback/`
   - результаты `all_on/publish_off/preview_off/builder_off`;
   - подтверждение отсутствия 5xx в `admin list/read` и `storefront read`.
3. `observability/`
   - `preview p95`, `publish p95`, sanitize failure rate, runtime error rate;
   - correlation trace examples `builder write -> pages publish -> storefront read`.
4. `rollback/`
   - rollback decision log (`keep` / `rollback`) с причиной;
   - owner on-call подтверждение и timestamp.

Минимальный стандарт: без полного packet template модуль не может перейти из Wave 0 в Wave 1. Template и gate синтетического packet должны оставаться частью aggregate baseline gate `verify-page-builder-fba-baseline.mjs`; переход в Wave 1 требует заменить синтетические snapshots фактическими before/after артефактами tenant dry-run.

### 12.7 Следующий практический шаг “прямо сейчас” (next 10 working days)

Чтобы команда могла продолжить работу без дополнительного re-planning, фиксируется минимальный стартовый пакет на ближайшие 10 рабочих дней:

1. **Contract registry update**
   - [x] создать/обновить machine-readable запись `builder_contract_version=1.0` для provider и `consumer_min_version=1.0` для `rustok-pages`;
   - [x] добавить ссылку на запись в `docs/modules/registry.md` и локальный implementation-plan `crates/rustok-pages/docs/implementation-plan.md`.
2. **Fallback smoke baseline**
   - [ ] выполнить smoke по профилям `all_on`, `publish_off`, `preview_off`, `builder_off` на одном internal tenant;
   - [ ] приложить краткий отчёт с фактами по `admin list/read`, `storefront read`, `publish(dry)`.
3. **Observability wiring check**
   - [ ] подтвердить наличие correlation-id в цепочке `builder write -> pages publish -> storefront read`;
   - [ ] зафиксировать baseline-значения `preview p95`, `publish p95`, sanitize failure rate.
4. **Go/No-Go prep draft**
   - [ ] подготовить черновик Wave 1 readiness packet по шаблону 12.6;
   - [ ] провести асинхронный review owner-ами Platform/Builder/Pages/Frontend.

**Exit criteria (next 10 working days):**
- есть валидный contract registry snapshot;
- есть fallback smoke evidence минимум для `all_on/publish_off/preview_off/builder_off`;
- есть observability baseline с correlation examples;
- есть черновик readiness packet с незакрытыми рисками и owner-назначениями.

### 12.8 Реестр рисков для Sprint 1–3 и правила эскалации

Для того чтобы “дальше по плану” не превратилось в narrative-only tracking, фиксируется обязательный risk register:

| Risk ID | Описание риска | Trigger | Mitigation | Escalation owner |
| --- | --- | --- | --- | --- |
| PB-FBA-R1 | anti-drift между provider/consumer metadata | несовместимые `builder_contract_version`/`consumer_min_version` | hard CI gate + rollback к последней совместимой паре | Platform team |
| PB-FBA-R2 | fallback regression в `pages` read surfaces | 5xx или timeout при `builder_off`/`publish_off` | блокировка Wave promotion + hotfix fallback matrix | Pages owners |
| PB-FBA-R3 | деградация capability health под pilot нагрузкой | `preview/publish` p95 выше SLO или рост sanitize failures | ограничение rollout cohort + tuning + повторный smoke | Builder reference owners |
| PB-FBA-R4 | UX drift между Next/Leptos/Flutter adapters | различающиеся typed error semantics | parity review checkpoint + единый error mapping table | Frontend owners |

**SLA эскалации:**
- критические риски (`R1`, `R2`) эскалируются в течение 30 минут с момента детекта;
- деградационные риски (`R3`, `R4`) — в течение 1 рабочего дня с обязательным remediation plan;
- без закрытого mitigation item модуль не продвигается в следующую rollout-волну.

### 12.9 Execution cadence и DoD по волнам (чтобы продолжать без повторных трактовок)

#### Еженедельный cadence (до завершения Wave 1)

- **Понедельник (plan sync, 30 мин):**
  - актуализация статуса checkpoint’ов `A1/B1/C1-D1`;
  - проверка открытых рисков `PB-FBA-R1..R4` и назначение owner/action.
- **Среда (evidence sync, 30 мин):**
  - сверка, что пакет `metadata/fallback/observability/rollback` пополняется фактическими артефактами;
  - фиксация drift-замечаний между provider/consumer metadata.
- **Пятница (promotion review, 30 мин):**
  - решение `keep/rollback/hold` по текущему tenant cohort;
  - обновление go/no-go статуса и блокеров следующей волны.

#### Definition of Done для перехода Wave 0 -> Wave 1

Переход разрешён только при одновременном выполнении:

1. **Contract integrity**
   - [x] `builder_contract_version` и `consumer_min_version` подтверждены anti-drift gate без waiver (`verify-page-builder-contract-registry.mjs`; CI aggregate gate обновлён).
2. **Fallback integrity**
   - [ ] проверены `all_on/publish_off/preview_off/builder_off` и нет 5xx в `admin list/read` + `storefront read`.
3. **Operational integrity**
   - [ ] есть complete audit trail (`before/after`, smoke, decision log) по toggle change-set.
4. **Observability integrity**
   - [ ] подтверждены SLO-границы (`preview p95`, `publish p95`, sanitize failure rate) и есть correlation trace examples.
5. **Ownership integrity**
   - [ ] есть явный sign-off Platform + Builder + Pages + Frontend owners.

Если любой пункт не закрыт, статус волны остаётся `hold`, а модуль не масштабируется на `blog/forum` очереди.

### 12.10 Что считаем “продолжили по плану” к концу июля 2026

Чтобы зафиксировать измеримый результат, к **2026-07-31** ожидается минимальный outcome:

- [ ] `pages` прошёл Wave 0 с полным evidence packet и без блокирующих `R1/R2`.
- [ ] Wave 1 readiness packet подготовлен и подписан owner-группами.
- [ ] Для `blog` и `forum` создан стартовый migration backlog по тому же шаблону (`contract/fallback/observability/rollback`).
- [ ] В `docs/modules/registry.md` отражён актуальный maturity-state по `builder/pages` треку.

Этот outcome является checkpoint для решения о переходе к broad rollout (Wave 2) в следующем плановом цикле.

## 13. Forum UI как widget-driven consumer Page Builder (phpFox-подобный сценарий)

Ниже фиксируется целевая трактовка, если forum UI собирается из page-builder “кирпичиков” (widgets/blocks), как в phpFox-подобном подходе.

### 13.1 Что меняется в роли `forum` в этом треке

- `rustok-forum` остаётся владельцем forum domain (topics/replies/moderation/policies), но UI-композиция страниц форума переходит в capability-consumer режим через builder widgets.
- Builder в этом случае выступает layout/composition layer, а не заменой forum runtime.
- `forum` не получает pages-local ownership editor runtime; он потребляет тот же reference provider-контракт (`preview/tree/properties/publish` + typed errors).

### 13.2 Минимальный widget contract для forum-builder интеграции

Для rollout без vendor-lock обязательный baseline:

1. `widget_type` (machine-readable identifier, например `forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`);
2. `data_contract_version` (версия входных данных виджета, проверяется anti-drift gate);
3. `props_schema` (валидируемый JSON schema для UI-настроек);
4. `capability_requirements` (`preview`, `publish`, `moderation_view` при необходимости);
5. `fallback_mode` (`readonly`, `hidden`, `degraded`) при частичном disable builder capabilities.

### 13.3 Границы ответственности (обязательно)

- **Forum owners:** domain data, moderation semantics, ACL/RBAC checks, query contracts.
- **Builder owners:** widget rendering host, layout tree, publish pipeline integration, typed error surfacing.
- **Frontend owners:** adapter parity (Next/Leptos/Flutter), UX fallback при недоступности отдельных widgets.
- **Platform team:** tenant toggle policy, rollout/rollback governance, observability SLO gates.

### 13.4 Ограничения rollout (чтобы не сломать forum runtime)

- Запрещено переносить domain-логику forum в widget layer; widgets только композируют и отображают уже контрактные forum capabilities.
- При `builder_off` forum read-path обязан оставаться доступным через baseline forum routes (без 5xx).
- Для Wave 1 требуется parity-check: одинаковая typed error семантика для forum widgets на Next/Leptos/Flutter.
- Любое расширение widget props должно идти через versioned `data_contract_version` и CI anti-drift check.

### 13.5 Очередь внедрения forum widgets после `pages`

- [x] **FW-1 (contract freeze):** widget catalog v1 (`topic_list/topic_detail/reply_stream`), `data_contract_version`/compatibility matrix и typed error mapping зафиксированы как machine-readable contract (manifest + REST/GraphQL catalog surface), без rollout activation до `P5`.
- [ ] **FW-2 (fallback hardening):** подтвердить `builder_off/publish_off` без деградации forum read/moderation surfaces.
- [ ] **FW-3 (pilot):** включить 1–2 low-traffic tenant с evidence packet (metadata/fallback/observability/rollback).
- [ ] **FW-4 (promotion):** расширять rollout только после owner sign-off и SLO stability 24–72h.

## 14. Актуализация порядка исполнения: “без хвостов” (single critical path)

Чтобы пройтись по плану без накопления параллельных незакрытых веток, ниже фиксируется обязательный порядок исполнения.

### 14.1 Правило приоритезации

- До закрытия `Section 12 / Sprint 1–3` новые scope-расширения (включая FW-1..FW-4 для forum widgets) не стартуют в delivery, допускаются только к��к design-ready backlog.
- Любая задача, которая не влияет на текущий wave-gate (`Wave 0 -> Wave 1`), получает статус `deferred`.
- Считаем “хвостом” любую незакрытую задачу из текущего checkpoint, если по ней нет артефакта в evidence packet.

### 14.2 Новый порядок (reordered execution queue)

1. **P0 — Contract freeze + anti-drift (обязательный старт)**
   - закрыть `builder_contract_version` + `consumer_min_version`;
   - включить CI anti-drift gate без waiver.
2. **P1 — Fallback hardening (до любых pilot шагов)**
   - подтвердить `all_on/publish_off/preview_off/builder_off` без 5xx;
   - зафиксировать typed error parity для Next/Leptos/Flutter.
3. **P2 — Control-plane operability**
   - dry-run atomic toggle change-set;
   - обязательные before/after snapshots + decision log.
4. **P3 — Observability & SLO gate**
   - зафиксировать `preview p95`, `publish p95`, sanitize failure rate;
   - подтвердить correlation-id chain `builder write -> pages publish -> storefront read`.
5. **P4 — Wave 0 execution**
   - выполнить internal wave и собрать полный evidence packet.
6. **P5 — Wave 1 readiness / Go-NoGo**
   - совместный owner sign-off Platform + Builder + Pages + Frontend;
   - только после sign-off разрешается активация forum FW-1 в delivery.

### 14.3 Явный deferred-список до закрытия P5

- FW-1/FW-2/FW-3/FW-4 по forum widgets — **deferred** (кроме уточнения контрактов в документации).
- Расширение rollout beyond pilot cohort — **deferred**.
- Broad rollout / default-on сценарии — **deferred**.

### 14.4 Критерий “без хвостов” для перехода к следующему пункту

Переход с `P(n)` на `P(n+1)` разрешён только если одновременно выполнены:

- [ ] все checklist-пункты текущего шага закрыты;
- [ ] есть связанный evidence artifact (metadata/fallback/observability/rollback);
- [ ] нет открытых critical risks (`PB-FBA-R1`, `PB-FBA-R2`);
- [ ] `next action` в registry и локальных implementation-plan синхронизирован с новым шагом.

Если хотя бы одно условие не выполнено — шаг остаётся `in_progress`, новые направления не открываются.


## 8. Продолжение разработки page builder (текущий sprint для `rustok-pages`)

Этот блок фиксирует следующий практический шаг после Phase B closure в `rustok-pages`:
перевести consumer-контур page builder в CI-проверяемый FBA baseline для Wave 0.

### 8.1 Sprint objective (PB-FBA-1)

- Закрыть typed fallback matrix для профилей `builder_off`, `preview_off`, `publish_off`.
- Зафиксировать единый error catalog (`validation`, `sanitize`, `runtime`, `feature-disabled`)
  без дрейфа между `#[server]`, GraphQL и UI adapters.
- Добавить CI fallback-gate для профилей `all_on`, `publish_off`,
  `preview_off` и `builder_off`.
- Сформировать Wave 0 evidence package: toggle snapshots + smoke output +
  observability snapshot + decision note (`keep/rollback`).

### 8.2 Delivery slices (пошаговое выполнение)

1. **Contract slice:** добавить machine-readable mapping fallback-профилей в runtime metadata
   `rustok-pages` и синхронизировать с docs модуля.
2. **Error semantics slice:** привести payload typed ошибок к одному catalog key-space
   для `preview/properties/publish` capability endpoints.
3. **Verification slice:** расширить module test gate целевыми проверками
   `all_on/publish_off/preview_off/builder_off` без деградации list/read.
4. **Operability slice:** оформить единый evidence-template для Wave 0 и привязать
   к control-plane audit trail.

### 8.3 Exit criteria для hand-off в Wave 1

- CI fallback regression checks стабильно зелёные на актуальном коммите.
- RBAC parity подтверждён для `editor/moderator/admin` в builder-related сценариях.
- Rollback toggle execution укладывается в <=10 минут без redeploy `pages` runtime.
- Legacy blocks path зафиксирован как read/bridge-only (без расширения write-surface).

### 8.4 Dependency sync note (обязательная синхронизация)

Для каждого завершённого slice обязательно обновлять одновременно:

- `crates/rustok-pages/docs/implementation-plan.md` (execution checkpoint + backlog);
- текущий документ (phase-level platform track);
- `docs/research/flutter.md` (явный статус зависимостей mobile contract scaffolding).

Несинхронные изменения считаются release-blocker для pilot-wave.

### 8.5 Execution backlog (следующие 2 недели, без расширения scope)

Статус на текущий sprint: `in_progress` (фокус только на `P0 -> P3` из раздела 14.2).

#### Week 1 — закрыть P0/P1

- [x] **PB-FBA-1A / Contract freeze:**
  - [x] зафиксировать `builder_contract_version=1.0` и `consumer_min_version=1.0` в machine-readable registry `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`;
  - [x] приложить anti-drift diff-check в baseline gate (`verify-page-builder-contract-registry.mjs`, aggregate `verify-page-builder-fba-baseline.mjs`, fail-fast при несовместимости).
- [~] **PB-FBA-1B / Fallback hardening:**
  - [x] подтвердить service-level smoke-профили `all_on/publish_off/preview_off/builder_off` без деградации `pages` read/list через `pages_builder_fallback_*` gate;
  - [x] приложить admin/storefront host-helper evidence без деградации read/list и без builder capability requirement на storefront render;
  - [x] связать `degraded_modes` с typed error catalog (`FEATURE_DISABLED`) в provider/consumer metadata, FBA registry и runtime anti-drift gate;
  - [x] закрепить Next Admin typed-error parity (`validation/sanitize/runtime/feature-disabled`) и operator guidance через static baseline gate;
  - [x] закрепить Leptos admin typed-error parity и localized operator guidance через static baseline gate;
  - [x] закрепить Flutter app-core typed-error parity и operator guidance через static baseline gate;
  - [ ] собрать device/runtime evidence packet для Flutter adapters в Wave hand-off.

#### Week 2 — закрыть P2/P3

- [~] **PB-FBA-1C / Control-plane operability:**
  - [x] зафиксировать machine-readable evidence template для metadata/control-plane/fallback/observability/rollback/approvals;
  - [x] зафиксировать синтетический Wave 0 dry-run packet для всех baseline toggle profiles как gate формы/семантики;
  - [ ] провести реальный dry-run toggle change-set (tenant internal), сохранить фактические `before/after` snapshots;
  - [ ] оформить реальный decision log (`keep|rollback`) с owner sign-off.
- [~] **PB-FBA-1D / Observability baseline:**
  - [x] зафиксировать synthetic Wave 0 baseline-метрики и SLO thresholds/evaluation (`preview p95`, `publish p95`, `sanitize failure rate`, `runtime error rate`) в evidence packet; фактические tenant-метрики остаются Wave hand-off задачей.
  - [x] приложить минимум 2 synthetic correlation trace (`builder write -> pages publish -> storefront read`) и включить проверку `trace_samples` в evidence gate; фактические trace examples остаются Wave hand-off задачей.

#### Артефакты, обязательные для checkpoint update

1. `metadata snapshot` (provider/consumer versions + fallback profile mapping): `crates/rustok-page-builder/contracts/page-builder-fba-registry.json`;
2. `fallback smoke report` (`all_on`, `publish_off`, `preview_off`, `builder_off`): service-level gate `cargo test -p rustok-pages --test page_service_kind_guard pages_builder_fallback`, admin/storefront host-helper static checks inside `verify-page-builder-pages-fallback-gate.mjs`;
3. `toggle audit log` (change-set id, before/after, decision);
4. `observability snapshot` (p95/error-rate/sanitize + минимум 2 `trace_samples`; synthetic baseline уже проверяется gate, фактические tenant traces нужны для Wave hand-off).

#### Жёсткие ограничения на период backlog

- Запрещено открывать delivery по `FW-1..FW-4` до полного закрытия `P5` (раздел 14.2).
- Любой waiver по anti-drift или fallback-check автоматически ставит статус Wave 1 readiness в `hold`; текущий PB-FBA-1A anti-drift gate должен проходить без waiver.
- Любой change в builder-contract без синхронного обновления:
  - `crates/rustok-pages/docs/implementation-plan.md`;
  - `docs/modules/registry.md`;
  - `docs/research/flutter.md`;
  считается release-blocker.
