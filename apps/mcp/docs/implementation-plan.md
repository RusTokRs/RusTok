# MCP App — Implementation Plan

## Цели

Стабилизировать `apps/mcp` как production-ready MCP runtime для RusToK с предсказуемыми API-контрактами, наблюдаемостью и безопасностью.

## Приоритетный backlog

### 1. Архитектурные долги

- Зафиксировать границы ответственности: `apps/mcp` (bootstrap/runtime) vs `crates/rustok-mcp` (бизнес-адаптеры).
- Добавить явную схему конфигурации (env + config file) с валидацией обязательных параметров.
- Ввести профиль запуска (dev/staging/prod) с разными policy по timeouts/retries.

### 2. API/контракты MCP

- Описать и версионировать список MCP tools/resources/prompts, включая стабильные идентификаторы и payload schema.
- Ввести контракт совместимости: policy deprecation и migration notes при изменении tool signatures.
- Синхронизировать контракты с возможностями `apps/server` (tenant-aware доступ и авторизация).

### 3. Observability

- Добавить метрики по категориям MCP-вызовов: latency, error rate, timeout rate, tool usage.
- Пробросить trace context между MCP runtime и backend API вызовами.
- Описать операционный runbook по деградации внешних интеграций.

### 4. Security

- Ввести обязательную проверку входных payload по whitelist-схемам.
- Добавить ограничение прав MCP-клиента (scope-based access) и аудит критических операций.
- Реализовать защиту от resource exhaustion: лимиты размера payload, concurrency и rate limit.

### 5. Test coverage

- Добавить contract-тесты для MCP tools/resources (валидные/невалидные payload).
- Добавить интеграционные тесты для сценариев `MCP -> server API -> response mapping`.
- Ввести smoke-набор для CI: запуск минимального MCP runtime и проверка core tools.

## Критерии готовности (DoD)

- Документированные и версионированные MCP-контракты.
- Базовые метрики/трейсы доступны в observability stack.
- Security controls (валидация, scope, лимиты) включены по умолчанию.
- Тестовое покрытие ключевых MCP-сценариев не ниже согласованного baseline.
