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
- [ ] **Фаза 3 — Feature flags и стратегия rollout**
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
- [ ] Вынести в отдельный runbook процедуру включения/отключения builder-capabilities tenant-by-tenant без отката всего pages runtime.

**DoD фазы:** `pages` и соседние контуры не владеют builder-доменом напрямую, а используют reference-модуль через стабильный контракт.

### Cross-plan dependency note (обязательно для hand-off)

- До завершения backend/parity шагов этой дорожной карты Flutter-команда может делать только contract-safe registry scaffolding.
- Любые изменения mobile module contracts для page-builder обязаны содержать явное уведомление о зависимостях и блокерах между:
  - `docs/research/flutter.md`;
  - текущим документом;
  - `crates/rustok-pages/docs/implementation-plan.md`.

### Фаза 3 — Feature flags и стратегия rollout

**Статус:** [ ] Todo

- [ ] Ввести флаги уровня tenant/module/form.
- [ ] Определить стратегию включения: internal → pilot → broad rollout.
- [ ] Подготовить матрицу включения/исключения по tenant и модулю.
- [ ] Согласовать операционный runbook переключений.
- [ ] Зафиксировать baseline-only rollout: OSS GrapesJS + vendor-neutral `grapesjs_v1` contract без расширения platform-контракта под вендор-специфику.
- [ ] Зафиксировать FBA governance-профиль для `rustok-pages` как reference-модуля: capability boundaries, control-plane hooks, module health contract, ownership SLA.

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

### Фаза 3.2 — Матрица rollout по волнам

### Фаза 3.3 — Runbook переключений (tenant-by-tenant)

Процедура для каждого tenant выполняется как атомарная операция control-plane:

1. Снять pre-check snapshot: текущие flags, модульные permissions, состояние publish queue.
2. Включить/выключить `builder.enabled` и дочерние capability flags в одном change-set.
3. Выполнить smoke-проверки: `preview -> properties -> publish(dry)` на тестовой page.
4. Проверить observability probes: sanitize failures, publish latency, error-rate за последние 15 минут.
5. Зафиксировать post-check snapshot + решение (`keep` / `rollback`) в audit trail.

Условия немедленного rollback:

- рост runtime error-rate выше agreed threshold;
- regression в RBAC (доступ editor/moderator/admin расходится с policy);
- publish pipeline queue backlog превышает baseline x2 в течение 10+ минут.

SLO-проверка после переключения:

- `preview` p95 < 1.5s;
- `publish` p95 < 3s;
- sanitize failures <= baseline + alert threshold.


- **Wave 0 (internal):** platform tenants + synthetic data; цель — проверить control-plane toggle semantics.
- **Wave 1 (pilot):** 1–3 tenant с low traffic; цель — проверить publish latency / sanitize failures.
- **Wave 2 (broad):** расширение на cohort tenants после прохождения release-gate Phase 5.

Go/No-Go для перехода в следующую волну:

- нет блокирующих RBAC regression;
- P95 publish latency в пределах согласованного SLO;
- sanitize failure rate не растёт относительно baseline больше порога алерта;
- есть утверждённый rollback шаг и подтверждённый owner on-call.

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
- [ ] Для Wave 0 зафиксированы toggle snapshots (before/after) и audit trail в control-plane логах.


### 7.5 Ownership / approvals matrix

- **Platform team:** владеет control-plane toggles, lifecycle hooks, rollback decision.
- **Pages module owners:** владеют page/menu runtime contract и storefront read guarantees.
- **Builder reference owners:** владеют capability API/schema (`preview/tree/properties/publish`) и sanitize policy.
- **Frontend owners (Next/Leptos/Flutter):** владеют adapter parity и UX fallback semantics.

Перед переводом tenant в следующую волну требуется явное подтверждение от Platform + Pages owner.
