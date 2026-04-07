# Контракт `rustok-module.toml`

Этот документ описывает обязательный контракт path-модуля в RusTok. `modules.toml` определяет состав сборки платформы, а `rustok-module.toml` фиксирует publish-ready metadata, зависимости, UI wiring и локальные contract-артефакты конкретного модуля.

## Источник правды

- `modules.toml` отвечает за build composition и `depends_on`.
- `rustok-module.toml` отвечает за модульный metadata contract.
- `RusToKModule::dependencies()` отвечает за runtime dependency contract.

Для path-модуля эти три слоя должны совпадать по slug, версии и зависимостям.

## Обязательный минимум для каждого path-модуля

В каталоге `crates/<name>/` должны существовать:

- `Cargo.toml`
- `rustok-module.toml`
- `README.md`
- `docs/README.md`
- `docs/implementation-plan.md`

Если модуль публикует Leptos UI surfaces, дополнительно обязателен фактический subcrate и соответствующий manifest wiring:

- `admin/Cargo.toml` ↔ `[provides.admin_ui]`
- `storefront/Cargo.toml` ↔ `[provides.storefront_ui]`

## Минимальная структура `rustok-module.toml`

```toml
[module]
slug = "auth"
name = "Auth"
version = "0.1.0"
description = "Core authentication module for JWT, credentials, and token lifecycle"
ownership = "first_party"
trust_level = "core"
ui_classification = "capability_only"

[crate]
entry_type = "AuthModule"
```

## Секции контракта

### `[module]`

Обязательные поля:

- `slug`
- `name`
- `version`
- `description`
- `ownership`
- `trust_level`

Дополнительные поля:

- `ui_classification`
- `recommended_admin_surfaces`
- `showcase_admin_surfaces`

### `[crate]`

- `entry_type` — canonical Rust entry type модуля для registry/publish tooling.

### `[dependencies]`

- Описывает модульные зависимости в формате `slug = { version_req = ">=0.1.0" }`.
- По составу должен совпадать с `modules.toml.depends_on`.
- Runtime contract `RusToKModule::dependencies()` должен описывать тот же набор slug-ов.

### `[provides.*]`

Используется для publishable surfaces:

- `[provides.graphql]`
- `[provides.http]`
- `[provides.admin_ui]`
- `[provides.storefront_ui]`

Для UI surfaces host wiring валиден только при фактическом наличии subcrate и корректном `leptos_crate`.

### `[provides.*_ui.i18n]`

Используется для package-owned locale bundles:

- `default_locale`
- `supported_locales`
- `leptos_locales_path`

### `[settings]`, `[conflicts]`, `[marketplace]`

Эти секции optional, но если модуль ими пользуется, их данные проходят тем же validation path, что и базовый contract.

## `ui_classification`

`ui_classification` нужен для явной surface-классификации:

- `dual_surface`
- `admin_only`
- `storefront_only`
- `no_ui`
- `capability_only`
- `future_ui`

Правила:

- UI-классы должны совпадать с фактическим wiring через `[provides.admin_ui]` и `[provides.storefront_ui]`.
- Для модулей без UI допустимы `no_ui`, `capability_only` и `future_ui`.
- Отсутствие UI crate больше не считается достаточным implicit объяснением архитектуры модуля.

## Проверка

Обязательный локальный verification path:

- `cargo xtask module validate`
- `cargo xtask module test <slug>`
- `npm run verify:i18n:ui`
- `npm run verify:i18n:contract`
- `npm run verify:storefront:routes`
- `powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1` после установки Python

`cargo xtask module validate` должен fail-fast, если path-модуль:

- не имеет `rustok-module.toml`;
- не имеет `docs/README.md` или `docs/implementation-plan.md`;
- имеет broken UI wiring;
- имеет drift между `modules.toml` и `[dependencies]`;
- не отражён корректными ссылками в `docs/modules/_index.md`.

## Связанные документы

- [План унификации module-system](./module-system-plan.md)
- [Индекс документации по модулям](./_index.md)
- [Карта модулей и владельцев](./registry.md)
- [Исследование по единому стандарту модулей](../research/deep-research-modules.md)
