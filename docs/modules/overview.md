# Документация по модулям RusToK

Этот документ фиксирует текущее состояние модульной архитектуры в репозитории:
- какие **обязательные Core-модули платформы** должны быть включены всегда;
- какие дополнительные доменные модули можно подключать по необходимости;
- какие остальные обязательные core-модули входят в ядро платформы.

## 1. Общая картина

RusToK — модульный монолит: модули компилируются в общий бинарник и поднимаются через `ModuleRegistry`.

Ключевой момент: в RusToK есть обязательные core-модули платформы и дополнительные optional-модули.

**Где смотреть в коде:**
- Runtime-регистрация модулей: `apps/server/src/modules/mod.rs`
- Синхронизация манифеста и runtime-регистрации: `apps/server/src/modules/manifest.rs`
- Контракт модуля и виды модулей: `crates/rustok-core/src/module.rs`
- Реестр Core/Optional: `crates/rustok-core/src/registry.rs`
- Манифест модулей: `modules.toml`

## 2. Что реально зарегистрировано в сервере

В текущей сборке в `ModuleRegistry` регистрируются:

### Обязательные Core-модули (`ModuleKind::Core`)

| Slug | Crate | Назначение |
| --- | --- | --- |
| `index` | `rustok-index` | **Core (critical)**: CQRS/read-model индексатор |
| `tenant` | `rustok-tenant` | **Core (critical)**: Tenant lifecycle и метаданные |
| `rbac` | `rustok-rbac` | **Core (critical)**: RBAC lifecycle и health |

Эти три модуля считаются **критичными для корректной работы платформы** и являются базовым contract-first минимумом для `apps/server`.

### Дополнительные доменные модули (`ModuleKind::Optional`)

| Slug | Crate | Назначение |
| --- | --- | --- |
| `content` | `rustok-content` | Базовый CMS-контент |
| `commerce` | `rustok-commerce` | e-commerce домен |
| `blog` | `rustok-blog` | Блоговая надстройка (depends_on: `content`) |
| `forum` | `rustok-forum` | Форумный модуль (depends_on: `content`) |
| `pages` | `rustok-pages` | Страницы и меню |

## 3. Остальные обязательные core-модули

Эти crate'ы относятся к обязательным core-модулям платформы:

| Crate | Статус | Примечание |
| --- | --- | --- |
| `rustok-core` | **Core (critical)** | Контракты, базовые типы и инфраструктура |
| `rustok-outbox` | **Core (critical)** | Транзакционная доставка событий (required в `modules.toml`) |
| `rustok-telemetry` | **Core (critical)** | Сквозная observability |

Итого обязательные core-модули платформы: `index`, `tenant`, `rbac`, `rustok-core`, `rustok-outbox`, `rustok-telemetry`.

Также есть дополнительные optional crate'ы (`rustok-iggy`, `rustok-iggy-connector`, `rustok-mcp`, `alloy-scripting`).

## 4. Приложения

- `apps/server` (`rustok-server`) — API-сервер и orchestration модулей.
- `apps/admin` (`rustok-admin`) — админ-панель на Leptos (CSR/WASM).
- `apps/storefront` (`rustok-storefront`) — storefront на Leptos (SSR).
- `apps/next-admin` — Next.js Admin (основной React-based admin dashboard).
- `apps/next-frontend` — Next.js Storefront (React-based storefront).
- `crates/rustok-mcp` (bin `rustok-mcp-server`) — MCP сервер/адаптер.

Принцип развития UI-стеков: пары Leptos и Next.js развиваются консистентно и параллельно —
`apps/admin` ↔ `apps/next-admin`, `apps/storefront` ↔ `apps/next-frontend`. При изменениях UI-контрактов
изменения и планы внедрения должны синхронно отражаться в обеих парах приложений.

## 5. Связанные документы

- `docs/modules/registry.md` — реестр приложений и crate'ов.
- `docs/modules/manifest.md` — манифест и правила описания модулей.
- `docs/architecture/improvement-recommendations.md` — рекомендации и roadmap архитектуры.

## 6. Что делать при изменениях модульного состава

При добавлении/удалении модульных crate'ов или их регистрации в сервере:
1. Обновить `apps/server/src/modules/mod.rs` (если меняется runtime-регистрация).
2. Обновить `modules.toml` (required/depends_on/default_enabled).
3. Обновить `docs/modules/overview.md`, `docs/modules/registry.md` и при необходимости `docs/index.md`.
4. Если затронуты UI-контракты, синхронно обновить implementation-планы и связанные docs для обоих UI-стеков:
   Leptos (`apps/admin`, `apps/storefront`) и Next.js (`apps/next-admin`, `apps/next-frontend`).

## 7. Проверка готовности к внедрению Tiptap / Page Builder (blog/forum/pages/content)

Ниже — консолидированная проверка готовности backend + admin UI контрактов для перехода на `rt_json_v1` и компонентный Page Builder (статус на текущей ветке).

### 7.1 Что уже готово (можно использовать для поэтапного запуска)

- **Единый backend-контракт rich-text уже реализован в core**:
  `markdown` + `rt_json_v1`, нормализация устаревшего alias `rt_json`, обязательная серверная sanitize/validation через `prepare_content_payload(...)` и `validate_and_sanitize_rt_json(...)`.
- **Blog/Forum/Pages сервисы принимают `content_json` + format и возвращают response с учётом формата**:
  `body_format/content_format` + `content_json` (для `markdown` остаётся текстовый fallback).
- **Есть tenant-scoped migration job** для перевода legacy markdown в `rt_json_v1`:
  `apps/server/src/bin/migrate_legacy_richtext.rs` (идемпотентность/checkpoint/retry).
- **Pages BlockService уже обеспечивает schema-first валидацию блоков и sanitize-политику для URL/embed/HTML**, что технически совместимо с UX Page Builder.
- **UI-прототипы под Tiptap/Page Builder существуют** в `crates/rustok-blog/ui/admin`:
  `RtJsonEditor`, `PageBuilder`, `ForumReplyEditor`, helper для markdown→rt_json.

### 7.2 Ключевые зазоры (что блокирует внедрение уровня production)

- **Admin/runtime интеграция UI-компонентов неполная**:
  компоненты Tiptap/Page Builder находятся в отдельном пакете, но не подключены в реальные `apps/next-admin`/`apps/admin` потоки как обязательный UX в production.
- **Forum/Blog productionization остаётся незавершённой** по собственным планам модулей:
  часть задач по RBAC/runtime hardening и operational-задач ещё в backlog.
- **Pages implementation-plan остаётся высокоуровневым** (Phase 2/3 в статусе planned), нет зафиксированного rollout-checklist именно для массового внедрения Page Builder.
- **Интеграционные тесты в blog отмечены как partial** (часть lifecycle-сценариев всё ещё не CI-ready), что повышает риск регрессий при миграции редактора.

### 7.3 Практический вывод по готовности

- **Backend-контракт и data-layer: готовность высокая** — база для Tiptap/Page Builder уже есть.
- **Готовность фронтенд-внедрения: средняя** — есть reusable-компоненты, но нужна интеграция в боевые админ-приложения.
- **Готовность production-rollout: средняя/ниже средней** — до включения фичи по умолчанию нужно закрыть backlog по hardening и расширить integration/e2e срезы.

### 7.4 Минимальный чеклист перед запуском (рекомендуемый)

1. Подключить `RtJsonEditor`/`PageBuilder`/`ForumReplyEditor` в целевой admin runtime (Next Admin/Leptos Admin) с feature-flag rollout.
2. Зафиксировать migration runbook tenant-by-tenant на основе `migrate_legacy_richtext` и rollback policy.
3. Довести до CI-ready интеграционные тесты blog/forum/pages для create/update/read в режиме `rt_json_v1`.
4. Закрыть P0/P1 hardening-задачи из implementation-plan (минимум RBAC enforcement и observability/release-gate).
5. Перед default-on включением выполнить smoke-проверку index/reindex и canonical URL поведения после миграции контента.
