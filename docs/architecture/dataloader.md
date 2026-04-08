# DataLoader и пакетные read-paths

Этот документ фиксирует роль DataLoader в RusToK как request-scoped механизма
для batched read-paths, в первую очередь в GraphQL.

## Назначение

DataLoader нужен для решения N+1-проблемы в UI-facing read-paths:

- GraphQL queries
- связанные batched-loader-ы в host/runtime-слое
- локальные read helpers, если они живут в request scope

DataLoader не является местом для бизнес-логики и не должен превращаться в
скрытый service layer.

## Основной принцип

DataLoader:

- собирает ключи в рамках одного request scope
- делает batched loading
- кэширует результат только на время запроса
- возвращает данные конкретному resolver/call site

Его задача — оптимизация чтения, а не ownership доменной логики.

## Где допустим DataLoader

Допустимые сценарии:

- GraphQL field resolvers
- batched lookups связанных сущностей
- locale-aware/profile-aware read paths, если batching не ломает tenant boundaries

Недопустимые сценарии:

- write-side operations
- долгоживущий shared cache между запросами
- domain orchestration
- скрытая авторизация или tenant resolution внутри loader-а

## Инварианты

Для любого DataLoader в платформе должны соблюдаться правила:

- batching не смешивает tenant boundaries
- batching не смешивает locale-контракт, если locale влияет на результат
- loader не обходит RBAC/auth assumptions host layer
- loader можно безопасно повторно вызвать в рамках одного request scope
- result mapping остаётся детерминированным и идемпотентным

## Владение

DataLoader принадлежит host/read-слою:

- module/service-слой поставляет canonical read-контракты
- GraphQL/runtime layer решает, нужен ли batching
- loader не становится source of truth для доменной модели

Если batching-логика становится сложной доменной логикой, её нужно выносить в
module-owned service/read-контракт.

## Performance-контракт

DataLoader используется там, где он:

- сокращает число запросов
- снижает повторное чтение одних и тех же связей
- делает UI-facing query-path предсказуемее

Но он не должен применяться автоматически в каждом read path без реальной
проблемы N+1.

## Что не делать

- не класть в loader бизнес-правила и orchestration
- не хранить loader-кэш дольше одного запроса
- не смешивать в одном batched-loader-е разные tenant или locale-contexts
- не использовать loader как замену нормальному module-owned read service

## Когда обновлять этот документ

Этот документ нужно обновлять, если меняется:

- роль DataLoader в GraphQL/runtime-слое
- request-scope caching-контракт
- tenant/locale batching rules
- граница между loader и module-owned read-service

## Связанные документы

- [Архитектура API](./api.md)
- [Маршрутизация и границы transport-слоя](./routing.md)
- [Архитектура модулей](./modules.md)
- [Обзор архитектуры платформы](./overview.md)
