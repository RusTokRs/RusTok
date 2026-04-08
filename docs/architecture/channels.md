# Каналы и real-time-поверхности

Этот документ фиксирует роль real-time-каналов в RusToK и их место в общей
transport-архитектуре.

## Назначение

Real-time-каналы используются там, где нужен push-формат доставки событий в
долгоживущем соединении:

- streaming статуса build/runtime операций
- live progress для длительных задач
- будущие notification/event streams, если они требуют push-доставки

Каналы не заменяют GraphQL, REST или event bus. Это отдельный transport surface
для live delivery.

## Текущий baseline

На текущем слое real-time-каналы строятся поверх WebSocket-routing в
`apps/server`.

Канонические правила:

- websocket route живёт в host layer
- payload-контракт должен быть типизирован и документирован
- auth/tenant/RBAC policy применяется до выдачи канала или в handshake path
- канал не должен становиться источником правды для доменного состояния

## Где проходит граница

### Host-слой

`apps/server` отвечает за:

- websocket handshake
- connection lifecycle
- auth/session validation
- tenant context
- fan-out transport и shutdown behavior

### Module / service-слой

Модуль или runtime service отвечает за:

- генерацию typed-событий
- публикацию в hub/broadcast layer
- семантический контракт payload-а

### Центральный event-flow

WebSocket-канал не должен подменять event-runtime:

- доменные события идут через `rustok-outbox` и `rustok-events`
- read-side и projections обновляются через event flow
- websocket нужен только для live delivery текущего статуса или прогресса

## Build/event-streaming

Build/runtime progress-канал остаётся допустимым сценарием, если:

- есть typed event-контракт
- payload сериализуется стабильно
- reconnect и lag не ломают семантический контракт
- клиент может восстановить состояние через canonical API, если пропустил события

Это важно: WebSocket-stream не должен быть единственным источником состояния.

## Wire-контракт

Для WebSocket payload действует такой минимум:

- явный `type`
- стабильный machine-readable payload
- минимальный набор обязательных полей для consumer-а
- совместимость с tracing/observability

Если канал становится долговременным платформенным контрактом, его payload должен быть
описан и в local docs owning component.

## Shutdown и отказоустойчивость

Канал должен корректно переживать:

- закрытие клиентом
- graceful shutdown хоста
- lag/backpressure
- временную недоступность publisher-а

Отказ transport-канала не должен ломать write-side-операцию, если она уже
завершена и подтверждена canonical API/state.

## Что не делать

- не использовать websocket как единственный источник доменного состояния
- не обходить auth/tenant/RBAC policy ради convenience
- не публиковать ad-hoc JSON без typed-контракта
- не переносить ownership доменных событий из event runtime в websocket hub

## Связанные документы

- [Контракт потока доменных событий](./event-flow-contract.md)
- [Архитектура API](./api.md)
- [Маршрутизация и границы transport-слоя](./routing.md)
- [Обзор архитектуры платформы](./overview.md)
