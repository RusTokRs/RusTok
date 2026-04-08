# План реализации `rustok-core`

Статус: foundation crate уже служит shared contract layer; основной риск сейчас
не в отсутствии baseline, а в дрейфе ответственности и разрастании surface.

## Область работ

- удерживать `rustok-core` как минимально необходимый shared foundation layer;
- синхронизировать typed primitives, validation/security contracts и local docs;
- не допускать превращения `rustok-core` в свалку host- или domain-owned логики.

## Текущее состояние

- crate уже используется как базовая зависимость для platform и domain modules;
- shared typed contracts и foundation helpers уже являются частью live surface;
- другие модули строят свои integration contracts поверх `rustok-core`, не размазывая базовые типы по workspace;
- local docs и root `README.md` теперь должны удерживаться как часть scoped audit path.

## Этапы

### 1. Contract stability

- [x] закрепить `rustok-core` как shared foundation layer;
- [x] удерживать typed primitives и shared helpers вне host/domain buckets;
- [ ] удерживать sync между public surface, compatibility exports и module metadata.

### 2. Boundary hardening

- [ ] продолжать вычищать domain-specific logic из foundation layer;
- [ ] переносить shared primitives сюда только при реальной cross-module необходимости;
- [ ] покрывать новые foundation contracts targeted tests и compatibility checks.

### 3. Operability

- [ ] документировать изменения foundation contracts одновременно с изменением runtime surface;
- [ ] удерживать local docs и `README.md` синхронизированными;
- [ ] обновлять consumer-module docs, если меняются базовые typed contracts.

## Проверка

- `cargo xtask module validate core`
- `cargo xtask module test core`
- targeted tests для primitives, validation, security и compatibility exports

## Правила обновления

1. При изменении foundation contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении shared contracts обновлять связанные consumer docs там, где это влияет на live behavior.
