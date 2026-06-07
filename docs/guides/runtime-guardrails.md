# Runtime Guardrails

Этот документ описывает operator-facing contract runtime guardrails в `apps/server`.

## Зачем это нужно

Runtime guardrails агрегируют живые сигналы рантайма в один snapshot, чтобы оператор быстро видел:

1. можно ли продолжать обслуживать трафик;
2. какой subsystem сейчас деградирует runtime.

Сейчас в snapshot входят:

- состояние rate-limit backends и memory saturation;
- состояние event transport fallback;
- состояние event bus backpressure.
- состояние `rustok.registry.remote_executor` для lease-based validation runner path.

## Endpoints

- `GET /health/runtime` — структурированный snapshot runtime guardrails;
- `GET /health/ready` — readiness с агрегированным статусом;
- `GET /metrics` — Prometheus-метрики guardrails.

## Snapshot Shape

`GET /health/runtime` возвращает:

- `status` — effective runtime status после rollout policy;
- `observed_status` — raw severity до softening в режиме `observe`;
- `rollout` — `observe` или `enforce`;
- `reasons` — человекочитаемые причины деградации;
- `rate_limits` — per-namespace состояние limiter'ов (`api`, `auth`, `oauth`);
- `event_bus` — snapshot backpressure budget;
- `event_transport` — relay fallback state.
- `remote_executor` — состояние internal validation runner contract (`enabled`, token/config, active/expired claims, lease policy).

## Как читать snapshot

Если `status != ok`, проверять поля в таком порядке:

1. `reasons`
2. `rate_limits[*].healthy`
3. `rate_limits[*].state`
4. `rate_limits[*].policy`
5. `event_transport.relay_fallback_active`
6. `event_bus.state`
7. `remote_executor.state`

## Основные сценарии

Rate-limit backend unavailable:

- `rate_limits[*].healthy = false`;
- обычно означает недоступность Redis или другого distributed backend;
- `/health/ready` должен содержать matching `runtime_guardrails` reason.

Memory limiter saturation:

- `rate_limits[*].distributed = false`;
- `total_entries` пересёк warning/critical thresholds;
- обычно лечится снижением cardinality, сокращением retention или переходом на distributed backend.

Event relay fallback active:

- `event_transport.relay_fallback_active = true`;
- для production это реальная деградация, а не harmless warning.

Event bus backpressure:

- `event_bus.state = degraded` или `critical`;
- `current_depth` подходит к `max_depth` или уже упирается в него;
- `events_rejected` показывает, начал ли runtime терять работу.

Remote executor degradation:

- `remote_executor.enabled = true`, но `token_configured = false` — critical misconfiguration;
- `remote_executor.expired_claims > 0` — reaper уже должен вернуть stage в `queued`, но оператору всё равно нужно смотреть на runner availability и lease policy;
- `remote_executor.active_claims` помогает отличать idle host от host, на котором реально работают thin runners.

## Метрики

Через `/metrics` публикуются:

- `rustok_runtime_guardrail_rollout_mode`
- `rustok_runtime_guardrail_observed_status`
- `rustok_runtime_guardrail_status`
- `rustok_runtime_guardrail_rate_limit_backend_healthy`
- `rustok_runtime_guardrail_rate_limit_state`
- `rustok_runtime_guardrail_rate_limit_total_entries`
- `rustok_runtime_guardrail_rate_limit_active_clients`
- `rustok_runtime_guardrail_rate_limit_config`
- `rustok_runtime_guardrail_event_transport_fallback_active`
- `rustok_runtime_guardrail_event_backpressure_state`
- `rustok_runtime_guardrail_remote_executor_enabled`
- `rustok_runtime_guardrail_remote_executor_state`
- `rustok_runtime_guardrail_remote_executor_active_claims`
- `rustok_runtime_guardrail_remote_executor_expired_claims`
- `rustok_runtime_guardrail_remote_executor_config`


## Wave 6 diagnostics runbook

Этот раздел закрывает короткий runbook для быстрых P0/P1 diagnostics из плана
`docs/research/error fix.md`. Он предназначен для ситуаций, когда оператору или
ревьюеру нужно проверить runtime invariants без полной компиляции workspace.

### Module graph drift

Симптомы:

- `cargo xtask validate-manifest` падает на несовпадении `modules.toml`,
  generated runtime registry или central registry evidence;
- `scripts/verify/verify-runtime-context-invariants.mjs` сообщает, что
  `pages -> [content, page_builder]` больше не подтверждается source-level
  evidence.

Быстрая диагностика:

```bash
cargo xtask validate-manifest
node scripts/verify/verify-runtime-context-invariants.mjs
```

Что проверить:

1. `modules.toml` — canonical dependency graph.
2. `apps/server/src/modules/mod.rs` — runtime registry/test evidence.
3. `docs/modules/registry.md` — central documentation evidence.

Исправление считается корректным только если manifest, runtime registry и docs
снова описывают один и тот же graph; ручной special-case для одного модуля не
считается достаточным.

### Channel resolution без locale или OAuth/client dimension

Симптомы:

- channel resolver получает пустой `RequestFacts.locale` при наличии resolved
  locale в request extensions;
- разные OAuth/client contexts шарят один channel cache key;
- negative cache entry повторно используется для другого locale/client context.

Быстрая диагностика:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

Что проверить:

1. `apps/server/src/middleware/channel.rs` — `build_request_facts` читает
   `AuthContextExtension` и `ResolvedRequestLocale`.
2. `ChannelCacheKey` содержит `oauth_app_id` и `locale`.
3. `apps/server/src/services/app_router.rs` сохраняет фактический порядок
   execution: locale -> auth_context -> channel.

### Locale DB amplification

Симптомы:

- repeated tenant-bound requests стабильно увеличивают
  `rustok_tenant_locale_db_queries_total` без cache hits;
- `rustok_tenant_locale_cache_misses_total` растёт на каждый запрос одного
  tenant внутри TTL;
- `rustok_tenant_locale_cache_entries` не отражает ожидаемые tenant snapshots.

Быстрая диагностика:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
curl -s http://localhost:5150/metrics | rg 'rustok_tenant_locale_(cache|db)'
```

Что проверить:

1. `apps/server/src/middleware/locale.rs` — tenant locale policy cache включён
   перед DB lookup.
2. `apps/server/src/controllers/metrics.rs` — cache hit/miss/db query/
   invalidation counters и entries gauge экспортируются.
3. Если policy менялась вручную, проверить invalidation path или дождаться TTL
   snapshot refresh перед сравнением метрик.

### Migration dependency failure

Симптомы:

- `migration-smoke` падает на пустой PostgreSQL DB;
- dependency descriptor ссылается на отсутствующую migration;
- order/cycle validation падает после добавления module migration.

Быстрая диагностика:

```bash
./scripts/verify/verify-migration-smoke.sh
RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 ./scripts/verify/verify-migration-smoke.sh
```

Что проверить:

1. Module crate с cross-module FK/order assumption объявляет
   `migration_dependencies()` рядом с `migrations()`.
2. Server migrator агрегирует descriptors через module `MigrationSource`, а не
   через package-local allowlist для одного crate.
3. Descriptor names ссылаются на существующие migrations, без duplicate/cycle.
4. Если failure воспроизводится только в GitHub Actions, фиксировать именно
   environment-specific причину, не отключая smoke job.

### Inventory admin boundary drift

Симптомы:

- inventory admin write facade начинает использовать transitional GraphQL
  fallback;
- `set_variant_quantity` / `adjust_variant_quantity` снова выводят `inStock`
  только из числовой quantity;
- transitional adapter содержит inventory mutation markers.

Быстрая диагностика:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
```

Что проверить:

1. `crates/rustok-inventory/src/services/inventory.rs` — typed write result
   строится из committed quantity + inventory policy.
2. `crates/rustok-inventory/admin/src/api.rs` — write facades идут через
   `crate::native::*` без `fallback_*`.
3. `crates/rustok-inventory/admin/src/transport.rs` — transitional GraphQL
   adapter остаётся read-only до удаления adapter-а.

## Stop-the-line условия

- любой limiter backend стал unhealthy;
- event relay fallback активирован;
- event bus дошёл до critical backpressure;
- readiness деградировал из-за runtime guardrails, а причина не объяснена оператором.

## Связанные файлы

- [health.rs](/C:/проекты/RusTok/apps/server/src/controllers/health.rs)
- [metrics.rs](/C:/проекты/RusTok/apps/server/src/controllers/metrics.rs)
- [runtime_guardrails.rs](/C:/проекты/RusTok/apps/server/src/services/runtime_guardrails.rs)
- [rate-limiting.md](/C:/проекты/RusTok/docs/guides/rate-limiting.md)
