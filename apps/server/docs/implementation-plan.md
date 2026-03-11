# Server App — Implementation Plan

## Фокус

Укрепить `apps/server` как центральный backend runtime с формальными API-контрактами, предсказуемой операционной диагностикой и усиленными security-гейтами.

## Улучшения

### Архитектурные долги

- Сократить связность между HTTP/GraphQL слоями и модульной бизнес-логикой через более строгие service boundaries.
- Довести до единообразия lifecycle модулей (bootstrap, readiness, graceful shutdown).
- Уменьшить дублирование конфигурации transport/auth по подсистемам.

### API/UI контракты

- Финализировать единый контракт ошибок для REST и GraphQL (коды, machine-readable fields, correlation id).
- Стабилизировать контракты tenant-aware headers и auth claims для всех frontend-клиентов.
- Расширить версионирование публичных API-изменений через changelog/contract notes.

### Observability

- Выровнять покрытие метрик по всем critical endpoints и фоновой обработке событий.
- Добавить end-to-end tracing: gateway -> handlers -> modules -> outbox/transport.
- Сформировать SLO-дашборды по latency/error budget и health per module.

### Security

- Усилить RBAC enforcement checks на уровне middleware и service layer.
- Ввести регулярный security-review для sensitive endpoints (auth, tenant, admin operations).
- Расширить аудит событий безопасности (login, privilege changes, tenant boundary violations).

### Test coverage

- Увеличить долю интеграционных тестов для модульных сценариев с реальной БД/миграциями.
- Добавить contract-тесты на стабильность API ответов для фронтендов.
- Включить негативные тесты по RBAC/tenant isolation и failure-mode тесты для event transport.
