# План внедрения rich-text (Tiptap) и GrapesJS Page Builder

Этот документ фиксирует **отдельный план реализации** для двух связанных, но разных контуров:
- `Tiptap`/`rt_json_v1` для rich-text сценариев blog/forum;
- `GrapesJS`/`grapesjs_v1` для визуального Page Builder в pages.

## 1. Цель и критерии готовности

Цель: безопасно перевести rich-text admin UX blog/forum на `rt_json_v1` и довести `GrapesJS`-based `PageBuilder` для pages без деградации RBAC, publish-пайплайна, индексации и storefront-rendering.

Критерии завершения:
- `rt_json_v1` используется как основной rich-text формат ввода в admin для blog/forum;
- pages редактируются через `GrapesJS`-builder с каноническим body-форматом `grapesjs_v1`, а не через Tiptap-rich-text flow;
- миграция legacy markdown проведена tenant-by-tenant с подтверждённым rollback-сценарием;
- интеграционные/e2e проверки и observability release-gate пройдены;
- feature flag переведён в `default-on` после стабилизации.

## 2. Статус фаз

- [x] **Фаза 0 — Контракт и backend-baseline зафиксированы**
- [~] **Фаза 1 — Интеграция rich-text editor'ов и Page Builder в admin runtime**
- [ ] **Фаза 2 — Feature flags и стратегия rollout**
- [ ] **Фаза 3 — Миграция legacy markdown → rt_json_v1**
- [ ] **Фаза 4 — Release-gate: тесты, RBAC, observability**
- [ ] **Фаза 5 — Pre-production smoke и pilot rollout**
- [ ] **Фаза 6 — Default-on и пост-релизная стабилизация**

## 3. Фазы реализации

### Фаза 0 — Контракт и backend-baseline (выполнено)

**Статус:** [x] Done

- [x] Единый контракт rich-text/page-builder в backend: `markdown` + `rt_json_v1` + `grapesjs_v1`.
- [x] Серверная sanitize/validation для `rt_json_v1` и schema-check для `grapesjs_v1` включены в write-path.
- [x] Blog/Forum/Pages read-path возвращает `*_format` и `content_json` для rich payload.
- [x] Доступен migration job `migrate_legacy_richtext` для tenant-scoped запуска.

**Выход артефакта:** контракт готов к consumer-интеграции.

### Фаза 1 — Интеграция rich-text editor'ов и Page Builder в admin runtime

**Статус:** [~] In progress

- [x] Подключить `RtJsonEditor` в production CRUD-flow blog.
- [x] Подключить `ForumReplyEditor` в production CRUD-flow forum.
- [x] Подключить `PageBuilder` в production CRUD-flow pages.
- [ ] Зафиксировать parity-план для двух стеков: `apps/next-admin` и `apps/admin`.
- [ ] Выровнять UX-обработку validation/sanitize ошибок в формах.
- [ ] Синхронизировать milestone-dependency с Flutter registry/codegen планом (`docs/research/flutter.md`, секция anti-drift guardrail), чтобы mobile host не расходился с backend/page-builder rollout.

Текущее состояние `apps/next-admin`: production-формы blog и forum снова используют реальный Tiptap-based editor, а сериализация в write-path идёт в канонический payload `rt_json_v1` (`version` / `locale` / `doc`) без textarea-fallback в основном UX. `PageBuilder` переведён на реальный `GrapesJS` runtime, сохраняет `projectData` в body-формат `grapesjs_v1`, работает с реальным выбором страниц и оставляет legacy `blocks` как отдельную migration-compatible поверхность. Для `pages` compatibility rules теперь зафиксированы явно: `body` считается приоритетным payload для visual-builder consumer-ов, но legacy block-driven pages могут оставаться без `body`, а запись `body` не удаляет старые `blocks` автоматически.

Отдельный детальный план по `pages` как модулю ведётся в `crates/rustok-pages/docs/implementation-plan.md` в секции `Dedicated page-builder track`, чтобы rollout visual builder не смешивался ни с OAuth/app-registration, ни с rich-text задачами blog/forum.

**DoD фазы:** все целевые формы работают через компонентные редакторы, без ручного markdown-only fallback в основном UX.

### Cross-plan dependency note (обязательно для hand-off)

- До завершения backend/parity шагов этой дорожной карты Flutter-команда может делать только contract-safe registry scaffolding.
- Любые изменения mobile module contracts для page-builder обязаны содержать явное уведомление о зависимостях и блокерах между:
  - `docs/research/flutter.md`;
  - текущим документом;
  - `crates/rustok-pages/docs/implementation-plan.md`.

### Фаза 2 — Feature flags и стратегия rollout

**Статус:** [ ] Todo

- [ ] Ввести флаги уровня tenant/module/form.
- [ ] Определить стратегию включения: internal → pilot → broad rollout.
- [ ] Подготовить матрицу включения/исключения по tenant и модулю.
- [ ] Согласовать операционный runbook переключений.

**DoD фазы:** controlled rollout возможен без redeploy.

### Фаза 3 — Миграция legacy markdown → rt_json_v1

**Статус:** [ ] Todo

- [ ] Выполнить `--dry-run` миграции для каждого tenant.
- [ ] Сохранить отчёты `processed/succeeded/failed/skipped` по tenant.
- [ ] Подтвердить backup scope и rollback policy до production-wave.
- [ ] Провести поэтапный боевой запуск миграции по согласованному графику.

**DoD фазы:** целевые tenant-группы мигрированы, rollback протестирован процедурно.

### Фаза 4 — Release-gate: тесты, RBAC, observability

**Статус:** [ ] Todo

- [ ] Довести до CI-ready интеграционные/e2e сценарии blog/forum/pages (create/update/read/publish/moderation).
- [ ] Проверить RBAC enforcement для editor/moderator/admin на новых маршрутах и действиях.
- [ ] Зафиксировать мониторинг: sanitize-failures, error-rate, publish latency, migration metrics.
- [ ] Определить пороги алертов и регламент реакции на инциденты rollout.

**DoD фазы:** release-gate формализован и выполняется автоматически.

### Фаза 5 — Pre-production smoke и pilot rollout

**Статус:** [ ] Todo

- [ ] Smoke-проверки: create/update/read, preview/publish, index/reindex, canonical URL.
- [ ] Проверить rendering parity в storefront для migrated rich-контента.
- [ ] Запустить pilot-wave на ограниченном списке tenant.
- [ ] Зафиксировать результаты pilot и решения go/no-go.

**DoD фазы:** pilot подтверждает стабильность и прогнозируемое поведение.

### Фаза 6 — Default-on и пост-релизная стабилизация

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
