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
- Довести MCP management surface (`/api/mcp/*`, GraphQL `mcp*`) до platform-grade уровня: persisted clients/tokens/policies/audit, session-start runtime binding, live binding Alloy scaffold tools к persisted draft store и persisted Alloy scaffold drafts уже есть; server-owned remote MCP transport bootstrap (`POST /api/mcp/runtime/bootstrap`) добавлен как первичный token-to-runtime-binding handshake; следующий шаг — полноценный remote stream transport и admin UI.

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
