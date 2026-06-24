# План реализации `rustok-cache`

Статус: core cache baseline зафиксирован; модуль приведён к обязательному
manifest/doc contract.

## Execution checkpoint

- Current phase: runtime_hardening
- Last checkpoint: Invalidation contract усилен validation guardrails, channel-scoped local subscriptions, service-level counters и source-level real-Redis integration сценариями; Redis subscription adapter теперь reject-ит пустые channel до подключения и отбрасывает invalid payloads без вызова handler-а. Tenant anti-stampede path уже переведён на `CacheService::load_or_fill`, а cache capability экспортирует service-level Prometheus gauges для Redis health/configuration, metrics toggle, in-flight loaders и invalidation counters.
- Next step: Добавить compile/test evidence при снятии ограничения на компиляции и прогнать ignored real-Redis сценарий с `RUSTOK_CACHE_REAL_REDIS_URL` поверх channel-scoped subscription contract.
- Open blockers: Compile/test evidence отложен по явному ограничению итерации: без компиляций.
- Hand-off notes for next agent: Проверить `cargo test -p rustok-cache --lib` при разрешённых компиляциях; затем прогнать `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture` для real-Redis pub/sub evidence.
- Last updated at (UTC): 2026-06-24T00:00:00Z

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
- [x] добавить validation guardrails для invalidation messages и channel-scoped local subscription parity;

### 3. Operability

- [x] довести Prometheus metrics export до production-ready service-level слоя, включая invalidation publish/rejection counters;
- [x] добавить baseline hit/miss/invalidation/entry stats и health diagnostics в cache factory contract;
- [x] добавить source-level ignored real-Redis pub/sub integration сценарий для publish/subscription parity;
- [x] документировать publisher/local fan-out guarantees для generic invalidation contract;
- [x] документировать listener/reconnect guarantees после выноса subscription adapter в `rustok-cache`.

## Проверка

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests для backend selection, fallback, `load_or_fill` coalescing, invalidation validation/channel filtering, invalidation counters, Prometheus formatting и health semantics

## Правила обновления

1. При изменении cache backend contract сначала обновлять этот файл.
2. При изменении public/runtime contract синхронизировать `README.md` и `docs/README.md`.
3. При изменении module metadata синхронизировать `rustok-module.toml`.


## Quality backlog

- [x] Актуализировать покрытие тестами по ключевым сценариям модуля: добавлены source-level тесты для invalidation validation guardrails, channel-scoped local subscription parity, invalidation counters и Prometheus counter formatting без запуска компиляции.
- [ ] Добавить compile/test evidence для нового invalidation coverage и ignored real-Redis сценария при снятии ограничения на компиляции.
- [x] Проверить полноту и актуальность `README.md` и локальных docs: listener contract синхронизирован с Redis subscription validation и invalid payload rejection.
- [x] Зафиксировать/обновить verification gates для текущего состояния модуля: добавлен explicit ignored gate для real Redis pub/sub (`RUSTOK_CACHE_REAL_REDIS_URL=... cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture`).
