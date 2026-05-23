# Next Admin App — Implementation Plan

## Фокус

Усилить `apps/next-admin` как primary admin UI с контрактной синхронизацией с backend и единым operational quality baseline.

## Улучшения

### Архитектурные долги

- Завершить нормализацию FSD-структуры и ограничить импортные зависимости между слоями.
- Централизовать data-access/auth integrations в `shared` для исключения копипаста по страницам.
- Упростить повторное использование виджетов между разделами админки.
- Удалить legacy import paths после прохождения type-check/build, чтобы canonical `shared/*`, `entities/*`, `widgets/*` стали единственным живым API.

### API/UI контракты

- Выровнять контракты GraphQL/REST ответов с `apps/server` для критичных admin сценариев.
- Зафиксировать единые UX-паттерны для таблиц, форм, уведомлений, optimistic updates.
- Синхронизировать RBAC-навигацию и action-level permissions с backend policy.

### Observability

- Добавить клиентские telemetry events для critical admin flows.
- Пробросить trace/correlation идентификаторы в backend вызовы.
- Определить SLI для UX: время загрузки экрана, успешность submit, частота recoverable ошибок.

### Security

- Усилить защиту клиентских маршрутов/действий через RBAC guards и fail-closed поведение.
- Добавить secure handling токенов/сессий и аудит чувствительных операций.
- Проверить CSP/XSS/CSRF меры для административных форм и rich content inputs.

### Test coverage

- Расширить e2e покрытие критических разделов (auth, users, content, settings).
- Добавить contract-тесты API маппинга и проверки typed clients.
- Увеличить unit/component coverage для shared UI и form logic.
- Держать `pnpm --filter next-admin type-check` и `pnpm --filter next-admin build` в зелёном baseline после каждого изменения FSD/UI структуры.

## Готовность Blog/Forum к rich-text (Tiptap) и Pages к GrapesJS Builder

- [x] Production-форма постов использует реальный Tiptap-based `RtJsonEditor` и сериализует rich-text в канонический `rt_json_v1`.
- [x] Добавлены отдельные маршруты для сценариев:
  - `/dashboard/blog/page-builder` для визуального `GrapesJS`-конструктора `PageBuilder` (функционал страниц внутри меню блога).
  - `/dashboard/forum/reply` для `ForumReplyEditor` (`rt_json_v1`) внутри меню форума.
- [x] `ForumReplyEditor` использует тот же Tiptap-based `RtJsonEditor` и тот же контракт `rt_json_v1`, что и production CRUD-flow блога.
- [x] Placeholder ID заменены на выбор реальных сущностей (селекторы page/topic) через live GraphQL-запросы.
- [x] `PageBuilder` сохраняет pages в канонический body-формат `grapesjs_v1`; legacy `blocks` остаются read-compatible до отдельного storefront migration slice.

## Паритет стеков (Leptos/Next.js)

- Любая feature для админки/витрины планируется, декомпозируется и трекается сразу для обеих реализаций (Leptos и Next.js) в одном цикле поставки.

### Capability-first parity contract (Phase 1, 2026-05-23)

Must-have parity между `apps/next-admin` и `apps/admin`:

- единый backend payload contract `grapesjs_v1` (`body.format`, `body.contentJson`);
- capability surfaces `preview/tree/properties/publish` на обоих host-стэках;
- publish/write actions и compatibility-правила для legacy `blocks/body` не расходятся;
- единый UX-паттерн ошибок write-path (`validation/sanitize/runtime`) для rich/page-builder форм.

Host-specific UX, который допустим без drift:

- разные визуальные компоненты, layout и взаимодействие внутри capability surface;
- разная глубина визуального tree/preview при неизменном payload contract;
- разная компоновка route-shell, если RBAC/navigation semantics совпадают.

### Checklist готовности фичи

- [x] Реализовано в Leptos-варианте.
- [x] Реализовано в Next.js-варианте.
- [x] Контракты API/UI совпадают на capability-уровне.
- [x] Навигация и RBAC-поведение эквивалентны для `pages` write/publish surfaces.

## FSD/UI follow-up backlog

- Вычистить compatibility imports из `components/`, `lib/`, `hooks/` и перевести потребителей на canonical FSD-layer paths.
- Выровнять widget/shared boundaries для таблиц, form shells и app-shell композиций.
- Довести parity-check с `apps/admin` по loading/error/permission-gated UX и navigation contract.
- Удерживать `@iu/*` и `UI/docs/api-contracts.md` как source of truth для cross-stack UI API.

### Текущий статус rich-text/blog-forum и GrapesJS pages

- **Админка (Leptos, `apps/admin`)**: [~] Частично реализовано (`pages` получил capability surfaces `preview/tree/properties/publish`, rich/page-builder write-path ошибки выровнены в общий паттерн).
- **Админка (Next.js, `apps/next-admin`)**: [~] Частично реализовано (production blog/forum уже используют реальный Tiptap-based editor и канонический `rt_json_v1`, pages переведены на `GrapesJS` + `grapesjs_v1`, forum flow использует live entity selection, остаётся parity-check со storefront rendering slice).
- **Витрина (Leptos SSR, `apps/storefront`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
- **Витрина (Next.js, `apps/next-frontend`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
