# Документация `rustok-commerce-foundation`

`rustok-commerce-foundation` — shared support crate для split commerce family.
Он держит общие DTO, entities, ошибки и search/query helpers, не становясь
самостоятельным доменным модулем.

## Назначение

- публиковать общий foundation surface для split commerce crates;
- держать shared DTO, entities и error contracts вне umbrella-модуля;
- уменьшать дублирование между `product`, `pricing`, `inventory` и другими commerce crates.

## Зона ответственности

- shared commerce DTOs;
- shared SeaORM entities;
- единый `CommerceError` / `CommerceResult`;
- shared query/search helpers для commerce family;
- отсутствие самостоятельного transport/runtime orchestration слоя.

## Интеграция

- используется `rustok-product`, `rustok-pricing`, `rustok-inventory` и `rustok-commerce`;
- должен оставаться dependency-only support crate без собственной domain/business boundary;
- изменения shared DTO/entities должны синхронизироваться с consumer crates и umbrella docs;
- не должен поглощать логику, которая уже принадлежит устойчивому bounded context.

## Проверка

- structural verification: shared docs и consumer expectations должны оставаться синхронизированными;
- targeted compile/tests выполняются при изменении shared DTO/entities/error surface;
- любые incompatible changes требуют синхронизации consumer crates.

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [План umbrella `commerce`](../../rustok-commerce/docs/implementation-plan.md)
