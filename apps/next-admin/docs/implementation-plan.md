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


## Blog/Forum/Pages rich-text readiness (Tiptap/Page Builder)

- [x] Connected `@rustok/blog-admin` Tiptap-based rich-text post form to dashboard blog routes.
- [x] Added dedicated demo routes:
  - `/dashboard/blog/page-builder` for `PageBuilder` block composition flow.
  - `/dashboard/blog/forum-reply` for `ForumReplyEditor` (`rt_json_v1`) flow.
- [ ] Replace placeholder IDs with entity pickers (page/topic selectors) backed by live GraphQL queries.
