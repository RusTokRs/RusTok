# План реализации `rustok-commerce-foundation`

Статус: support crate уже служит shared substrate для split commerce family;
ключевая задача — удерживать его минимальным и не допускать повторной сборки
монолита в foundation-слое.

## Область работ

- удерживать `rustok-commerce-foundation` как dependency-only support crate;
- синхронизировать shared DTO/entities/error contracts и local docs;
- не допускать переноса domain/runtime logic из split commerce modules в foundation layer.

## Текущее состояние

- crate уже содержит shared DTOs, entities, errors и search/query helpers;
- consumer modules уже используют его как общий слой переиспользования;
- umbrella `rustok-commerce` опирается на этот crate для общих контрактов split family;
- самостоятельного transport/runtime surface у crate нет и не должно появляться.

## Этапы

### 1. Contract stability

- [x] закрепить foundation crate как общий dependency layer для commerce family;
- [x] удерживать shared error/entity/DTO surface единым для consumer crates;
- [ ] удерживать sync между foundation contracts, consumer crates и local docs.

### 2. Boundary hardening

- [ ] переносить сюда только действительно shared contracts;
- [ ] не втягивать сюда domain-owned services и orchestration logic;
- [ ] покрывать incompatible changes targeted compile/tests в consumer crates.

### 3. Operability

- [ ] документировать изменения foundation surface одновременно с изменением consumer expectations;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять umbrella commerce docs при изменении split-family contracts.

## Проверка

- structural verification для docs и shared boundary;
- targeted compile/tests при изменении DTO/entity/error surface;
- consumer sync across split commerce crates.

## Правила обновления

1. При изменении shared commerce foundation contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении consumer expectations обновлять связанные docs в split commerce crates.
