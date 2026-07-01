# ADR: Портовая граница для content orchestration

- Status: Accepted
- Date: 2026-03-28

## Контекст

После storage split `rustok-blog`, `rustok-forum`, `rustok-pages` и `rustok-comments`
владеют собственными таблицами, но продолжают зависеть от `rustok-content` как от shared helper
слоя для locale, rich-text, slug policy и других общих правил.

Старый `ContentOrchestrationService` оставался завязан на `NodeService` и shared `nodes`,
что ломало новую архитектурную границу:

- orchestration зависел от legacy storage topology;
- перенос `topic ↔ post` был реализован как rebinding shared children;
- прямое переписывание `rustok-content` на зависимости `blog/forum/comments` создавало бы
  циклический граф зависимостей.

## Решение

`ContentOrchestrationService` переводится на портовую границу:

- `rustok-content` оставляет у себя orchestration state, idempotency, audit и event publication;
- доменная конверсия выносится в `ContentOrchestrationBridge`;
- runtime-адаптеры, которые знают о `blog/forum/comments` persistence, должны жить вне
  shared helper слоя и реализовывать `ContentOrchestrationBridge`;
- runtime-адаптер и conversion GraphQL mutation surface живут в
  `rustok-content-orchestration`; canonical-route query остаётся в `rustok-content`;
- content GraphQL entity dataloaders для `nodes`, `node_translations` и `bodies`
  также живут в `rustok-content`; `apps/server` может регистрировать их, но не владеть ими;
- `apps/server` только подключает эти GraphQL roots и не владеет resolver-ами, DTO или
  concrete connection-типами модулей;
- `rustok-content` больше не имеет права напрямую переносить shared `node` children между
  родителями и не должен считать `nodes` каноническим источником истины для conversion flows.

## Последствия

Плюсы:

- убирается жёсткая зависимость orchestration от legacy `NodeService`;
- сохраняется роль `rustok-content` как shared helper/orchestration слоя без циклов;
- RBAC, idempotency, audit и event contract остаются централизованными.

Минусы:

- для реальных runtime conversion flows нужен отдельный adapter layer;
- host schema composition зависит от feature-gated owner/support GraphQL entrypoints
  и owner-owned dataloader types;
- mapping rules `blog comments ↔ forum replies` теперь должны быть описаны явно и реализованы
  в integration-адаптере, а не “магически” через shared topology.

## Что считается недопустимым

- возвращать `ContentOrchestrationService` на shared `NodeService`;
- привязывать новую orchestration-логику к `nodes`/`node_translations` как к источнику истины;
- добавлять прямые зависимости `rustok-content -> rustok-blog/rustok-forum/rustok-comments`,
  если это замыкает цикл зависимостей.
- возвращать conversion GraphQL resolver/DTO, content entity dataloaders или module-specific concrete connection-типы
  в `apps/server`.
