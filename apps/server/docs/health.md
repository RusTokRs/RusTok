# Health endpoints (`apps/server`)

Документ описывает поведение health endpoints в `apps/server/src/controllers/health.rs`.

## Endpoints

- `GET /health` — базовый статус процесса и версия приложения.
- `GET /health/live` — liveness probe.
- `GET /health/ready` — readiness probe с агрегированным статусом зависимостей и модулей.
- `GET /health/runtime` — operator-facing snapshot runtime guardrails.
- `GET /health/modules` — health только по зарегистрированным модулям.

## Readiness модель

`/health/ready` возвращает:

- `status`: `ok | degraded | unhealthy`
- `checks`: инфраструктурные проверки
- `modules`: health модулей из `ModuleRegistry`
- `degraded_reasons`: список причин деградации

### Dependency checks

- `database` — критичная проверка доступности БД;
- `cache_backend` — базовая проверка tenant cache path;
- `tenant_cache_invalidation` — не-критичная проверка Redis pubsub listener для cross-instance invalidation;
- `event_transport` — критичная проверка инициализации event transport;
- `search_backend` — не-критичная проверка search connectivity.

## Aggregation

- если есть `critical` проверка со статусом `unhealthy`, общий статус `unhealthy`;
- если critical `unhealthy` нет, но есть не-`ok` проверки, общий статус `degraded`;
- если все проверки `ok`, общий статус `ok`.

## Runtime guardrails

`/health/runtime` возвращает rollout-aware snapshot для операторов:

- `status` и `observed_status` для effective/raw severity;
- `rollout` (`observe|enforce`);
- `reasons` с человекочитаемыми причинами деградации;
- `rate_limits`, `event_bus`, `event_transport`.

Подробный контракт snapshot и его Prometheus-представление описаны в [runtime-guardrails.md](/C:/проекты/RusTok/docs/guides/runtime-guardrails.md).

## Надёжность проверок

Для readiness-проверок используются:

- timeout на выполнение проверки;
- in-process circuit breaker;
- fail-fast поведение при открытом circuit.

Это предотвращает зависание `/health/ready` на проблемной зависимости.
