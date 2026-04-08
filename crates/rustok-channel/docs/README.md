# Документация `rustok-channel`

`rustok-channel` — experimental core-модуль, который вводит platform-level
channel context для delivery surfaces и channel-aware runtime resolution.

## Назначение

- публиковать канонический runtime entry type `ChannelModule`;
- держать channel resolution logic внутри модуля, а не в `apps/server`;
- давать платформе единый channel-aware contract для host runtime и domain consumers.

## Зона ответственности

- storage для `channels`, `channel_targets`, `channel_module_bindings`, `channel_oauth_apps`;
- domain-owned resolution layer: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, `ChannelResolver`;
- tenant-scoped typed resolution policies и explicit default channel semantics;
- module-owned Leptos admin UI package `rustok-channel-admin`.

## Интеграция

- используется `apps/server` как обязательный `Core` module и как runtime composition root;
- публикует shared host contract через `rustok-api` (`ChannelContext`, request-level metadata);
- использует `rustok-auth` как источник истины для OAuth applications и access tokens;
- уже служит runtime proof point для `rustok-pages`, `rustok-blog` и `rustok-commerce`.

## Проверка

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests для resolution order и explicit default semantics

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
