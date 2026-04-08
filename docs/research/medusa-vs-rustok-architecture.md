# Сравнение архитектуры RusTok и Medusa

## Зачем нужен этот документ

Этот документ фиксирует архитектурное сравнение RusTok и Medusa в контексте цели
`Medusa JS clone` для ecommerce family. Он нужен для трёх практических вопросов:

1. насколько Medusa технически ближе к modular monolith, чем к классическим микросервисам;
2. насколько текущая архитектура RusTok совместима с Medusa-style подходом;
3. где реалистично добиваться parity, а где нельзя рассчитывать на прямой reuse.

Состояние Medusa в этом сравнении сверялось по официальной документации Medusa на
`2026-04-08`.

## Короткий вывод

Medusa технически ближе к модульному монолиту с pluggable infrastructure и
workflow-orchestration, чем к классической microservices architecture.

RusTok в своей текущей форме архитектурно ближе к Medusa, чем к service-per-domain
ландшафту:

- обе системы строятся вокруг одного application runtime;
- обе разделяют домены на изолированные модули;
- обе предпочитают composition через контейнер/module registry, а не сетевые hops между
  внутренними доменами;
- обе держат third-party integrations на provider/module seams.

При этом RusTok и Medusa не идентичны:

- Medusa сильнее завязана на JS/TS container + workflow engine + plugin/module/provider
  contracts;
- RusTok сильнее завязан на Rust crate-boundaries, manifest-driven composition и thin-host
  модель;
- Medusa проектирует commerce как набор модулей внутри одного приложения;
- RusTok проектирует платформу шире ecommerce и использует commerce как один из module
  families.

## Что в Medusa делает её ближе к modular monolith

По официальной архитектуре Medusa приложение выглядит как единый runtime, где:

- HTTP/API routes запускают workflows;
- workflows координируют commerce и infrastructure modules;
- modules регистрируются в application container;
- модулям передаётся подключение к одной configured PostgreSQL database;
- integrations подключаются через module/provider seams, а не как обязательные
  внутренние микросервисы.

Это видно в официальных документах:

- [Medusa Architecture](https://docs.medusajs.com/learn/introduction/architecture)
- [Modules](https://docs.medusajs.com/learn/fundamentals/modules)
- [Commerce Modules](https://docs.medusajs.com/learn/fundamentals/modules/commerce-modules)
- [Core Workflows Reference](https://docs.medusajs.com/resources/medusa-workflows-reference)
- [Plugins](https://docs.medusajs.com/learn/fundamentals/plugins)

Из этого следуют важные архитектурные признаки:

1. `single runtime`
Medusa по умолчанию не требует отдельного процесса на каждый домен.

2. `shared container`
Module services резолвятся из общего контейнера приложения.

3. `shared database baseline`
Модули живут в одной application-level database environment, а не в отдельных service
datastores по умолчанию.

4. `internal orchestration instead of network choreography`
Связи между доменами обычно проходят через workflows/steps внутри процесса, а не через
HTTP/gRPC между внутренними сервисами.

5. `pluggable infrastructure instead of distributed core`
Redis, file storage, analytics, locking и внешние commerce providers подключаются как
сменные providers/modules, но core application от этого не превращается в набор
независимых бизнес-сервисов.

Поэтому маркетинговая подача про “composable” и “service integrations” у Medusa
технически не равна классической microservices architecture.

## Где RusTok и Medusa реально похожи

### 1. Оба проекта модульные, а не service-per-domain

В Medusa модули являются базовой единицей business capability. В RusTok базовая единица
тоже модуль/crate family.

Практически это означает:

- доменные boundaries выражены кодом и contracts;
- расширение идёт через module seams;
- внутренние домены не обязаны быть отдельными deployment units.

### 2. Оба проекта держат orchestration отдельно от доменных сервисов

У Medusa маршруты обычно вызывают workflows, а workflows используют module services.

У RusTok host и transport routes вызывают application/domain services в crates, при этом
`apps/server` остаётся thin host.

### 3. Оба проекта предпочитают extensibility через providers/adapters

Medusa использует module providers для fulfillment, auth, file, locking, analytics и
других capabilities.

RusTok идёт в ту же сторону через provider SPI, module boundaries и integration seams.

### 4. Оба проекта допускают platform-level reuse домена

В Medusa core commerce logic доступна не только через API, но и напрямую из custom flows.

В RusTok модульные сервисы тоже предполагаются как reusable building blocks для transport,
UI и orchestration layers.

### 5. Оба проекта допускают marketplace/composability narrative без обязательных микросервисов

И Medusa, и RusTok могут быть “компонуемыми” без перехода в distributed system внутри
ядра.

## Где RusTok и Medusa принципиально различаются

### 1. Medusa module model жёстко завязана на JS/TS runtime

Medusa ожидает:

- JS/TS modules;
- container registration names;
- workflow steps/workflow SDK;
- plugin packaging через npm;
- `medusa-config.ts` как точку composition.

RusTok использует:

- Rust crates;
- manifest-driven module wiring;
- host-side composition через platform registry;
- собственные transport/runtime contracts.

Это главное техническое препятствие для прямого reuse.

### 2. Medusa workflow engine является центральной частью app semantics

В Medusa workflow layer участвует в core flow semantics, rollback и extension hooks.

В RusTok orchestration выражена application services, state machines, event flow и
module-owned transport contracts, а не Medusa workflow SDK.

### 3. Medusa modules рассчитаны на Medusa container contracts

Medusa customizations строятся вокруг того, что module service можно резолвить из
контейнера и использовать в workflows/routes/subscribers/jobs.

RusTok сервисы не проектировались как Medusa container resources и не реализуют их
interfaces.

### 4. Medusa plugins шире module semantics

Плагин Medusa может одновременно содержать:

- modules;
- workflows;
- API routes;
- subscribers;
- scheduled jobs;
- admin extensions.

У RusTok это разложено иначе: module crate, host composition, UI package, docs, manifest.

### 5. Medusa думает commerce-first, RusTok думает platform-first

Medusa архитектурно центрирована вокруг ecommerce.

RusTok центрирован вокруг более широкой platform/module system, где ecommerce только один
из крупных bounded families.

### 6. Medusa по умолчанию сильнее унифицирует lifecycle вокруг собственного domain model

Если использовать Medusa как основной runtime, её workflows и modules ожидают определённые
input/output semantics, provider ids, actor model, lifecycle transitions и data ownership.

RusTok может сделать похожую логику, но это не означает автоматической binary/runtime
совместимости.

## 10 совпадений

1. Оба проекта архитектурно ближе к modular monolith, чем к классическим микросервисам.
2. Оба проекта используют модули как primary domain boundary.
3. Оба проекта допускают third-party integrations через provider/module seams.
4. Оба проекта держат orchestration поверх доменных сервисов.
5. Оба проекта позволяют повторно использовать core domain logic вне чистого HTTP layer.
6. Оба проекта требуют явных contract boundaries между transport и domain.
7. Оба проекта допускают постепенное расширение commerce через bounded contexts.
8. Оба проекта могут жить в single database/runtime baseline без потери composability.
9. Оба проекта требуют parity discipline, чтобы transport не оторвался от доменной модели.
10. Оба проекта выигрывают от thin host/composition root вместо business logic в routing layer.

## 10 различий

1. Medusa реализована как JS/TS application platform, RusTok как Rust module platform.
2. Medusa container и workflow SDK являются обязательной частью extension model.
3. RusTok использует manifest-driven composition, Medusa использует plugin/module registration.
4. Medusa глубже стандартизует provider interfaces вокруг собственного framework.
5. RusTok сильнее отделяет platform host от module crates и publishable UI packages.
6. Medusa из коробки больше стандартизует ecommerce actor/provider semantics.
7. RusTok шире по platform scope и не ограничен commerce.
8. Medusa extension ecosystem ориентирован на npm packages, RusTok на crate/module workspace.
9. Medusa workflows являются каноническим orchestration seam, у RusTok это не центральный
   runtime primitive.
10. Прямой in-process reuse между системами почти отсутствует из-за несовпадения runtime model.

## Что это означает для цели `Medusa JS clone`

Цель “сделать Medusa JS clone” для RusTok технически реалистична, если понимать её как:

- повторить bounded contexts;
- повторить domain semantics;
- повторить transport surface и operator flows;
- повторить provider seams;
- повторить lifecycle expectations;
- но не пытаться повторить внутренний JS runtime Medusa один-в-один.

То есть parity должна быть:

- `semantic parity`;
- `API/flow parity`;
- `operator capability parity`;
- `domain lifecycle parity`.

Необязательной является:

- `runtime implementation parity`;
- `plugin binary compatibility`;
- `in-process module compatibility`.

## Можно ли использовать наши модули внутри Medusa

### Короткий ответ

Прямо “как есть” — почти нет.

Через adapter/plugin/provider layer — да, местами вполне реалистично.

### Что мешает прямому reuse

- наши модули написаны не под Medusa container contracts;
- они не реализуют Medusa module/provider interfaces;
- они не встроены в Medusa workflow SDK;
- они не упакованы как Medusa plugins/modules для `medusa-config.ts`;
- их domain ownership и transport contracts проектировались под RusTok host.

### Где интеграция реалистична

Наиболее реалистичные сценарии:

1. `provider-style integration`
Подключать RusTok capability как внешний backend за Medusa provider/module adapter.

2. `service-backed custom module`
Писать Medusa custom module, который ходит в RusTok API и использует RusTok как system of
record для части capability.

3. `headless sidecar integration`
Использовать Medusa как storefront/admin/runtime ecosystem, а RusTok как отдельный
headless commerce backend для части доменов.

### Где интеграция будет самой дорогой

Самые дорогие зоны:

- cart/order/checkout как core flow;
- inventory reservation semantics;
- pricing/promotions;
- post-order changes/returns/refunds;
- любые workflow-heavy lifecycle paths.

Причина проста: там Medusa опирается не только на похожие сущности, но и на собственную
оркестрацию, compensation/rollback semantics и data ownership expectations.

## Рекомендуемая позиция для RusTok

Для RusTok полезно смотреть на Medusa как на:

- хороший reference architecture для ecommerce domains;
- хороший reference transport surface для `/store/*` и `/admin/*`;
- хороший reference operator/provider seam model;
- но не как на runtime, с которым нужно добиться прямой модульной совместимости.

Практическая рекомендация:

1. строить Medusa-compatible semantics и API shape там, где это приносит ценность;
2. не проектировать RusTok crates под in-process reuse внутри Medusa;
3. если когда-нибудь понадобится интеграция с Medusa, делать её через adapter/plugin/provider
   layer, а не через попытку “вставить” RusTok modules в Medusa runtime.

## Источники

- [Medusa Architecture](https://docs.medusajs.com/learn/introduction/architecture)
- [Medusa Modules](https://docs.medusajs.com/learn/fundamentals/modules)
- [Medusa Commerce Modules](https://docs.medusajs.com/learn/fundamentals/modules/commerce-modules)
- [Medusa Plugins](https://docs.medusajs.com/learn/fundamentals/plugins)
- [Medusa Config / modules/plugins registration](https://docs.medusajs.com/learn/configurations/medusa-config)
- [Medusa Core Workflows Reference](https://docs.medusajs.com/resources/medusa-workflows-reference)
- [Fulfillment Module Provider](https://docs.medusajs.com/resources/commerce-modules/fulfillment/fulfillment-provider)
- [Infrastructure Modules](https://docs.medusajs.com/resources/infrastructure-modules)
