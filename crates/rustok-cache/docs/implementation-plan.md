# План реализации `rustok-cache`

Статус: core cache baseline зафиксирован; модуль приведён к обязательному
manifest/doc contract.

## Execution checkpoint

- Current phase: runtime_hardening
- Last checkpoint: Tenant anti-stampede path переведён на `CacheService::load_or_fill`; cache capability теперь экспортирует service-level Prometheus gauges для Redis health/configuration, metrics toggle и in-flight loaders.
- Next step: Добавить compile/test evidence при снятии ограничения на компиляции и расширить real-Redis/multi-instance интеграционные сценарии.
- Open blockers: Compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Проверить `cargo test -p rustok-cache --lib` при разрешённых компиляциях; затем продолжить anti-stampede helper и Redis pub/sub generalization.
- Last updated at (UTC): 2026-06-20T12:00:00Z

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
- [x] удерживать sync между backend contract и host integration tests через instrumented `CacheBackend::stats()` contract и documented verification debt.

### 2. Runtime hardening

- [x] завершить anti-stampede коалесцинг;
- [x] завершить circuit breaker для Redis backend на уровне cache factory options;
- [x] добавить generic Redis pub/sub invalidation publisher и local fan-out contract;
- [x] завершить generic subscription/listener adapter для Redis pub/sub invalidation между инстансами;

### 3. Operability

- [x] довести Prometheus metrics export до production-ready service-level слоя;
- [x] добавить baseline hit/miss/invalidation/entry stats и health diagnostics в cache factory contract;
- [ ] покрыть multi-instance и real-Redis сценарии интеграционными тестами;
- [x] документировать publisher/local fan-out guarantees для generic invalidation contract;
- [x] документировать listener/reconnect guarantees после выноса subscription adapter в `rustok-cache`.

## Проверка

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests для backend selection, fallback, `load_or_fill` coalescing, Prometheus formatting и health semantics

## Правила обновления

1. При изменении cache backend contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [ ] Актуализировать покрытие тестами по ключевым сценариям модуля.
- [ ] Проверить полноту и актуальность `README.md` и локальных docs.
- [ ] Зафиксировать/обновить verification gates для текущего состояния модуля.
