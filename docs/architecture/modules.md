# Module Architecture

RusToK реализован как modular monolith: platform modules компилируются в общий runtime и обслуживаются единым composition root в `apps/server`.

## Архитектурная классификация

У platform modules есть только две категории:

- `ModuleKind::Core`
- `ModuleKind::Optional`

`Core` modules всегда активны и не могут отключаться для tenant'а.
`Optional` modules могут входить в сборку и затем включаться или отключаться на уровне tenant lifecycle.

## Source of truth

- декларация состава модулей: `modules.toml`
- runtime registration: `apps/server/src/modules/mod.rs`
- manifest/runtime validation: `apps/server/src/modules/manifest.rs`
- базовые контракты: `crates/rustok-core/src/module.rs`

## Текущий platform baseline

### Core modules

- `auth`
- `cache`
- `channel`
- `email`
- `index`
- `search`
- `outbox`
- `tenant`
- `rbac`

### Optional modules

- `content`
- `commerce`
- `profiles`
- `blog`
- `forum`
- `pages`
- `media`
- `workflow`

## Wiring — это не taxonomy

Разные модульные поверхности могут подключаться по-разному:

- через `ModuleRegistry`
- через manifest/codegen wiring
- через host wiring для UI
- через bootstrap/runtime services

Эти способы подключения не вводят новые архитектурные типы.

`rustok-outbox` — `Core` module даже в тех местах, где сервер использует его напрямую для event runtime bootstrap.

## Crate role vs module status

`crate` — техническая единица Cargo.

Поэтому в `crates/` одновременно живут:

- module-crates
- shared libraries
- infrastructure/support crates

Например:

- `rustok-outbox` — platform module (`Core`)
- `rustok-core` — shared library/platform contract crate
- `rustok-events` — shared event contract crate
- `alloy` и `alloy-scripting` — capability crates

## Platform-level install/uninstall

Изменение `modules.toml` меняет состав platform modules в сборке:

1. manifest обновляется;
2. build pipeline пересобирает runtime;
3. новый артефакт поднимается уже с новым составом модулей.

Это platform-level операция.

## Tenant-level enable/disable

Tenant lifecycle работает только для `Optional` modules, уже присутствующих в сборке:

1. проверяются dependency edges;
2. обновляется `tenant_modules`;
3. вызываются lifecycle hooks модуля.

`Core` modules не проходят через этот toggle flow.

## Alloy

Alloy не входит в taxonomy `Core/Optional` platform modules:

- `alloy-scripting` — runtime capability
- `alloy` — API/transport shell

Alloy может использоваться workflow/MCP как capability, но не становится tenant-toggle module.
