# Rust/UI Component Catalog

Список компонентов и решения по их реализации в дизайн-системе.

**Легенда:**

- `adopt` — реализован, используется в продакшне
- `pilot` — экспериментально реализован или планируется
- `defer` — отложено (нет текущей потребности)
- `reject` — не нужен (дублирует другое, не подходит архитектурно)

## Дизайн-система по приложениям

Все четыре приложения используют единую систему — shadcn CSS-переменные + Tailwind:

| Приложение | Стек | CSS entry point |
|-----------|------|----------------|
| `apps/admin` | Leptos CSR | `apps/admin/input.css` |
| `apps/next-admin` | Next.js / React | `apps/next-admin/src/styles/globals.css` |
| `apps/next-frontend` | Next.js / React | `apps/next-frontend/src/styles/globals.css` |
| `apps/storefront` | Leptos SSR | `apps/storefront/assets/input.css` |

Каждый CSS entry point определяет одинаковый набор shadcn custom properties и подключает Tailwind. Подробнее: `DECISIONS/2026-02-25-shared-design-system-shadcn-port.md`.

## Подход к реализации Leptos-компонентов

Leptos-компоненты (`UI/leptos/src/`) реализуются как **прямой порт Tailwind-классов из shadcn/ui** — без зависимости от внешних Leptos UI-библиотек.

**Важное отличие:** В `apps/admin` (Leptos) компоненты именуются в `snake_case` с префиксом `ui_` (например, `ui_button`) для соответствия стандартам Rust и предотвращения конфликтов. В `apps/next-admin` (React) сохраняется стандартный `PascalCase`.

Это обеспечивает:

- **Визуальный паритет** с Next.js admin — одинаковые классы, одинаковые CSS-переменные
- **Отсутствие внешних зависимостей** — нет риска несовместимости версий Leptos
- **Простоту обновления** — при обновлении shadcn в Next.js достаточно обновить строки классов

Для сложных интерактивных компонентов (Combobox, Dialog, DatePicker) используются Thaw/Leptonic как временное решение до реализации нативной версии.

## Статус компонентов

| Component | Decision | Target crate / file | Notes |
|-----------|----------|---------------------|-------|
| Accordion | defer | — | Нет текущей потребности |
| Alert | pilot | `iu-leptos` + `UI/next` | Планируется рядом с Badge |
| Alert Dialog | pilot | `iu-leptos` + `UI/next` | Нужен для confirm-диалогов |
| Animate | defer | — | CSS transitions достаточно |
| Avatar | adopt | `UI/next/components/Avatar.tsx` | Next.js; Leptos — `entities/user/ui` в admin |
| Badge | adopt | `UI/leptos/src/badge.rs`, `UI/next/components/Badge.tsx` | Полный паритет, 6 вариантов |
| Bottom Nav | defer | — | Не используется |
| Breadcrumb | adopt | `apps/next-admin/src/shared/ui/breadcrumbs.tsx` | Через shadcn |
| Button | adopt | `UI/leptos/src/button.rs`, `UI/next/components/Button.tsx` | Полный паритет, 6 вариантов |
| Card | adopt | `crates/leptos-ui/src/card.rs` | Leptos-only; Next.js — shadcn напрямую |
| Checkbox | adopt | `UI/leptos/src/checkbox.rs`, `UI/next/components/Checkbox.tsx` | Полный паритет |
| Chips | defer | — | Badge с dismissible покрывает |
| Combobox | pilot | — | Планируется, временно Leptonic |
| Command | adopt | `apps/next-admin` через kbar | KBar — command palette |
| Data Table | adopt | `apps/next-admin/src/widgets/data-table/` | @tanstack/react-table |
| Date Picker | defer | — | react-day-picker / shadcn DatePicker |
| Dialog | adopt | shadcn `Modal` | `apps/next-admin/src/components/ui/modal.tsx` |
| Dropdown Menu | adopt | shadcn `DropdownMenu` | В nav-user, org-switcher |
| Dropzone | adopt | `apps/next-admin/src/shared/ui/file-uploader.tsx` | react-dropzone |
| Form | adopt | `apps/next-admin/src/shared/ui/forms/` | 10 form-field компонентов |
| Input | adopt | `UI/leptos/src/input.rs`, `UI/next/components/Input.tsx` | Полный паритет |
| Label | adopt | `crates/leptos-ui/src/label.rs` | Leptos-only; Next.js — shadcn |
| Pagination | adopt | `apps/next-admin/src/widgets/data-table/` | DataTablePagination |
| Popover | adopt | shadcn `Popover` | Next.js only пока |
| Select | adopt | `UI/leptos/src/select.rs`, `UI/next/components/Select.tsx` | Полный паритет (native select) |
| Separator | adopt | `crates/leptos-ui/src/separator.rs` | Leptos-only; Next.js — shadcn |
| Skeleton | adopt | `UI/next/components/Skeleton.tsx` | Next.js only; Leptos — планируется |
| Spinner | adopt | `UI/leptos/src/spinner.rs`, `UI/next/components/Spinner.tsx` | Полный паритет, custom (нет в shadcn) |
| Switch | adopt | `UI/leptos/src/switch.rs`, `UI/next/components/Switch.tsx` | Полный паритет |
| Table | adopt | shadcn `Table` + `widgets/data-table/` | Next.js only |
| Tabs | adopt | shadcn `Tabs` | Next.js only |
| Textarea | adopt | `UI/leptos/src/textarea.rs`, `UI/next/components/Textarea.tsx` | Полный паритет |
| Toast | adopt | shadcn Sonner | Next.js only |
| Tooltip | adopt | shadcn `Tooltip` | Next.js only |

## Компоненты с полным паритетом Leptos ↔ Next.js

Следующие компоненты реализованы в `UI/leptos/src/` и `UI/next/components/` с одинаковым API и визуальным результатом благодаря общим shadcn CSS-переменным:

| Component | Leptos file | Variants / notes |
|-----------|------------|-----------------|
| `ui_button` | `src/button.rs` | Default, Destructive, Outline, Secondary, Ghost, Link |
| `ui_input` | `src/input.rs` | size sm/md/lg, invalid state |
| `ui_textarea` | `src/textarea.rs` | size sm/md/lg, invalid state |
| `ui_select` | `src/select.rs` | native `<select>`, size variants |
| `ui_checkbox` | `src/checkbox.rs` | controlled via `ReadSignal<bool>` |
| `ui_switch` | `src/switch.rs` | controlled, `SwitchSize::Sm|Md` |
| `ui_badge` | `src/badge.rs` | Default, Secondary, Destructive, Outline, Success, Warning |
| `ui_spinner` | `src/spinner.rs` | size sm/md/lg |

## Shared CSS-переменные (theming contract)

Все shadcn-приложения определяют одинаковый набор CSS-переменных:

```
apps/admin/input.css                        ← @import "../../UI/tokens/base.css" + shadcn vars
apps/next-admin/src/styles/globals.css      ← shadcn vars
apps/next-frontend/src/styles/globals.css   ← shadcn vars (sky-based primary palette)
apps/storefront/assets/input.css            ← shadcn vars (sky-based primary palette)
```

Переменные: `--background`, `--foreground`, `--card`, `--card-foreground`, `--primary`, `--primary-foreground`, `--secondary`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`, `--ring`, `--radius`.

`UI/tokens/base.css` содержит дополнительные токены (`--iu-radius-*`, `--iu-font-*`, `--iu-space-*`) которые не дублируют shadcn-переменные.

## Компоненты только в Next.js (ещё не портированы в Leptos)

- `Avatar` — в `UI/next/components/Avatar.tsx`; в Leptos используется `entities/user/ui`
- `Skeleton` — в `UI/next/components/Skeleton.tsx`
- Overlay-компоненты (Dialog, Popover, Tooltip, Sheet) — shadcn напрямую

## Следующие компоненты для порта в Leptos (приоритет)

| Приоритет | Компонент | Сложность | Обоснование |
|-----------|-----------|-----------|-------------|
| 1 | `Alert` | Низкая | CSS-only. Нужен для информационных сообщений в admin |
| 2 | `AlertDialog` | Средняя | Нужен для confirm-диалогов (удаление пользователей и т.д.) |
| 3 | `Tabs` | Низкая | CSS + JS для активного таба. Нужен для страниц настроек |
| 4 | `Breadcrumb` | Низкая | CSS-only. Оформить header breadcrumbs как переиспользуемый компонент |
| 5 | `Skeleton` | Низкая | CSS-only. Loading states уже нужны в dashboard/users |
| 6 | `Dialog` | Высокая | Focus trap, scroll lock — Leptonic как временное решение |
| 7 | `Combobox` | Высокая | Виртуальный скролл, keyboard nav — Leptonic/Thaw пока |

---

Последнее обновление: 2026-02-25.
