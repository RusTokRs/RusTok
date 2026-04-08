# Шаблон документации модуля

Этот шаблон нужен для новых platform modules, а также для support/capability crates, которые хотят соответствовать текущему documentation contract RusToK.

Нормативный путь для module-level documentation такой:

- корневой `README.md` рядом с кодом;
- локальный `docs/README.md`;
- локальный `docs/implementation-plan.md`;
- при необходимости `rustok-module.toml`.

Не создавайте отдельный central doc для каждого модуля в `docs/modules/`. Central docs должны ссылаться на локальную документацию, а не дублировать её.

## 1. Минимальный набор файлов

Для нового path-модуля ожидается следующий набор:

```text
crates/rustok-<slug>/
  Cargo.toml
  README.md
  rustok-module.toml
  docs/
    README.md
    implementation-plan.md
```

Для support/capability crate `rustok-module.toml` не обязателен, если crate не входит в `modules.toml`.

## 2. Корневой `README.md`

Корневой README должен быть на английском и содержать этот каркас:

```md
# rustok-<slug>

## Purpose

One short paragraph explaining what this crate owns.

## Responsibilities

- Responsibility 1
- Responsibility 2
- Responsibility 3

## Entry points

- `MainType`
- `MainService`
- `controllers::routes`

## Interactions

- Interaction with `apps/server`
- Interaction with other modules/crates
- Notes about UI packages or runtime wiring

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
```

Правила:

- один файл — один язык;
- `README.md` не заменяет локальные docs;
- `Docs` section обязателен;
- названия разделов должны совпадать с contract-формой:
  - `## Purpose`
  - `## Responsibilities`
  - `## Entry points`
  - `## Interactions`

## 3. Локальный `docs/README.md`

Локальный docs README пишется на русском и описывает живой модульный контракт.

Минимальный каркас:

```md
# <Название модуля>

## Назначение

Кратко: что модуль делает и почему он существует.

## Зона ответственности

- Чем модуль владеет
- Чем модуль сознательно не владеет

## Интеграция

- GraphQL / REST / фоновые задачи / UI-поверхности
- host wiring и runtime boundaries
- зависимости на другие модули и crate-ы
- особенно важные кросс-модульные контракты

## Проверка

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`
- другие точечные команды при необходимости

## Связанные документы

- `implementation-plan.md`
- central docs
- соседние host/module docs
```

Допустимы дополнительные разделы, если они реально нужны модулю:

- `## Настройки и конфигурация`
- `## Health и observability`
- `## Ограничения`
- `## UI contract`

Но минимальные разделы выше должны оставаться на месте.

## 4. Локальный `docs/implementation-plan.md`

Этот файл фиксирует живой план доведения модуля до целевого состояния, а не подробную историю работ.

Минимальный каркас:

```md
# План развития <модуля>

## Область работ

Коротко: на чём сосредоточен текущий план.

## Текущее состояние

Коротко: что уже стабилизировано и какие инварианты модуль уже держит.

## Этапы

### 1. Ближайший срез

- ...

## Проверка

- `cargo xtask module validate <slug>`
- `cargo xtask module test <slug>`

## Правила обновления

1. При изменении runtime/module contract сначала обновлять этот файл.
2. При изменении public surface синхронизировать `README.md` и `docs/README.md`.
3. При изменении manifest metadata синхронизировать `rustok-module.toml`.
```

Допустимы дополнительные разделы:

- `## Риски и открытые вопросы`
- `## Приоритеты`
- `## Критерии готовности`

Но `## Область работ`, `## Текущее состояние`, `## Этапы`, `## Проверка` и `## Правила обновления` должны присутствовать как минимальный стандарт.

## 5. `rustok-module.toml`

Для path-модуля из `modules.toml` локальный manifest обязателен.

Минимальный каркас:

```toml
[module]
slug = "<slug>"
name = "<Name>"
version = "0.1.0"
description = "At least one publish-ready sentence."
ownership = "platform"
trust_level = "first-party"

[crate]
entry_type = "<PascalSlug>Module"
```

Дальше по необходимости добавляются:

- `[provides.graphql]`
- `[provides.http]`
- `[provides.admin_ui]`
- `[provides.storefront_ui]`
- `[settings]`
- `[marketplace]`

Подробный contract-слой описан в [docs/modules/manifest.md](../modules/manifest.md).

## 6. Обязательная локальная проверка

Для нового или существенно изменённого platform module:

```powershell
cargo xtask module validate <slug>
cargo xtask module test <slug>
```

Если меняется состав `modules.toml`, добавляется:

```powershell
cargo xtask validate-manifest
```

Минимальный Windows verification path описан в [docs/verification/README.md](../verification/README.md).

## 7. Что не делать

- не писать root `README.md` на русском;
- не хранить единственную документацию модуля только в `docs/modules/`;
- не добавлять path-модуль в `modules.toml` без `rustok-module.toml`;
- не считать подпапки `admin/` и `storefront/` доказательством интеграции без manifest wiring;
- не превращать local docs в исторический changelog, если нужен живой contract.

## 8. Связанные документы

- [Карта документации](../index.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Контракт manifest-слоя](../modules/manifest.md)
- [Индекс локальной документации по модулям](../modules/_index.md)
