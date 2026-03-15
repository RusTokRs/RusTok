# rustok-cache — План реализации

## Статус

`CacheModule` и `CacheService` уже реализованы (выделены из `rustok-core`, коммит `81e8bed`).
Этот документ фиксирует, что сделано, что осталось, и план доработок.

## Что реализовано

- [x] `CacheBackend` trait (в `rustok-core`)
- [x] `MokaBackend` — in-process кэш с TTL
- [x] `RedisBackend` — Redis connection-manager, get/set/del
- [x] `FallbackCacheBackend` — Redis → Moka деградация
- [x] `CacheModule` — `impl RusToKModule`, `ModuleKind::Core`
- [x] `CacheService` — tenant-aware API (namespace, TTL, invalidate)
- [x] `CacheBackendFactory` — создание backend из config

## Что осталось

### Приоритет: высокий

- [ ] Anti-stampede коалесцинг (через `tokio::sync::broadcast` или `DashMap<key, OnceLock>`)
- [ ] Circuit breaker для Redis backend
- [ ] Redis pub/sub инвалидация между инстансами

### Приоритет: средний

- [ ] `invalidate_pattern` с glob-паттернами
- [ ] Batch get/set API
- [ ] Prometheus-метрики (hit/miss/errors/degraded)
- [ ] Health check endpoint (`/metrics` + `ModuleHealth`)

### Приоритет: низкий

- [ ] Negative cache (кэширование "ключ не существует")
- [ ] Cache warming strategies
- [ ] Интеграционные тесты с реальным Redis (через `rustok-test-utils`)

## Зависимости от других crates

| Crate | Зависимость |
|-------|------------|
| `rustok-core` | `CacheBackend` trait, `ModuleRegistry`, `RusToKModule` |
| `moka` | In-process кэш |
| `redis` | Redis client (опциональный feature `redis-cache`) |

## Definition of Done

- [ ] Anti-stampede реализован и покрыт тестами
- [ ] Circuit breaker с метриками
- [ ] Redis pub/sub инвалидация работает в multi-instance сценарии
- [ ] Все метрики доступны через `/metrics`
- [ ] Health check возвращает корректный статус при недоступном Redis
- [ ] Интеграционные тесты в `tests/`
