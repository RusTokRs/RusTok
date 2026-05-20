# Fluid Backend Architecture для RusTok

Fluid Backend Architecture (FBA) — это предлагаемая RusTok архитектурная модель для
переносимых backend-модулей, в которой один и тот же module-owned domain/service слой
может работать как embedded часть modular monolith и как отдельный remote service без
переписывания бизнес-логики.

Короткий ответ на вопрос «можно ли применить похожий паттерн к backend-у, чтобы модули
могли работать как микросервисы через `server-grpc`»: **да, но FBA должна описывать не
обязательный переход всех модулей в микросервисы, а переносимость backend boundary между
in-process и out-of-process topology**.

В FBA `microservice` — это topology deployment-а, а не новая идентичность модуля. Меняется
runtime boundary и transport, но не domain contract, ownership, tenancy, policy model,
observability и module lifecycle semantics.

## Связь с FFA

Fluid Frontend Architecture (FFA) отвечает на вопрос:

```text
может ли один frontend работать embedded и headless без переписывания UI?
```

Fluid Backend Architecture (FBA) отвечает на симметричный backend-вопрос:

```text
может ли один backend-модуль работать in-process и remote service без переписывания
business/application layer?
```

Сравнение:

| Слой | Fluid-модель | Что становится заменяемым | Что остаётся стабильным |
|---|---|---|---|
| Frontend | FFA | `#[server]`, GraphQL, REST, RPC, edge transport | UI identity, routes, state, UX logic |
| Backend | FBA | in-process call, gRPC, HTTP/RPC, async events | module identity, service contract, domain rules |

Идея одинаковая: topology исполнения не должна диктовать архитектурную идентичность
компонента.

## Проблема backend-модулей

Обычно platform backend развивается одним из двух путей.

### Modular monolith

```text
Server process
├─ Module A
├─ Module B
└─ Module C
        ↓
     Database
```

Плюсы:

- простой deployment;
- короткий in-process call path;
- общие транзакции и единый request context;
- меньше сетевой сложности;
- удобнее поддерживать typed Rust contracts.

Минусы:

- operational isolation ограничен;
- тяжёлые модули нельзя независимо масштабировать;
- failure domain часто общий;
- модуль трудно вынести в отдельный сервис, если domain/service слой связан с host internals.

### Microservices

```text
Server/API gateway → Service A
                   → Service B
                   → Service C
                         ↓
                  Service-owned storage
```

Плюсы:

- независимое масштабирование;
- отдельные release/deploy cadence;
- независимые failure domains;
- явные service boundaries.

Минусы:

- network boundary становится частью business path;
- появляются latency, retries, timeouts, compatibility windows;
- сложнее транзакции и consistency;
- легко получить distributed monolith, если boundaries не совпадают с ownership.

## Почему «сразу микросервисы» — не цель FBA

FBA не говорит, что каждый модуль RusTok должен стать отдельным сервисом. Для большинства
модулей embedded modular monolith остаётся лучшим default: он проще, быстрее, дешевле в
эксплуатации и лучше подходит для ранней продуктовой итерации.

FBA нужна для другого: **проектировать module boundary так, чтобы remote extraction была
архитектурно возможна без переписывания модуля**.

Иными словами:

```text
FBA ≠ microservices-first
FBA = service-boundary-ready modules
```

## Определение

**Fluid Backend Architecture (FBA)** — архитектурная модель, в которой один и тот же
backend-модуль может работать как in-process часть modular monolith и как отдельный remote
service без переписывания domain/application layer.

В FBA:

- идентичность модуля остаётся неизменной;
- module-owned service contract остаётся canonical;
- transport является adapter layer, а не владельцем бизнес-логики;
- runtime topology может быть embedded, remote или hybrid;
- tenant, auth, locale, channel, policy и observability context передаются через общий
  contract, а не через ad-hoc headers;
- gRPC является одним из transport-ов для backend-to-backend boundary, но не заменяет
  GraphQL/REST/UI contracts.

## `server-grpc` как backend transport profile

Для RusTok `server-grpc` можно рассматривать как optional backend transport profile:

```text
apps/server ──in-process──> module service
apps/server ───gRPC───────> module service process
```

Важно: `server-grpc` — это не новый public API для UI и не замена GraphQL/REST. Его роль —
backend-to-backend communication между host/runtime и module-owned service boundary.

Минимальная модель:

```text
module domain/service trait
        │
        ├─ in-process implementation
        │
        └─ gRPC client/server adapter
```

При этом GraphQL, REST, `#[server]` functions и UI-facing surfaces остаются внешними
контрактами host-а. Они обращаются к тому же module service contract, независимо от того,
выполняется он локально или удалённо.

## Топологии FBA

### 1. Embedded modular monolith

```text
apps/server
  ├─ product module service
  ├─ order module service
  └─ forum module service
        ↓
     shared runtime/database boundary
```

Это baseline topology для RusTok. Модули живут in-process, но уже имеют явные typed
service contracts и не зависят напрямую от host-specific shortcuts.

### 2. Remote module service

```text
apps/server → gRPC → product-service
apps/server → gRPC → order-service
apps/server → gRPC → forum-service
```

Host остаётся composition root для public API, auth/session, tenant routing и UI-facing
contracts, но конкретный module service может выполняться в отдельном процессе.

### 3. Hybrid topology

```text
apps/server
  ├─ in-process: pages, blog, seo
  ├─ gRPC: search/index
  └─ gRPC: ai/recommendations
```

Часть модулей остаётся embedded, часть выносится remote. Эта модель наиболее практична:
она позволяет выносить только те модули, где есть реальная operational причина.

### 4. Async-first companion service

```text
module service → outbox/events → worker/service
```

Не каждый backend boundary должен быть synchronous gRPC. Для indexing, email, analytics,
AI enrichment, media processing и похожих задач event/outbox path часто лучше, чем request
path RPC.

## Основные принципы FBA

### 1. Сохранение идентичности модуля

Модуль остаётся тем же самым модулем во всех topology:

- тот же `slug` и ownership;
- те же domain rules;
- тот же service contract;
- те же RBAC/policy expectations;
- та же документация runtime profiles;
- та же совместимость с module lifecycle.

Если remote service требует отдельной реализации бизнес-логики, FBA нарушена.

### 2. Transport agnosticism service layer-а

Application/domain layer не должен знать, вызвали его in-process или через gRPC.

Правильная форма:

```text
GraphQL/REST/#[server]/job/CLI
        ↓
module service contract
        ↓
in-process impl или remote client adapter
```

Неправильная форма:

```text
GraphQL resolver → in-process business logic
Grpc handler     → separate business logic
REST handler     → third business logic
```

### 3. Context propagation как contract

Remote boundary не должен превращаться в набор случайных headers. Через FBA boundary
нужно явно переносить:

- tenant context;
- authenticated principal/service identity;
- RBAC/policy claims;
- locale/effective language context, если операция зависит от locale;
- channel context для storefront/read-side scenarios;
- correlation/request id;
- trace/span context;
- idempotency key для retry-safe mutations;
- deadline/timeout/cancellation semantics.

### 4. Data ownership и consistency должны быть явными

FBA не должна скрывать вопрос хранения данных. Для каждого remote-capable модуля нужно
заранее описать, какой режим он поддерживает:

| Режим | Описание | Когда подходит |
|---|---|---|
| Shared database, in-process | Модуль работает в общем DB/schema boundary host-а | baseline modular monolith |
| Shared database, remote service | Remote process обращается к той же DB с теми же tenant/policy constraints | временная extraction или controlled internal deployment |
| Service-owned database | Модуль владеет своей storage boundary и публикует read/write contracts | зрелый microservice boundary |
| Read-model replica | Remote service держит read-side projection через events/outbox | search, index, analytics, recommendations |

Переход к service-owned database — отдельное архитектурное решение. FBA может подготовить
contract, но не должна автоматически обещать distributed transaction semantics.

### 5. Synchronous RPC — не замена events

`server-grpc` подходит для request/response операций, где host-у нужен немедленный ответ.
Для фоновых, eventually consistent и fan-out сценариев лучше использовать events/outbox.

Практическое правило:

- command/query в request path → может быть gRPC;
- workflow, projection, integration, notification → чаще event/outbox;
- cross-module transaction → требует отдельного design, а не «просто gRPC».

### 6. Observability parity

Один и тот же service contract должен давать сопоставимую telemetry в embedded и remote
режимах:

- metrics для latency/error rate;
- tracing spans на host и service side;
- structured errors;
- health/readiness для remote service;
- version/capability negotiation;
- clear degradation path при недоступности optional remote service.

## Где FBA особенно полезна в RusTok

FBA имеет смысл применять не ко всем модулям одинаково, а к тем boundaries, где есть
реальная причина для remote topology.

Хорошие кандидаты:

- search/indexing service;
- AI/recommendations/enrichment;
- media processing;
- heavy export/import и batch jobs;
- integrations/webhooks dispatch;
- analytics/reporting projections;
- high-throughput catalog read-side;
- fraud/risk scoring, если появится отдельный lifecycle.

Осторожнее с выносом:

- checkout/payment/order write path;
- tenant/module lifecycle;
- auth/session/RBAC core;
- i18n/locale resolution foundation;
- cross-cutting SEO metadata write path;
- любые flows, где нужна strong consistency без зрелой saga/outbox модели.

## Архитектурный скелет для RusTok

Для FBA-ready модуля backend можно мыслить слоями:

```text
crates/rustok-<module>/
  domain types
  application service trait
  in-process service implementation
  repository interfaces
  errors/DTOs/context contracts

crates/rustok-<module>-grpc/       optional
  protobuf/service schema
  grpc server adapter
  grpc client adapter
  context mapping
  error mapping

apps/server
  module wiring
  transport selection
  public API surfaces
```

Ключевой момент: gRPC crate не владеет бизнес-логикой. Он только сериализует вызов,
переносит context и вызывает canonical service contract.

## Критерии FBA-ready модуля

Модуль можно считать FBA-ready, если выполняются условия:

1. Domain/application service contract отделён от Axum, GraphQL, REST и host internals.
2. Public API handlers вызывают service contract, а не дублируют business logic.
3. Есть typed request context для tenant/auth/locale/channel/policy/trace data.
4. Ошибки имеют stable mapping между domain errors и transport status codes.
5. Mutations имеют idempotency/deadline/retry story, если их планируют вызывать remote.
6. Cross-module dependencies выражены через explicit ports/events, а не через прямой доступ
   к чужим repository internals.
7. Data ownership и consistency model описаны в local docs модуля.
8. Observability и health/readiness поведение описаны для remote profile.
9. Версионирование service contract-а не ломает embedded и remote topology одновременно.
10. `server-grpc` profile можно включить или отключить без изменения UI-facing API contract-а.

## Антипаттерны

FBA нарушается, если появляются:

- «микросервис» с отдельной копией бизнес-логики;
- gRPC handler как новый canonical domain owner;
- прямые SQL-запросы из host-а в таблицы remote-owned модуля;
- ad-hoc headers для tenant/auth/locale вместо typed context contract;
- synchronous RPC там, где нужен event/outbox workflow;
- distributed transaction без явной saga/compensation модели;
- разные RBAC/policy checks для embedded и remote mode;
- remote service, который невозможно запустить in-process для local/dev/test profile;
- module extraction без документации data ownership и failure semantics.

## Практический вывод

Да, RusTok может применить похожий паттерн к backend-у. Но лучше формулировать FBA не как
«все модули становятся микросервисами», а так:

```text
Один module-owned backend contract может выполняться in-process или через server-grpc,
а public API/UI/event contracts продолжают видеть тот же модуль и те же domain semantics.
```

Это даёт RusTok постепенный путь:

1. сначала строить строгий modular monolith;
2. затем выделять service contracts и context propagation;
3. потом добавлять optional `server-grpc` adapters для подходящих модулей;
4. только после этого выносить конкретные модули в remote deployment, если есть
   operational причина.

## Короткая формула

Традиционная модель:

```text
Microservice mode = новая backend-реализация
```

FBA-модель:

```text
Same module service + different transport + different topology = fluid backend
```

Для RusTok это можно сформулировать так:

```text
Modular monolith и server-grpc microservice profile должны быть режимами исполнения
одного module-owned backend contract-а, а не двумя разными backend-продуктами.
```

## Связанные документы

- [План реализации Fluid Backend Architecture](./fluid-backend-architecture-implementation-plan.md)
- [Fluid Frontend Architecture для RusTok](./fluid-frontend-architecture.md)
- [Архитектура модулей](../architecture/modules.md)
- [API и surface-контракты](../architecture/api.md)
- [Контракт event flow](../architecture/event-flow-contract.md)
- [DataLoader](../architecture/dataloader.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Как писать модуль в RusToK](../modules/module-authoring.md)
