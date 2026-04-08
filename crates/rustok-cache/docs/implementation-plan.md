# План реализации `rustok-cache`

Статус: core cache baseline зафиксирован; модуль приведён к обязательному
manifest/doc contract.

## Область работ

- удерживать `rustok-cache` как capability-only core module без собственного UI;
- синхронизировать cache backend contract, local docs и manifest metadata;
- расширять cache semantics без размазывания backend wiring по host-слою.

## Текущее состояние

- `CacheModule` и `CacheService` уже выделены из `rustok-core`;
- модуль публикует единый cache backend contract для runtime;
- root `README.md`, local docs и `rustok-module.toml` входят в scoped audit path;
- Redis support остаётся optional feature, а in-memory/fallback path — частью базового contract.

## Этапы

### 1. Contract stability

- [x] вернуть `rustok-module.toml` в module standard path;
- [x] выровнять local docs и root README под единый contract;
- [ ] удерживать sync между backend contract и host integration tests.

### 2. Runtime hardening

- [ ] завершить anti-stampede коалесцинг;
- [ ] завершить circuit breaker для Redis backend;
- [ ] завершить Redis pub/sub invalidation между инстансами.

### 3. Operability

- [ ] довести Prometheus metrics и health semantics до production-ready слоя;
- [ ] покрыть multi-instance и real-Redis сценарии интеграционными тестами;
- [ ] документировать новые operational guarantees вместе с изменениями runtime contract.

## Проверка

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests для backend selection, fallback и health semantics

## Правила обновления

1. При изменении cache backend contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.
