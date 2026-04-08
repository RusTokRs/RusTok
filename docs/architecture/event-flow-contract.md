# Контракт потока доменных событий

Этот документ фиксирует канонический путь `DomainEvent` в RusToK: от доменной
операции до обновления downstream read-side состояния.

## Канонический путь

1. Domain/service layer выполняет бизнес-операцию.
2. Изменение write-side состояния и запись в outbox происходят в одной транзакции.
3. `rustok-outbox` доставляет событие в transport/runtime layer.
4. Зарегистрированные consumers обновляют projections, индексы и другие
   downstream surfaces идемпотентно.
5. UI и API читают уже согласованное read-side состояние.

## Источники истины

- canonical event contracts живут в `rustok-events`
- compatibility re-export может существовать в `rustok-core`, но не должен
  подменять ownership
- transactional delivery contract живёт в `rustok-outbox`
- consumer-specific semantics должны быть отражены в local docs publisher-а и
  consumer-а

## Роли компонентов

### Публикатор

Publisher:

- владеет semantic meaning события
- определяет обязательные поля payload-а
- публикует событие через canonical write path

Publisher не должен считать event bus своим read-model API.

### Outbox/runtime-слой

`rustok-outbox` отвечает за:

- transactional persistence
- retry/backoff
- delivery bookkeeping
- предсказуемый runtime contract для consumers

`rustok-outbox` остаётся `Core` module, а не support utility.

### Консьюмер

Consumer:

- должен быть идемпотентным
- должен пересчитывать своё состояние из source of truth, а не из локальных
  предположений
- не должен ломать write-side contract publisher-а

## Content и orchestration-события

Нужно различать:

- storage-owner domain events конкретного модуля
- orchestration/canonical-routing events
- helper/reindex events для legacy или shared paths

Новые сценарии должны опираться на typed storage-owner или orchestration events,
а не расширять бесконечно shared helper surface.

## Commerce-события

Для commerce family действует тот же принцип:

- ownership события у конкретного domain/service layer
- projections и index updates идут через consumer path
- transport/runtime не подменяет ownership домена

## Retry и устойчивость

Для event flow обязательны:

- конечный и наблюдаемый retry
- backoff
- идемпотентные consumer operations
- replay-safe поведение

Если consumer не идемпотентен, он не соответствует platform event contract.

## Что не делать

- не публиковать межмодульные события мимо outbox, если нужна транзакционная
  согласованность
- не считать event payload единственным долгоживущим storage format
- не переносить canonical ownership событий в host layer
- не строить новый consumer path без обновления local docs и central contract

## Когда обновлять этот документ

Этот central contract нужно обновлять, если меняется:

- ownership event family
- canonical publisher path
- consumer class
- retry/runtime semantics
- роль `rustok-events` или `rustok-outbox`

При этом сначала обновляются local docs publisher-а и consumer-а, потом central
docs.

## Связанные документы

- [Архитектура модулей](./modules.md)
- [Каналы и real-time surfaces](./channels.md)
- [Диаграммы платформы](./diagram.md)
- [Реестр crate-ов модульной платформы](../modules/crates-registry.md)
