# Fluid Frontend Architecture для RusTok

Fluid Frontend Architecture (FFA) — это предлагаемая RusTok архитектурная модель для
переносимых веб-фронтендов, в которой один и тот же frontend может работать как
встроенный монолитный runtime и как отдельный headless-клиент без переписывания UI-слоя.

Ключевая идея FFA: `headless` описывает topology deployment'а, а не идентичность
frontend-приложения. Меняется transport и runtime boundary, но не компоненты, роутинг,
state management и пользовательская логика.

## Контекст

Современная веб-разработка обычно выбирает одну из двух моделей.

### Монолит

```text
Frontend
    ↓
Backend
    ↓
Database
```

Frontend встроен в backend runtime и поставляется вместе с серверным приложением.
Типичные примеры: WordPress, Magento, Rails-монолиты.

Плюсы монолита:

- простое развёртывание;
- same-origin runtime;
- короткий путь до server-side auth, session, policy и service layer;
- меньше сетевых и CORS-boundary проблем.

Минусы монолита:

- frontend часто жёстко связан с backend runtime;
- standalone-клиент трудно вынести без переписывания;
- UI-компоненты начинают знать слишком много о deployment-среде.

### Headless

```text
Frontend → API → Backend → Database
```

Frontend и backend разворачиваются как отдельные системы. Типичные примеры: Next.js +
GraphQL, Saleor, Medusa.

Плюсы headless:

- независимое развёртывание frontend-а;
- удобная интеграция внешних клиентов;
- явный API boundary;
- возможность использовать разные frontend-стэки поверх одного backend-а.

Минусы headless:

- сетевой transport становится обязательной частью UI-архитектуры;
- frontend часто проектируется заново под API-only runtime;
- монолитная админка обычно не может стать standalone-клиентом;
- headless storefront обычно нельзя встроить обратно в backend runtime без отдельной
  реализации.

## Проблема

Большинство платформ, которые называют себя headless-совместимыми, на практике решают
только часть задачи. Они предоставляют:

- монолитный UI;
- внешний API;
- иногда отдельный headless starter или storefront template.

Но сам frontend остаётся непереносимым. В результате deployment topology диктует
frontend architecture:

- админка жёстко связана с backend runtime;
- storefront реализуется отдельно для headless-сценария;
- frontend-логика дублируется между embedded и remote UI;
- компоненты становятся transport-aware;
- миграция между монолитом и headless превращается в переписывание UI.

Фактически получается:

```text
headless mode = новый frontend
```

FFA предлагает другой контракт:

```text
headless mode = другая topology исполнения того же frontend-а
```

## Определение

**Fluid Frontend Architecture (FFA)** — архитектурная модель, в которой один и тот же
frontend может работать как встроенный монолитный runtime и как отдельный headless-клиент
без переписывания UI-слоя.

В FFA:

- идентичность frontend-а остаётся неизменной;
- topology исполнения становится текучей;
- transport является заменяемой инфраструктурной деталью;
- UI не зависит от того, выполняется ли backend-код локально, in-process, через GraphQL,
  REST, RPC, edge runtime или другой transport.

## Основные принципы FFA

### 1. Сохранение идентичности UI

Frontend остаётся одним и тем же приложением во всех режимах работы:

- те же компоненты;
- те же роуты;
- те же формы и state transitions;
- те же правила отображения;
- тот же module-owned UI package.

Если для monolith и headless приходится поддерживать разные UI-реализации одной и той же
поверхности, система не является fluid в полном смысле.

### 2. Transport agnosticism

UI-слой не должен зависеть от конкретного транспорта. Транспортами могут быть:

- локальные Leptos `#[server]` functions;
- in-process service calls;
- GraphQL;
- REST;
- RPC;
- edge functions;
- local-first синхронизация.

Компонент не должен напрямую кодировать знание о том, что он работает именно через
GraphQL или именно через server function. Он обращается к локальному frontend-facing
API-слою, а тот выбирает подходящий transport для текущего runtime.

Базовый паттерн:

```text
UI component
  → frontend-facing API adapter
  → runtime transport selector
  → local server function / in-process service / GraphQL / REST / RPC
  → domain service
```

### 3. Runtime fluidity

Frontend может перемещаться между runtime-профилями без перестройки архитектуры:

- embedded monolith;
- SSR/hydrate внутри backend host-а;
- standalone CSR/debug;
- remote headless host;
- hybrid deployment.

Runtime-профиль влияет на transport и packaging, но не должен требовать новой модели
компонентов.

### 4. Topology independence

Поведение frontend-а не зависит от:

- границ процессов;
- сетевой локальности;
- deployment topology;
- того, находится ли service layer в том же процессе или за API boundary.

Frontend не должен знать, где именно исполняется backend-код. Он должен знать только
стабильный application contract.

### 5. Parallel contracts вместо замены transport-а

FFA не требует выбрать один transport навсегда. Наоборот, FFA предполагает, что transport
может быть разным в разных runtime-профилях.

Для RusTok это особенно важно: появление native Leptos `#[server]` path не означает отказ
от GraphQL. Server functions дают короткий internal path для SSR-first monolith, а
GraphQL/REST сохраняют headless parity и внешний контракт.

## Как это выглядит в ecommerce-платформе

### Монолитный режим

```text
Admin UI
Storefront UI
    ↓
Backend runtime
    ↓
Database
```

Admin UI и Storefront UI встроены в один backend runtime. Они могут использовать
same-origin SSR/hydrate, server-side auth/session/policy и native internal calls.

### Headless-режим

```text
Admin UI      → GraphQL/REST API
Storefront UI → GraphQL/REST API
                    ↓
                Backend runtime
                    ↓
                 Database
```

Те же frontend bundles работают как удалённые клиенты через публичный API contract.

### Hybrid-режим

```text
Admin UI      → local server functions → Backend runtime
Storefront UI → GraphQL/REST API       → Backend runtime
Partner app   → GraphQL/REST API       → Backend runtime
```

Одна поверхность может быть embedded, другая — remote, а внешние клиенты продолжают
использовать API. FFA не требует, чтобы вся платформа одновременно была только монолитом
или только headless-системой.

## Почему существующие платформы часто не fluid

Многие платформы поддерживают API decoupling, но не поддерживают переносимость самого
frontend-а.

| Платформа | Headless support | Один frontend в embedded и headless runtime |
|-----------|------------------|---------------------------------------------|
| WordPress | Частично | Нет |
| Drupal | Частично | Нет |
| Shopify | Частично | Нет |
| Saleor | Да | Нет |
| Medusa | Да | Нет |

Эти платформы могут быть useful headless backend-ами, но обычно не гарантируют, что одна
и та же админка или storefront-поверхность будет без изменений работать и внутри
монолита, и как standalone/headless UI.

Иными словами, они решают:

```text
API decoupling
```

но не решают:

```text
frontend portability
```

## Почему FFA важна для RusTok

RusTok проектируется как модульная Rust-платформа, где backend host, module-owned UI и
headless API не должны развиваться как три независимые архитектуры. FFA даёт общий язык
для этого направления.

Для RusTok FFA означает:

- module-owned admin и storefront surfaces должны быть переносимыми между host-профилями;
- Leptos UI не должен становиться `#[server]`-only только потому, что монолитный SSR path
  удобнее;
- GraphQL/REST не должны исчезать только потому, что появился более короткий native path;
- Next.js hosts и внешние клиенты должны оставаться first-class headless consumers;
- embedded Leptos hosts должны получать преимущества same-origin SSR/hydrate без
  переписывания UI;
- runtime topology должна выбираться deployment-профилем, а не структурой компонентов.

Практический результат:

```text
deployment architecture перестаёт диктовать frontend architecture
```

## Как FFA соотносится с текущими UI-контрактами RusTok

Текущая dual-path модель RusTok уже близка к FFA:

- `apps/admin` и `apps/storefront` ориентируются на SSR-first Leptos runtime для
  embedded/monolith профиля;
- native Leptos `#[server]` functions используются как preferred internal data-layer там,
  где UI реально работает внутри SSR/hydrate host-а;
- GraphQL `/api/graphql` остаётся обязательным параллельным transport contract для
  Next.js hosts, standalone/debug профилей, внешних клиентов и headless parity;
- module-owned UI packages должны сохранять fallback-путь, если surface должна работать
  вне embedded runtime;
- `apps/server` держит server functions, GraphQL и REST как разные runtime surfaces, а не
  как взаимоисключающие архитектуры.

FFA превращает эту практику в более явную архитектурную модель: frontend идентичен,
transport выбирается runtime-профилем.

## Архитектурные правила для RusTok

Чтобы surface считалась совместимой с FFA, она должна соблюдать следующие правила.

### UI-компоненты не выбирают transport напрямую

Компонент не должен содержать ветвления вида `if headless { graphql } else { server_fn }`.
Transport selection должен жить в adapter/data-access слое, который является частью
frontend-facing contract конкретной поверхности.

### GraphQL и server functions сосуществуют

Для внутренних Leptos hosts native `#[server]` path может быть preferred, но GraphQL/REST
остаётся живым контрактом для headless и standalone paths. Добавление server function не
является причиной удалять GraphQL query, mutation или resolver, если они нужны для parity.

### Роутинг и state остаются стабильными

Переход из embedded runtime в headless runtime не должен менять:

- route semantics;
- URL-driven selection state;
- form lifecycle;
- validation model;
- authorization expectation;
- i18n contract.

### Host предоставляет runtime context

Module-owned UI должен получать runtime context от host-а, а не изобретать собственные
локальные схемы. Например, locale, tenant, auth/session и base API endpoints должны быть
host-provided contract-ами, а не package-local fallback chain.

### Domain contracts важнее transport contracts

Transport должен быть thin layer поверх domain/application service semantics. Если
GraphQL, REST и server function реализуют разные бизнес-правила, FFA нарушена: frontend
станет переносимым только формально.

## Почему Rust хорошо подходит для FFA

Rust-экосистема особенно хорошо подходит для FFA, потому что позволяет выразить различие
между domain contract и transport implementation без лишней runtime-магии.

Для FFA полезны:

- compile-time abstractions;
- transport-independent traits;
- shared domain types;
- строгие boundary между host, module и service layer;
- zero-cost abstraction switching;
- единые типы ошибок и DTO между transport-ами;
- возможность использовать один язык для backend, SSR runtime и части UI-экосистемы.

Стек RusTok вокруг Leptos, Axum и async-graphql позволяет строить frontend, где topology
исполнения является implementation detail, а не ограничением архитектуры.

## Критерии совместимости с FFA

Поверхность можно считать FFA-ready, если выполняются условия:

1. Один и тот же UI package может быть смонтирован в embedded host и использован в
   headless-compatible профиле.
2. Компоненты обращаются к frontend-facing adapter layer, а не к конкретному transport-у.
3. Есть live transport parity для нужных runtime-профилей.
4. GraphQL/REST fallback реально работает для standalone/headless paths, если surface
   заявляет такую поддержку.
5. Native server path не меняет бизнес-семантику по сравнению с remote API path.
6. Locale, tenant, auth и policy resolution приходят от host/runtime contract-а.
7. Документация surface явно описывает supported runtime profiles и transport matrix.

## Антипаттерны

FFA нарушается, если в системе появляются:

- отдельная embedded-админка и отдельная headless-админка с разной UI-логикой;
- GraphQL-only UI, который невозможно эффективно встроить в SSR-first monolith;
- `#[server]`-only UI, который невозможно проверить или запустить вне embedded runtime;
- transport-aware компоненты;
- локальные i18n/auth/tenant fallback chains внутри module-owned UI;
- разные domain rules в GraphQL resolver и server function;
- route/state model, меняющийся между monolith и headless deployment.

## Короткая формула

Традиционная модель:

```text
Monolith UI ≠ Headless UI
```

FFA-модель:

```text
Same UI + different transport + different topology = fluid frontend
```

Для RusTok это можно сформулировать так:

```text
Leptos embedded runtime и headless GraphQL/REST runtime должны быть режимами исполнения
одной UI-архитектуры, а не двумя разными frontend-продуктами.
```

## Связанные документы

- [Fluid Backend Architecture для RusTok](./fluid-backend-architecture.md)
- [План реализации Fluid Backend Architecture](./fluid-backend-architecture-implementation-plan.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [Архитектурные принципы](../architecture/principles.md)
- [API и surface-контракты](../architecture/api.md)
- [Маршрутизация](../architecture/routing.md)
- [SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
