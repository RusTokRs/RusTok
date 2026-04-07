# План реализации `rustok-email`

Статус: core baseline зафиксирован, модуль приведён к единому manifest/doc contract.

## Что уже должно оставаться стабильным

- SMTP transport и template rendering остаются модульной ответственностью.
- `apps/server` использует модуль как host integration point, а не как место для дублирования email-логики.
- README, локальная документация и `rustok-module.toml` являются обязательной частью acceptance contract.

## Ближайшие задачи сопровождения

- удерживать typed email contracts согласованными с auth lifecycle;
- расширять template surface только вместе с documented locale/i18n expectations;
- не вводить package-local fallback chains для locale selection.

## Проверка

- `cargo xtask module validate email`
- `cargo xtask module test email`
- targeted integration checks для password-reset/invite email flow при изменении delivery contract
