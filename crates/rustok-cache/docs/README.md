# Документация `rustok-cache`

`rustok-cache` — core-модуль кэширования платформы. Он держит Redis lifecycle,
fallback/in-memory cache semantics и cache health contract для host runtime.

## Назначение

- публиковать канонический runtime entry type `CacheModule`;
- централизовать cache backend selection и lifecycle вне `apps/server`;
- давать платформе единый cache service contract для runtime-модулей.

## Зона ответственности

- `CacheService`, `CacheBackendOptions` и backend selection logic;
- Redis lifecycle, configurable circuit breaker settings, fallback semantics и cache health reporting;
- lightweight backend instrumentation через `CacheBackend::stats()` для hits/misses/invalidations/entries;
- service-level Prometheus gauges через `CacheService::prometheus_metrics()` для Redis configuration/health, metrics toggle и in-flight `load_or_fill` loaders;
- generic anti-stampede helper `CacheService::load_or_fill`, который коалесцирует concurrent misses по cache key и возвращает источник результата (`Hit`, `Filled`, `Coalesced`);
- generic invalidation publisher/subscriber `CacheService::publish_invalidation` / `CacheInvalidationService`, который валидирует непустые channel/key, публикует namespaced invalidation messages в Redis pub/sub при включённом backend, всегда fan-out-ит сообщение local subscribers в текущем процессе, поддерживает channel-scoped local subscriptions через `subscribe_local_channel()` и даёт host/runtime listener-ам единый `consume_subscription` adapter для Redis pub/sub без прямого Redis wiring;
- tenant-aware cache namespace и invalidation contract;
- отсутствие собственной RBAC vocabulary и UI surface.

## Интеграция

- зависит от `rustok-core`, `moka`, `tokio`, optional `redis` и shared infra;
- используется `apps/server` как platform cache capability для tenant/RBAC/runtime caches;
- остаётся `ui_classification = "capability_only"` и не публикует module-owned UI;
- доступ к admin-facing cache operations авторизуется host-слоем или owning module.

## Проверка

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests для cache backend selection, stats instrumentation, load coalescing, invalidation message validation, invalidation publishing/local fan-out, channel-scoped local subscriptions, circuit breaker options и health semantics при изменении wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)

## Listener/reconnect guarantees

`CacheInvalidationService::consume_subscription(channel, handler)` держит один Redis pub/sub stream до закрытия или ошибки и отдаёт каждое сообщение в domain handler как `CacheInvalidationMessage`. `CacheInvalidationMessage::try_new()` фиксирует валидируемый constructor для call sites, которые хотят fail-fast до publish; `publish()` дополнительно отбрасывает пустой channel/key без local/Redis fan-out. Retry/backoff остаются за host/runtime listener-ом, чтобы каждый домен мог публиковать собственный health status и reconnect telemetry; `apps/server` tenant listener использует этот adapter внутри существующего retry-loop. В non-Redis сборке subscription adapter недоступен, full local fan-out через `subscribe_local()` остаётся baseline contract для single-instance runtime и тестов, а `subscribe_local_channel(channel)` даёт namespace-filtered receiver для multi-listener сценариев без Redis.
