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
- generic anti-stampede helper `CacheService::load_or_fill`, который коалесцирует concurrent misses по cache key и возвращает источник результата (`Hit`, `Filled`, `Coalesced`);
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
- targeted runtime tests для cache backend selection, stats instrumentation, load coalescing, circuit breaker options и health semantics при изменении wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
