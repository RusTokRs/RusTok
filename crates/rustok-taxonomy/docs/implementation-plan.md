# План реализации `rustok-taxonomy`

Статус: shared dictionary baseline уже работает; модуль используется несколькими
доменами и удерживается как vocabulary layer без захвата attachment ownership.

## Область работ

- удерживать `rustok-taxonomy` как shared vocabulary module;
- синхронизировать dictionary contracts, scope rules и local docs;
- не допускать превращения taxonomy в shared product storage.

## Текущее состояние

- term dictionary, translations и aliases уже реализованы как module-owned storage;
- term identity остаётся tenant-scoped и locale-independent;
- blog, forum, product и profiles уже используют taxonomy-backed relations через собственные attachment tables;
- locale normalization и fallback уже опираются на shared content contract.

## Этапы

### 1. Contract stability

- [x] зафиксировать dictionary baseline для `kind = tag`;
- [x] удерживать scope model `global | module`;
- [x] внедрить taxonomy-backed relations в первые consumer modules;
- [ ] удерживать sync между dictionary contracts, consumer integrations и module metadata.

### 2. Expansion

- [ ] расширять kinds и lookup semantics только при наличии реального domain pressure;
- [ ] добавлять новых consumer modules только через explicit module-owned attachment tables;
- [ ] удерживать alias/slug uniqueness и locale fallback guarantees покрытыми targeted tests.

### 3. Operability

- [ ] документировать новые dictionary guarantees одновременно с изменением runtime surface;
- [ ] развивать runbooks для dictionary drift и integration incidents по мере появления pressure;
- [ ] синхронизировать local docs, README и central references при изменении module role.

## Проверка

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- targeted tests для CRUD, alias lookup, scope restrictions и consumer-module sync

## Правила обновления

1. При изменении taxonomy contract сначала обновлять этот файл.
2. При изменении public/runtime surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
4. При изменении consumer-module integration rules обновлять связанные docs у owning modules.
