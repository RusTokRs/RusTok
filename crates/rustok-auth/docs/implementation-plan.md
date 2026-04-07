# План реализации `rustok-auth`

Статус: core baseline зафиксирован, модуль приведён к единому manifest/doc contract.

## Что уже должно оставаться стабильным

- `AuthModule` зарегистрирован как обязательный core-модуль.
- JWT, credential hashing и token lifecycle живут внутри модуля, а не в host-слое.
- README, локальная документация и `rustok-module.toml` входят в обязательный acceptance contract.

## Ближайшие задачи сопровождения

- удерживать sync между runtime RBAC surface и `apps/server` integration tests;
- не выносить auth business logic обратно в `apps/server`;
- расширять локальную документацию и verification только вместе с реальными изменениями public/runtime contract.

## Проверка

- `cargo xtask module validate auth`
- `cargo xtask module test auth`
- targeted server tests для auth/RBAC contracts при изменении runtime wiring
