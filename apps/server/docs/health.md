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

Отдельный инвариант compile-time profile: `embed-admin` и `embed-storefront` управляют не только routes,
но и самим linkage соответствующих UI host-ов; аналогично `mod-commerce`, `mod-blog`, `mod-forum`
и `mod-pages` управляют включением своих REST/OpenAPI transport fragments, а content-only maintenance
binary `migrate_legacy_richtext` требует `mod-content`. Reduced/headless server build не обязан
тянуть ecommerce или content surfaces, которые ему не нужны.

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
- `rate_limits`, `event_bus`, `event_transport`, `remote_executor`.

Prometheus surface теперь также публикует:

- `rustok_runtime_guardrail_runtime_dependencies_enabled`
- `rustok_runtime_guardrail_host_mode{mode="full|registry_only"}`
- `rustok_runtime_guardrail_remote_executor_enabled`
- `rustok_runtime_guardrail_remote_executor_state`
- `rustok_runtime_guardrail_remote_executor_active_claims`
- `rustok_runtime_guardrail_remote_executor_expired_claims`
- `rustok_runtime_guardrail_remote_executor_config{setting="lease_ttl_ms|requeue_scan_interval_ms"}`

Подробный контракт snapshot и его Prometheus-представление описаны в [runtime-guardrails.md](/C:/проекты/RusTok/docs/guides/runtime-guardrails.md).

## Локальный runbook для `registry_only`

Если нужно локально поднять read-only catalog host из того же бинарника `apps/server`, канонический
минимум сейчас такой:

```bash
RUSTOK_RUNTIME_HOST_MODE=registry_only cargo run -p rustok-server
```

```powershell
$env:RUSTOK_RUNTIME_HOST_MODE="registry_only"
cargo run -p rustok-server
```

Минимальный smoke после старта:

```bash
curl -i http://127.0.0.1:5150/health/ready
curl -i http://127.0.0.1:5150/health/runtime
curl -i http://127.0.0.1:5150/health/modules
curl -i http://127.0.0.1:5150/v1/catalog?limit=1
curl -i http://127.0.0.1:5150/v1/catalog/blog
curl -i http://127.0.0.1:5150/api/openapi.json
```

Ожидаемое поведение:

- `GET /health/ready` и `GET /health/modules` возвращают `200`, несмотря на reduced surface;
- `GET /health/runtime` явно возвращает `host_mode="registry_only"` и `runtime_dependencies_enabled=false`;
- `GET /v1/catalog` возвращает read-only catalog contract с `ETag`, `Cache-Control` и `X-Total-Count`;
- `GET /v1/catalog/{slug}` остаётся доступным как canonical detail contract для внешнего discovery;
- `GET /api/openapi.json` рекламирует только registry/health/metrics/swagger surface;
- `POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`, `POST /v2/catalog/publish/{request_id}/stages`, `POST /v2/catalog/publish/{request_id}/request-changes`, `POST /v2/catalog/publish/{request_id}/hold`, `POST /v2/catalog/publish/{request_id}/resume`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer` и `POST /v2/catalog/yank` не должны быть доступны и в норме дают `404`;
- `GET /api/graphql`, `GET /api/auth/me`, `GET /admin` не должны быть доступны и в норме дают `404`.

Для автоматизированной локальной проверки тот же runtime contract покрыт в
`scripts/verify/verify-deployment-profiles.sh` и `scripts/verify/verify-deployment-profiles.ps1`.
Если нужно прогнать тот же smoke уже против внешнего dedicated host, эти же скрипты теперь
понимают `RUSTOK_REGISTRY_BASE_URL`, optional `RUSTOK_REGISTRY_SMOKE_SLUG` и optional
`RUSTOK_REGISTRY_EVIDENCE_DIR`.

Если проверяется именно reduced build matrix, полезно отдельно подтвердить compile-time срез:

- `cargo check -p rustok-server --no-default-features` для самого узкого headless compile-time binary;
- `cargo check -p rustok-server --no-default-features --features redis-cache` для headless binary с Redis-backed runtime integrations;
- при server-side SEO/catalog/runtime изменениях дополнительно один module-sliced profile вроде
  `cargo check -p rustok-server --no-default-features --features mod-commerce` или targeted
  no-commerce content host, если конкретный deployment не должен тянуть чужой transport surface.

## Production rollout для `modules.rustok.dev`

Для внешнего dedicated catalog host канонический deployment contract сейчас такой:

- build profile: `headless-api` (`--no-default-features`; добавлять `redis-cache` только если deployment реально использует Redis-backed runtime integrations);
- runtime host mode: `RUSTOK_RUNTIME_HOST_MODE=registry_only`;
- process role: отдельный read-only host для V1 catalog, а не урезанный monolith;
- write-path V2 на этот host не маршрутизируется и не должен быть доступен после rollout.

Для этого dedicated host `mod-commerce` не является обязательным compile-time dependency, если каталог
не публикует ecommerce REST/OpenAPI surface.

Минимальный production checklist перед переключением трафика:

1. Убедиться, что deployment собран тем же `apps/server` бинарником, но без embedded admin/storefront surface.
2. Убедиться, что runtime env явно задаёт `RUSTOK_RUNTIME_HOST_MODE=registry_only`.
3. Проверить `/health/ready` и `/health/runtime` на целевом instance.
4. Проверить `GET /v1/catalog?limit=1` и `GET /v1/catalog/{slug}` на целевом instance.
5. Проверить `ETag`, `Cache-Control` и `X-Total-Count` на `GET /v1/catalog?limit=1`.
6. Проверить `GET /api/openapi.json` и убедиться, что в spec нет `/v2/catalog/*`, `/api/graphql`, `/api/auth/*`.
7. Проверить negative smoke: `POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`, `POST /v2/catalog/publish/{request_id}/stages`, `POST /v2/catalog/publish/{request_id}/request-changes`, `POST /v2/catalog/publish/{request_id}/hold`, `POST /v2/catalog/publish/{request_id}/resume`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer`, `POST /v2/catalog/yank`, `GET /api/graphql`, `GET /admin` должны давать `404`.

Provider-agnostic edge/runtime invariants для этого host:

- edge/CDN/reverse proxy не должны переписывать path prefix и query string для `/v1/catalog*`, `/health/*`, `/metrics`, `/api/openapi.*`;
- edge не должен вырезать `ETag`, `Cache-Control`, `If-None-Match` и `X-Total-Count`, потому что это часть live V1 contract;
- edge не должен подменять API-ответы собственными HTML error pages для `404` на write/admin paths;
- `GET /v1/catalog*` можно кэшировать только с уважением к origin headers; `/health/*` и `/api/openapi.*` не должны превращаться в долгоживущий CDN cache;
- TLS termination/HSTS и redirect policy должны быть настроены на edge, но без path rewrites и без downgrade на `http`;
- WAF/rate-limit layer не должен инжектить auth headers и не должен превращать expected `404` на write-path в provider-specific `401/403`, иначе теряется внешний reduced-surface contract.

Канонический automated smoke для уже развёрнутого host:

```bash
export RUSTOK_REGISTRY_BASE_URL="https://modules.rustok.dev"
export RUSTOK_REGISTRY_SMOKE_SLUG="blog"
export RUSTOK_REGISTRY_EVIDENCE_DIR="./tmp/modules-rustok-dev-smoke"
./scripts/verify/verify-deployment-profiles.sh
```

```powershell
$env:RUSTOK_REGISTRY_BASE_URL="https://modules.rustok.dev"
$env:RUSTOK_REGISTRY_SMOKE_SLUG="blog"
$env:RUSTOK_REGISTRY_EVIDENCE_DIR="C:\tmp\modules-rustok-dev-smoke"
./scripts/verify/verify-deployment-profiles.ps1
```

Этот external smoke не заменяет локальную build/profile matrix, а дополняет её:

- проверяет `/health/ready` и `/health/runtime` уже на публичном host;
- проверяет `/health/modules` как live marker для зарегистрированного `ModuleRegistry` даже на reduced host;
- проверяет `GET /v1/catalog?limit=1` и `GET /v1/catalog/{slug}` на live instance;
- проверяет `ETag`, `Cache-Control` и `X-Total-Count`;
- проверяет reduced OpenAPI (`/api/openapi.json` и `/api/openapi.yaml`) на отсутствие write/API/UI surface;
- проверяет, что `POST /v2/catalog/*`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer`, `POST /v2/catalog/yank` и `GET /admin` реально дают `404`.

Минимальный evidence package после rollout:

- сохранить stdout/stderr external smoke из `scripts/verify/verify-deployment-profiles.sh` или `.ps1`;
- сохранить ответ `/health/runtime` как rollout snapshot для этого release;
- сохранить snapshot `GET /api/openapi.json` как доказательство reduced surface;
- зафиксировать artifact identifier / build SHA / image tag и timestamp smoke-проверки;
- если перед host стоит CDN/WAF, отдельно отметить effective cache/TLS policy и отсутствие path rewrites для catalog endpoints.

Если задан `RUSTOK_REGISTRY_EVIDENCE_DIR`, verify-скрипт автоматически сохраняет туда как минимум:

- `runtime-headers.txt` и `runtime-body.json`;
- `catalog-headers.txt` и `catalog-body.json`;
- `openapi-headers.txt` и `openapi-body.json`;
- `openapi-yaml-headers.txt` и `openapi-yaml-body.yaml`;
- `registry-smoke-metadata.txt` с `base_url`, `smoke_slug` и UTC timestamp.

Минимальный acceptance после rollout:

- `/health/ready` возвращает `200`;
- `/health/runtime` возвращает `host_mode="registry_only"` и `runtime_dependencies_enabled=false`;
- `GET /v1/catalog` отвечает как cache-friendly V1 contract;
- `GET /v1/catalog/{slug}` отвечает как canonical detail contract;
- reduced OpenAPI не рекламирует write/API/UI surface;
- V2 write-path и monolith shell реально недоступны снаружи.

Rollback для этого host остаётся обычным rollback deployment-артефакта или переключением трафика на предыдущий release. Важный инвариант: не переводить `modules.rustok.dev` в `full` runtime как временную меру, потому что это ломает контракт dedicated read-only catalog host.
Отдельно для rollback/incident path: если smoke падает именно на reduced surface, сначала откатить deployment или traffic switch, а не чинить проблему временным включением full-host routes.

## Надёжность проверок

Для readiness-проверок используются:

- timeout на выполнение проверки;
- in-process circuit breaker;
- fail-fast поведение при открытом circuit.

Это предотвращает зависание `/health/ready` на проблемной зависимости.
