# Каталог Rust UI-компонентов

Этот документ фиксирует текущий shared UI surface в RusToK и разделение ответственности между `UI/*`, `crates/leptos-ui` и app-local компонентами.

## Источники shared UI

В репозитории сейчас есть три уровня UI-переиспользования:

- `UI/tokens` — общие design tokens и базовые CSS-переменные;
- `UI/leptos` и `UI/next/components` — параллельные shared primitives для Leptos и Next.js;
- `crates/leptos-ui` — RusToK-специфичный Leptos package boundary с реэкспортами и локальными helper-компонентами.

App-local сложные компоненты остаются внутри конкретных host-приложений и не считаются частью shared catalog, пока не появился повторно используемый contract.

## Shared design contract

- Все host-приложения используют единый theming contract на базе shared tokens и shadcn-compatible CSS variables.
- Leptos и Next.js компоненты должны сохранять parity на уровне назначения, визуального результата и базового API, но не обязаны иметь буквальное one-to-one устройство.
- Shared UI packages остаются presentational слоем и не владеют transport, auth, routing или domain behavior.

## Shared primitives: `UI/leptos` ↔ `UI/next/components`

Текущий набор компонентов, для которых уже существует явный shared surface:

| Primitive | Leptos | Next.js | Статус |
|-----------|--------|---------|--------|
| Alert | `UI/leptos/src/alert.rs` | app-local / shadcn path | Leptos canonical |
| Badge | `UI/leptos/src/badge.rs` | `UI/next/components/Badge.tsx` | parity |
| Button | `UI/leptos/src/button.rs` | `UI/next/components/Button.tsx` | parity |
| Checkbox | `UI/leptos/src/checkbox.rs` | `UI/next/components/Checkbox.tsx` | parity |
| Input | `UI/leptos/src/input.rs` | `UI/next/components/Input.tsx` | parity |
| Select | `UI/leptos/src/select.rs` | `UI/next/components/Select.tsx` | parity |
| Spinner | `UI/leptos/src/spinner.rs` | `UI/next/components/Spinner.tsx` | parity |
| Switch | `UI/leptos/src/switch.rs` | `UI/next/components/Switch.tsx` | parity |
| Textarea | `UI/leptos/src/textarea.rs` | `UI/next/components/Textarea.tsx` | parity |
| Avatar | отсутствует в shared Leptos surface | `UI/next/components/Avatar.tsx` | Next-only |
| Skeleton | отсутствует в shared Leptos surface | `UI/next/components/Skeleton.tsx` | Next-only |

`UI/leptos/src/lib.rs` и `UI/next/components/index.ts` являются входными точками этого shared primitive layer.

## Leptos-specific package boundary: `crates/leptos-ui`

`crates/leptos-ui` держит RusToK-специфичный Leptos surface для приложений и module-owned UI packages. Текущие entry points:

- `Button`
- `Input`
- `Badge`
- `Alert`
- `Card`
- `Label`
- `Separator`
- `LanguageToggle`

Этот crate нужен там, где простой shared primitive layer недостаточен и требуется стабильная package boundary внутри Rust workspace.

## App-local UI, который не входит в shared catalog

Следующие поверхности пока остаются app-local и не должны автоматически считаться частью общего каталога:

- `apps/next-admin/src/shared/ui/*`
- `apps/next-admin` data-table и related admin-only widgets
- `apps/admin` host-local layout/navigation components
- module-owned admin/storefront UI inside `crates/rustok-*/admin` и `crates/rustok-*/storefront`

Если такой компонент начинает повторно использоваться в нескольких host-ах или модулях, его нужно либо поднять в `UI/*`, либо оформить через `crates/leptos-ui` для Leptos-пути.

## Проверка при изменении shared UI

- сверять `UI/leptos` и `UI/next/components` на предмет API drift;
- проверять, что shared компоненты не затягивают domain-specific зависимости;
- обновлять app-local docs, если меняется host integration contract;
- обновлять [UI index](./README.md) и связанные app docs, если меняется граница shared vs app-local UI.

## Связанные документы

- [UI index](./README.md)
- [GraphQL architecture](./graphql-architecture.md)
- [Leptos admin docs](../../apps/admin/docs/README.md)
- [Leptos storefront docs](../../apps/storefront/docs/README.md)
- [Next.js admin docs](../../apps/next-admin/docs/README.md)
- [Next.js storefront docs](../../apps/next-frontend/docs/README.md)
