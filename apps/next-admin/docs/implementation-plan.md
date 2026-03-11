# Next Admin App — Implementation Plan

## Фокус

Усилить `apps/next-admin` как primary admin UI с контрактной синхронизацией с backend и единым operational quality baseline.

## Улучшения

### Архитектурные долги

- Завершить нормализацию FSD-структуры и ограничить импортные зависимости между слоями.
- Централизовать data-access/auth integrations в `shared` для исключения копипаста по страницам.
- Упростить повторное использование виджетов между разделами админки.

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


## Готовность Blog/Forum/Pages к rich-text (Tiptap/Page Builder)

- [x] Подключена Tiptap-форма постов из `@rustok/blog-admin` к маршрутам dashboard для блога.
- [x] Добавлены отдельные маршруты для сценариев:
  - `/dashboard/blog/page-builder` для блочного конструктора `PageBuilder` (функционал страниц внутри меню блога).
  - `/dashboard/forum/reply` для `ForumReplyEditor` (`rt_json_v1`) внутри меню форума.
- [ ] Заменить placeholder ID на выбор реальных сущностей (селекторы page/topic) через live GraphQL-запросы.

## Паритет стеков (Leptos/Next.js)

- Любая feature для админки/витрины планируется, декомпозируется и трекается сразу для обеих реализаций (Leptos и Next.js) в одном цикле поставки.

### Checklist готовности фичи

- [ ] Реализовано в Leptos-варианте.
- [ ] Реализовано в Next.js-варианте.
- [ ] Контракты API/UI совпадают.
- [ ] Навигация и RBAC-поведение эквивалентны.

### Текущий статус rich-text (blog/forum/pages)

- **Админка (Leptos, `apps/admin`)**: [ ] Не начато / в процессе синхронизации с Next.js-реализацией.
- **Админка (Next.js, `apps/next-admin`)**: [~] Частично реализовано (подключены Tiptap/Page Builder маршруты, требуется завершить работу с реальными entity ID и parity-check с Leptos).
- **Витрина (Leptos SSR, `apps/storefront`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).
- **Витрина (Next.js, `apps/next-frontend`)**: [ ] Не начато (rich-text rendering parity для blog/forum/pages запланирован).

