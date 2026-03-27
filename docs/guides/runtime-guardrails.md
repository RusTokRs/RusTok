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

## Как читать snapshot

Если `status != ok`, проверять поля в таком порядке:

1. `reasons`
2. `rate_limits[*].healthy`
3. `rate_limits[*].state`
4. `rate_limits[*].policy`
5. `event_transport.relay_fallback_active`
6. `event_bus.state`

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
