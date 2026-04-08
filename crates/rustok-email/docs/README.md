# Документация `rustok-email`

`rustok-email` — core-модуль доставки писем платформы. Он держит SMTP transport,
typed email rendering и delivery helpers для auth и operational notification flow.

## Назначение

- публиковать канонический runtime entry type `EmailModule`;
- держать SMTP transport и email rendering вне host-слоя;
- давать платформе единый delivery contract для typed email payloads.

## Зона ответственности

- SMTP configuration и sender wiring на уровне модуля;
- typed rendering contract для password reset и соседних email flows;
- delivery abstractions и email-related error model;
- отсутствие собственной RBAC vocabulary и UI surface.

## Интеграция

- зависит от `rustok-core` и shared libraries;
- используется `apps/server` для auth lifecycle и operational notification path;
- не публикует собственный UI и остаётся `ui_classification = "capability_only"`;
- любые admin-facing actions, которые триггерят отправку писем, авторизуются в вызывающем модуле, а не в `rustok-email`.

## Проверка

- `cargo xtask module validate email`
- `cargo xtask module test email`
- targeted host tests для auth/email delivery flows при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План реализации](./implementation-plan.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
