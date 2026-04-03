# Health endpoints (`apps/server`)

Документ описывает поведение health endpoints в `apps/server/src/controllers/health.rs`.

## Endpoints

- `GET /health` — базовый статус процесса и версия приложения.
- `GET /health/live` — liveness probe.
- `GET /health/ready` — readiness probe с агрегированным статусом зависимостей и модулей.
- `GET /health/runtime` — operator-facing snapshot runtime guardrails.
- `GET /health/modules` — health только по зарегистрированным модулям.

Если `apps/server` запущен в `settings.rustok.runtime.host_mode = "registry_only"`, health/observability surface
работает как read-only catalog host, а не как full monolith.

Важно: `host_mode` не заменяет deployment profile. `DeploymentProfile` продолжает описывать build/deploy
surface (`monolith`, `server-with-admin`, `server-with-storefront`, `headless-api`), а
`settings.rustok.runtime.host_mode` описывает только runtime-exposed API surface (`full` или
`registry_only`).

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

### Registry-only mode

В `settings.rustok.runtime.host_mode = "registry_only"` readiness выравнивается под реально поднятый surface:

- остаются только `database`, `cache_backend` и marker-check `host_mode`;
- не проверяются `tenant_cache_invalidation`, `event_transport`, `search_backend`, rate-limit runtime и module runtime;
- `modules` в readiness не используются как hard gate и возвращают operator marker вместо попытки валидировать полный module runtime.

## Aggregation

- если есть `critical` проверка со статусом `unhealthy`, общий статус `unhealthy`;
- если critical `unhealthy` нет, но есть не-`ok` проверки, общий статус `degraded`;
- если все проверки `ok`, общий статус `ok`.

## Runtime guardrails

`/health/runtime` возвращает rollout-aware snapshot для операторов:

- `status` и `observed_status` для effective/raw severity;
- `rollout` (`observe|enforce`);
- `host_mode` (`full|registry_only`);
- `runtime_dependencies_enabled` — поднят ли полный runtime dependency layer;
- `reasons` с человекочитаемыми причинами деградации;
- `rate_limits`, `event_bus`, `event_transport`.

Prometheus surface теперь также публикует:

- `rustok_runtime_guardrail_runtime_dependencies_enabled`
- `rustok_runtime_guardrail_host_mode{mode="full|registry_only"}`

Подробный контракт snapshot и его Prometheus-представление описаны в [runtime-guardrails.md](/C:/проекты/RusTok/docs/guides/runtime-guardrails.md).

## Локальный runbook для `registry_only`

Если нужно локально поднять read-only catalog host из того же бинарника `apps/server`, канонический
минимум сейчас такой:

```bash
RUSTOK_RUNTIME_HOST_MODE=registry_only cargo run -p rustok-server
```

Минимальный smoke после старта:

```bash
curl -i http://127.0.0.1:5150/health/ready
curl -i http://127.0.0.1:5150/health/modules
curl -i http://127.0.0.1:5150/v1/catalog?limit=1
curl -i http://127.0.0.1:5150/api/openapi.json
```

Ожидаемое поведение:

- `GET /health/ready` и `GET /health/modules` возвращают `200`, несмотря на reduced surface;
- `GET /v1/catalog` возвращает read-only catalog contract с `ETag`, `Cache-Control` и `X-Total-Count`;
- `GET /api/openapi.json` рекламирует только registry/health/metrics/swagger surface;
- `GET /api/graphql`, `GET /api/auth/me`, `GET /admin` не должны быть доступны и в норме дают `404`.

Для автоматизированной локальной проверки тот же runtime contract покрыт в
`scripts/verify/verify-deployment-profiles.sh`.

## Надёжность проверок

Для readiness-проверок используются:

- timeout на выполнение проверки;
- in-process circuit breaker;
- fail-fast поведение при открытом circuit.

Это предотвращает зависание `/health/ready` на проблемной зависимости.
