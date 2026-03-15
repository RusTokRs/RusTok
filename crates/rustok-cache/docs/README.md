# rustok-cache — Документация

## Обзор

`rustok-cache` — инфраструктурный crate для кэширования, выделенный из `rustok-core`.

Содержит Redis connection lifecycle, backend-factory и `CacheModule` (`ModuleKind::Core`).

## Архитектура

### Уровни кэширования

```
apps/server
  └─ регистрирует CacheModule через ModuleRegistry

CacheModule (ModuleKind::Core)
  └─ создаёт CacheBackend через CacheBackendFactory
  └─ публикует в AppContext.shared_store

CacheService (tenant-aware API)
  └─ namespace: {tenant_id}:{key}
  └─ TTL management
  └─ batch invalidation

CacheBackend (trait из rustok-core)
  ├─ MokaBackend — in-process, anti-stampede, negative cache
  ├─ RedisBackend — distributed, pub/sub invalidation
  └─ FallbackBackend — Redis → Moka при деградации
```

### Anti-stampede коалесцинг

При одновременных промахах (`cache miss`) на один ключ только один запрос уходит в источник данных. Остальные ждут результата через общий `tokio::sync::broadcast`.

### Circuit breaker

Обёртка поверх Redis-backend. При N последовательных ошибках переключается в `fallback mode` (Moka). Автоматически переключается обратно после `probe_interval`.

### Redis pub/sub инвалидация

При `invalidate(key)` публикуется сообщение в канал `rustok:cache:invalidate:{tenant_id}`.
Все инстансы подписаны и удаляют ключ из локального Moka.

## Конфигурация

```yaml
# config/development.yaml
rustok:
  cache:
    backend: fallback  # moka | redis | fallback
    moka:
      max_capacity: 10000
      ttl_seconds: 300
    redis:
      url: "redis://127.0.0.1:6379"
      pool_size: 10
      connection_timeout_ms: 500
    circuit_breaker:
      failure_threshold: 5
      probe_interval_seconds: 30
```

## Метрики

| Метрика | Описание |
|---------|----------|
| `rustok_cache_hits_total` | Количество попаданий в кэш |
| `rustok_cache_misses_total` | Количество промахов |
| `rustok_cache_errors_total` | Ошибки обращений к backend |
| `rustok_cache_degraded` | Gauge: 1 если Redis недоступен (fallback mode) |
| `rustok_cache_stampede_coalesced_total` | Предотвращённые дублирующие запросы |

## API

```rust
// Получить значение
let value: Option<MyType> = cache_service.get::<MyType>(tenant_id, "my_key").await?;

// Установить значение с TTL
cache_service.set(tenant_id, "my_key", &value, Some(Duration::from_secs(300))).await?;

// Удалить ключ (+ pub/sub инвалидация всех инстансов)
cache_service.invalidate(tenant_id, "my_key").await?;

// Batch-инвалидация по паттерну
cache_service.invalidate_pattern(tenant_id, "tenant_*").await?;
```

## Связанные документы

- [README crate](../README.md)
- [Реестр модулей](../../../docs/modules/registry.md)
- [Архитектурный обзор](../../../docs/architecture/overview.md)
- [Loco интеграция: Cache](../../../apps/server/docs/loco-core-integration-plan.md#23-loco-осознанный-самопис-не-мигрировать)
