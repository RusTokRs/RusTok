---
id: doc://docs/guides/error-handling.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Error Handling Guide

Полное руководство по обработке ошибок в RusToK находится в [`docs/standards/errors.md`](../standards/errors.md).

## Краткое резюме

RusToK использует унифицированный тип `RichError` (RFC 7807 compatible) с категориями:

| HTTP | Категория | Когда использовать |
|------|-----------|-------------------|
| 400 | `Validation` | Ошибки валидации входных данных |
| 401 | `Unauthenticated` | Требуется аутентификация |
| 403 | `Forbidden` | Нет прав доступа |
| 404 | `NotFound` | Ресурс не найден |
| 409 | `Conflict` | Дублирование или race condition |
| 429 | `RateLimited` | Превышен лимит запросов |
| 500 | `Internal` | Неожиданная ошибка |
| 502/503 | `ExternalService` | Ошибка внешнего сервиса |
| 504 | `Timeout` | Таймаут запроса |

## Правила

1. Все функции, которые могут упасть, возвращают `Result<T, RusToKError>`.
2. Использование `.unwrap()` / `.expect()` запрещено (кроме тестов).
3. Внутренние ошибки не раскрываются клиенту — только `user_message`.
4. Для трассировки используется `request_id` из контекста запроса.

## Полная документация

→ [`docs/standards/errors.md`](../standards/errors.md)
