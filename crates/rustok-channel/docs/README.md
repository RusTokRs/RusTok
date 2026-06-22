# Документация `rustok-channel`

`rustok-channel` — experimental core-модуль, который вводит platform-level
channel context для delivery surfaces и channel-aware runtime resolution.

## Назначение

- публиковать канонический runtime entry type `ChannelModule`;
- держать channel resolution logic внутри модуля, а не в `apps/server`;
- давать платформе единый channel-aware contract для host runtime и domain consumers.

## Зона ответственности

- storage для `channels`, `channel_targets`, `channel_module_bindings`, `channel_oauth_apps`;
- storage для `channel_resolution_policy_sets` и `channel_resolution_policy_rules`;
- domain-owned resolution layer: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, `ChannelResolver`;
- tenant-scoped typed resolution policies и explicit default channel semantics;
- canonical resolution order `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`, где built-in host fast-path остаётся отдельным совместимым слоем перед policy-only evaluation;
- module-owned Leptos admin UI package `rustok-channel-admin` с operator flow для policy authoring/edit/reorder/enable-disable и native-first `#[server]` + REST fallback transport parity;
- FBA provider boundary `ChannelReadPort` / `channel.read_projection.v1` для channel/default/host-target read projections, где `npm run verify:channel:fba` без компиляции фиксирует registry, static matrix и source-locked runtime fallback smoke (`embedded_native`, `rest_compatibility`, `unresolved_context`);
- source-locked proof points для `rustok-pages`, `rustok-blog` и `rustok-commerce`, где `npm run verify:channel:proof-points` удерживает использование resolved host `ChannelContext`, `channel_module_bindings`, metadata `channelSlugs` visibility и commerce channel snapshot без второго sales-channel домена.

## Интеграция

- используется `apps/server` как обязательный `Core` module и как runtime composition root;
- публикует shared host contract через `rustok-api` (`ChannelContext`, request-level metadata, `resolution_trace`);
- использует `rustok-auth` как источник истины для OAuth applications и access tokens;
- уже служит runtime proof point для `rustok-pages`, `rustok-blog` и `rustok-commerce`, а их source/docs синхронизация закреплена `npm run verify:channel:proof-points`.

## Проверка

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests для resolution order и explicit default semantics
- `npm run verify:channel:fba` для no-compile FBA registry/static-matrix/runtime-fallback-smoke guardrail
- `npm run verify:channel:resolution-contract` для no-compile guardrail порядка resolution и решения по built-in host fast-path
- `npm run verify:channel:proof-points` для no-compile guardrail текущих channel-aware proof points в `rustok-pages`, `rustok-blog` и `rustok-commerce`

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
