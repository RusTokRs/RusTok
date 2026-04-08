# Быстрый старт по UI-пакетам модулей

Этот быстрый старт нужен для создания или дооформления module-owned UI-поверхности без
старого шума вокруг установки и деплоя. Канонический путь начинается с локальных docs
модуля и manifest wiring, а не с host-specific хаков.

## Что должно получиться

У модуля с UI к концу этого прохода должны быть:

- root `README.md` на английском;
- `docs/README.md` на русском;
- `docs/implementation-plan.md` на русском;
- `rustok-module.toml` с корректным `[provides.admin_ui]` и/или
  `[provides.storefront_ui]`;
- `admin/` и/или `storefront/` sub-crate, если UI реально поставляется;
- проходящий `cargo xtask module validate <slug>`.

## Шаг 1. Оформите контракт документации

Перед UI wiring модуль должен получить минимальный docs-standard:

- root `README.md` с `Purpose`, `Responsibilities`, `Entry points`,
  `Interactions` и ссылкой на `docs/README.md`;
- локальный `docs/README.md` с разделами `Назначение`, `Зона ответственности`,
  `Интеграция`, `Проверка`, `Связанные документы`;
- локальный `docs/implementation-plan.md` с минимумом `Фокус` и `Улучшения`.

Если модуль уже существует, сначала обновляется локальная документация, потом
добавляется или меняется UI wiring.

## Шаг 2. Определите ownership UI

До создания UI-пакета зафиксируйте:

- это module-owned admin-поверхность, storefront-поверхность или оба варианта;
- какой host будет монтировать пакет: `apps/admin`, `apps/storefront`,
  `apps/next-admin` или `apps/next-frontend`;
- нужен ли только Leptos UI, или также Next.js host integration;
- есть ли package-owned locale bundles, которые нужно объявить через manifest.

Host application не должен становиться владельцем этой UI-функциональности.

## Шаг 3. Добавьте manifest wiring

В `rustok-module.toml` укажите только реально существующие UI surfaces.

Пример для admin UI:

```toml
[provides.admin_ui]
leptos_crate = "rustok-blog-admin"
route_segment = "blog"
nav_label = "Blog"
```

Пример для storefront UI:

```toml
[provides.storefront_ui]
leptos_crate = "rustok-blog-storefront"
route_segment = "blog"
page_title = "Blog"
slot = "home_after_catalog"
```

Если UI sub-crate объявлен в manifest, `admin/Cargo.toml` или
`storefront/Cargo.toml` должен реально существовать и совпадать по версии с
основным модулем.

## Шаг 4. Реализуйте UI surface

Для Leptos module-owned UI действует такой baseline:

- internal data layer по умолчанию строится на `#[server]` functions;
- GraphQL не удаляется и остаётся параллельным transport-контрактом;
- locale берётся из host/runtime-контракта, а не из локальных cookie/header/query
  fallback chains;
- UI package не тащит в себя ownership доменной логики, который должен жить в
  самом модуле.

Для Next.js host integration:

- модуль публикует package-owned UI surface или host-specific integration layer;
- сам доменный контракт остаётся у module crate и server/API-слоя;
- host only mounts, routes and composes.

## Шаг 5. Обновите локальные docs

После появления UI wiring синхронно обновите:

- `README.md` модуля;
- `docs/README.md` модуля;
- `docs/implementation-plan.md`, если UI слой меняет roadmap;
- при необходимости `admin/README.md` или `storefront/README.md`.

Central docs в `docs/modules/*` обновляются только после того, как актуальны
локальные docs модуля.

## Шаг 6. Проверьте модуль точечно

Минимальный локальный прогон:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

Если затронут host/UI layer, дополнительно обычно нужны:

```powershell
npm.cmd run verify:i18n:ui
npm.cmd run verify:i18n:contract
npm.cmd run verify:storefront:routes
```

На Windows architecture guard запускается через:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify/verify-architecture.ps1
```

## Что не делать

- не описывать UI-пакет только в `apps/*`;
- не оставлять `admin/` или `storefront/` без manifest wiring;
- не вводить отдельный i18n-контракт на уровне UI package;
- не считать старые инструкции по установке и деплою canonical source of truth;
- не заменять GraphQL на `#[server]` и не заменять `#[server]` на GraphQL там,
  где нужен параллельный transport-контракт.

## Куда идти дальше

- [Индекс UI-пакетов модулей](./UI_PACKAGES_INDEX.md)
- [Контракт `rustok-module.toml`](./manifest.md)
- [Шаблон документации модуля](../templates/module_contract.md)
- [GraphQL и Leptos server functions](../UI/graphql-architecture.md)
- [UI README](../UI/README.md)
