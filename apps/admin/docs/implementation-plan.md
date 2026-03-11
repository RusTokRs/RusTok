# Admin App (Leptos) — Implementation Plan

## Фокус

Довести `apps/admin` до устойчивого production-уровня как Rust/Leptos админку с сильными UI/API контрактами и наблюдаемостью клиентских сценариев.

## Улучшения

### Архитектурные долги

- Завершить консолидацию FSD-структуры с минимизацией cross-layer импортов.
- Устранить дубли бизнес-логики между widgets/features и shared-integration слоем.
- Сформировать единый набор UI primitives и policy повторного использования.

### API/UI контракты

- Зафиксировать контракт GraphQL-операций и типизацию ошибок в пользовательских формах.
- Синхронизировать UI-поведение с `apps/next-admin` (loading/error/empty states).
- Стандартизировать контракт локализации для всех новых экранов и системных сообщений.

### Observability

- Добавить клиентские метрики UX-потоков (critical actions, failures, latency).
- Пробрасывать correlation id в запросах для связки с backend traces.
- Описать frontend incident checklist для деградации API и auth flows.

### Security

- Ввести централизованную проверку permission guards на route и action уровне.
- Защитить клиентские формы от небезопасных payload-мутаций и XSS-вставок в rich fields.
- Расширить контроль за токенами с явным policy хранения/обновления сессии.

### Test coverage

- Увеличить покрытие unit/component тестов для shared UI и критичных форм.
- Добавить e2e smoke-сценарии для core admin workflows.
- Ввести contract checks для i18n ключей и API-type drift.
