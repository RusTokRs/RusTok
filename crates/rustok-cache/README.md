# rustok-cache

Crate для управления кешированием в RusToK.
Выделен из `rustok-core` для разделения ответственности между инфраструктурным ядром и lifecycle Redis/Moka соединений.

## Назначение

Предоставляет:

- **`CacheModule`** — `RusToKModule`-реализация, регистрирующая backend в `ModuleRegistry`.
- **`CacheService`** — высокоуровневый сервис get/set/del/invalidate с поддержкой namespace и TTL.
- **`CacheBackendFactory`** — создание backend из конфигурации (`moka`, `redis`, `fallback`).

## Как работает

`CacheBackend` абстрагирует три режима:

```
Moka (in-process)
  └─ circuit breaker, anti-stampede coalescing, negative cache

Redis (distributed)
  └─ pub/sub invalidation между инстансами
  └─ connection-manager с авто-переподключением

FallbackCacheBackend
  └─ Redis → Moka (при недоступности Redis деградирует gracefully)
  └─ метрики hit/miss, ошибок, degradation
```

Все три варианта реализуют `CacheBackend` из `rustok-core`.

## Зона ответственности

- Lifecycle Redis-соединения (pool, reconnect, health check).
- Конфигурирование и создание `CacheBackend` по settings.
- Регистрация как `ModuleKind::Core` в `ModuleRegistry`.
- Метрики: cache hit/miss, degradation, error rate.

## Взаимодействие

| Компонент | Связь |
|-----------|-------|
| `rustok-core` | Зависит от: предоставляет `CacheBackend` trait и `ModuleRegistry` |
| `apps/server` | Регистрирует `CacheModule` при старте |
| `rustok-tenant` | Использует `CacheService` для кэширования tenant-resolver |
| `rustok-rbac` | Использует `CacheService` для кэширования RBAC-матрицы |

## Точки входа

- `src/lib.rs` — публичный API: `CacheModule`, `CacheService`, `CacheBackendFactory`
- `src/backends/` — реализации `moka.rs`, `redis.rs`, `fallback.rs`
- `src/module.rs` — `impl RusToKModule for CacheModule`

## Документация

- [Детальная документация](./docs/README.md)
- [Глобальный docs/index.md](../../docs/index.md)
