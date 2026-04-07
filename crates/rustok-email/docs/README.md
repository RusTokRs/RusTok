# Документация `rustok-email`

`rustok-email` — core-модуль почтовой доставки платформы. Он держит SMTP transport, шаблоны писем и typed delivery contracts, которыми пользуются auth и операционные notification flows.

## Что находится внутри модуля

- `EmailModule` и runtime metadata для `ModuleRegistry`
- SMTP config и transport wiring
- typed email payloads и template rendering
- delivery abstractions для auth и notification paths

## Границы и взаимодействия

- `rustok-email` не владеет UI и фиксируется как `ui_classification = "capability_only"`.
- Модуль не публикует собственный RBAC surface; права на запуск конкретных email flows принадлежат вызывающим модулям и `apps/server`.
- `apps/server` собирает transport adapters и orchestration, а сам модуль остаётся reusable delivery boundary.

## Проверка

- `cargo xtask module validate email`
- `cargo xtask module test email`
- targeted server/auth flows, если меняется password-reset или invite delivery contract

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
