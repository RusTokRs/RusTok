# Контракт `modules.toml` и `rustok-module.toml`

Этот документ описывает два связанных слоя модульного контракта RusToK:

- `modules.toml` в корне репозитория задаёт состав платформенных модулей для конкретной сборки.
- `rustok-module.toml` внутри path-модуля задаёт publish/runtime/UI-контракт самого модуля.

`modules.toml` отвечает за composition root. `rustok-module.toml` отвечает за identity, surface wiring, UI-пакеты и publish-ready metadata.

## Где какой контракт живёт

### `modules.toml`

Корневой manifest фиксирует:

- список платформенных модулей, попадающих в сборку;
- источник каждого модуля: `path`, `git`, `crates-io`/`registry`;
- coarse-grained зависимости через `depends_on`;
- platform-level settings, включая `settings.default_enabled`.

Это runtime/build-level контракт всей платформы, а не отдельного crate.

### `rustok-module.toml`

Локальный manifest path-модуля фиксирует:

- `module.slug`, `module.name`, `module.version`, `module.description`;
- runtime-точки входа модуля;
- admin/storefront UI wiring;
- module-owned schema настроек;
- marketplace/publish metadata;
- зависимости и конфликты, относящиеся к самому модулю.

Для path-модулей из `modules.toml` наличие `rustok-module.toml` обязательно.

## Обязательный минимум для path-модуля

Каждый path-модуль из `modules.toml` должен иметь:

- `Cargo.toml`;
- корневой `README.md` на английском;
- `docs/README.md` на русском;
- `docs/implementation-plan.md` на русском;
- `rustok-module.toml`.

Корневой `README.md` считается частью acceptance contract и должен содержать:

- `## Purpose`
- `## Responsibilities`
- `## Entry points`
- `## Interactions`
- ссылку на локальный `docs/README.md`

Локальные docs нужны даже для модулей без admin/storefront UI.

### Минимальный контракт документации

Для path-модуля контракт документации считается закрытым, только если соблюдены оба слоя:

- корневой `README.md` на английском с разделами `Purpose`, `Responsibilities`, `Entry points`, `Interactions` и ссылкой на `docs/README.md`;
- локальный `docs/README.md` на русском как живой runtime/module contract;
- локальный `docs/implementation-plan.md` на русском как живой план доведения модуля до целевого состояния.

Минимальный каркас локального `docs/README.md`:

- `## Назначение`
- `## Зона ответственности`
- `## Интеграция`
- `## Проверка`
- `## Связанные документы`

Минимальный каркас локального `docs/implementation-plan.md`:

- `## Область работ`
- `## Текущее состояние`
- `## Этапы`
- `## Проверка`
- `## Правила обновления`

Дополнительные разделы допустимы, но этот минимум должен сохраняться.

## Что проверяет `cargo xtask module validate`

`cargo xtask module validate <slug>` работает только для slug из `modules.toml` и валидирует фактический scoped contract:

- slug существует в `modules.toml`;
- для `source = "path"` задан `path`;
- `rustok-module.toml` существует по ожидаемому пути;
- `module.slug` совпадает со slug из `modules.toml`;
- `module.version` в `rustok-module.toml` совпадает с версией из `Cargo.toml`;
- `package.license` резолвится через `Cargo.toml` или workspace inheritance;
- `module.description` достаточно полон для publish readiness;
- root `README.md`, `docs/README.md` и `docs/implementation-plan.md` присутствуют и соответствуют минимальному формату;
- wiring для `admin/` и `storefront/` согласован с `[provides.admin_ui]` и `[provides.storefront_ui]`;
- если UI sub-crate объявлен в manifest, его `Cargo.toml` реально существует и версия совпадает с версией основного модуля.

Если slug отсутствует в `modules.toml`, `xtask` возвращает `Unknown module slug`.

## Что проверяет `cargo xtask validate-manifest`

`cargo xtask validate-manifest` проверяет центральный composition contract:

- `modules.toml` парсится и использует поддерживаемую schema version;
- `default_enabled` ссылается только на реально объявленные модули;
- `depends_on` не содержит отсутствующих slug;
- `source`-спецификация валидна для каждого модуля;
- все path-модули действительно содержат `rustok-module.toml`.

Этот шаг не заменяет `cargo xtask module validate <slug>`, а дополняет его.

## Минимальный пример `modules.toml`

```toml
schema = 2
app = "rustok-server"

[modules]
blog = { crate = "rustok-blog", source = "path", path = "crates/rustok-blog", depends_on = ["content"] }
content = { crate = "rustok-content", source = "path", path = "crates/rustok-content" }

[settings]
default_enabled = ["content", "blog"]
```

## Минимальный пример `rustok-module.toml`

```toml
[module]
slug = "blog"
name = "Blog"
version = "0.1.0"
description = "Blog module with admin and storefront surfaces."
ownership = "platform"
trust_level = "first-party"

[crate]
entry_type = "BlogModule"

[provides.graphql]
query = "graphql::BlogQuery"
mutation = "graphql::BlogMutation"

[provides.http]
routes = "controllers::routes"

[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
route_segment = "blog"
nav_label = "Blog"

[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"

[marketplace]
category = "content"
publisher = "rustok"
tags = ["blog", "editorial"]
description = "Blog module with admin and storefront surfaces."
```

## Инварианты для UI sub-crates

- Наличие `admin/Cargo.toml` без `[provides.admin_ui].leptos_crate` считается ошибкой wiring.
- Наличие `storefront/Cargo.toml` без `[provides.storefront_ui].leptos_crate` считается ошибкой wiring.
- Объявление `[provides.admin_ui].leptos_crate` без реального `admin/Cargo.toml` считается ошибкой.
- Объявление `[provides.storefront_ui].leptos_crate` без реального `storefront/Cargo.toml` считается ошибкой.
- Версии UI sub-crates должны совпадать с версией основного модуля.

Само наличие подпапки `admin/` или `storefront/` не считается доказательством интеграции. Канонический источник правды здесь — manifest wiring.

## Support и capability crates

Не каждый crate из workspace является платформенным модулем.

- Platform modules живут в `modules.toml` и проходят scoped validation через `cargo xtask module validate <slug>`.
- Foundation/shared/support/capability crates могут иметь локальные docs и собственные контракты, но не обязаны иметь slug в `modules.toml`.

Для таких crates всё равно действует documentation minimum:

- корневой `README.md`;
- при необходимости `docs/README.md`;
- при необходимости `docs/implementation-plan.md`.

Если support/capability crate уже публикует локальные docs, для него рекомендуется тот же структурный стандарт, что и для платформенных модулей: английский root `README.md`, русский `docs/README.md`, русский `docs/implementation-plan.md`.

Но они не проходят `module validate`, пока не становятся платформенным модулем.

## Рекомендуемый локальный preflight

Для path-модуля перед публикацией или серьёзной доработкой используйте:

```powershell
cargo xtask module validate blog
cargo xtask module test blog
```

Если меняется весь composition contract платформы, добавляйте:

```powershell
cargo xtask validate-manifest
```

## Связанные документы

- [План модульной системы](./module-system-plan.md)
- [Реестр модулей и приложений](./registry.md)
- [Реестр crate-ов модульной платформы](./crates-registry.md)
- [Индекс локальной документации по модулям](./_index.md)
- [Шаблон документации модуля](../templates/module_contract.md)
- [Главный README по верификации](../verification/README.md)

> Статус документа: актуальный. При изменении правил `xtask`, acceptance-контракта для модулей или состава платформенных модулей обновляйте этот файл вместе с `docs/index.md`.
