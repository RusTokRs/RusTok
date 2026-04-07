# План реализации `rustok-product`

`rustok-product` — scoped-подмодуль ecommerce family. Локальный план нужен для единого формата модульной документации; orchestration-контекст и cross-module roadmap остаются в umbrella-плане `rustok-commerce`.

## Текущее состояние

- модуль зарегистрирован в `modules.toml` и имеет `rustok-module.toml`;
- локальная документация и README считаются обязательным acceptance contract;
- transport и UI surfaces по-прежнему агрегируются через `rustok-commerce`, если для сценария не выделен отдельный host-owned path.

## Ближайшие задачи сопровождения

- держать sync между `modules.toml`, runtime dependencies и `[dependencies]` в manifest-слое;
- выносить domain logic из umbrella-модуля только вместе с локальной документацией и targeted tests;
- не публиковать отдельный UI surface без явного manifest wiring и host integration.

## Проверка

- `cargo xtask module validate product`
- `cargo xtask module test product`
- related commerce verification при изменении orchestration boundary

## Связанные документы

- [README crate](../README.md)
- [Документация umbrella-модуля](../../rustok-commerce/docs/README.md)
- [План umbrella-модуля](../../rustok-commerce/docs/implementation-plan.md)
