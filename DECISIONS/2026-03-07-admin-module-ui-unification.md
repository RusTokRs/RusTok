# Унификация UI модулей между Next.js и Leptos Admin

- Date: 2026-03-07
- Status: Accepted & Implemented (v2 — с библиотеками i18n)

## Context

Модуль управления модулями (Modules page) был реализован в двух admin-панелях
(Next.js и Leptos), но с расхождениями в i18n и структуре компонентов.

В итерации v1 были добавлены самописные i18n-решения. В v2 они заменены на
**полноценные библиотеки**, которые уже были подобраны в зависимостях проекта:

- `leptos_i18n` = 0.6.0 (уже был в workspace Cargo.toml)
- `next-intl` = 4.0.0 (уже был в next-frontend)

## Decision

### 1. Библиотеки i18n

| Admin | Библиотека | API | Хранение locale |
|---|---|---|---|
| Leptos | `leptos_i18n` 0.6 | `t!(i18n, key.sub)`, `t_string!()` | Cookie (библиотека) |
| Next.js | `next-intl` 4.0 | `useTranslations('ns')`, `getTranslations()` | Cookie `rustok-admin-locale` |

**Leptos admin**:
```
build.rs                     — генерация i18n модуля (Config + TranslationsInfos)
src/lib.rs                   — include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"))
locales/en.json, ru.json     — nested JSON (source of truth)
```

Компоненты используют:
```rust
use crate::{t, t_string, use_i18n, Locale, I18nContextProvider};
let i18n = use_i18n();
view! { <span>{t!(i18n, modules.title)}</span> }
```

**Next.js admin**:
```
next.config.ts               — createNextIntlPlugin('./src/i18n/request.ts')
src/i18n/request.ts          — getRequestConfig (locale from cookie)
src/app/layout.tsx            — <NextIntlClientProvider>
messages/en.json, ru.json    — nested JSON (copy of Leptos locales)
```

Компоненты используют:
```tsx
import { useTranslations } from 'next-intl';
const t = useTranslations('modules');
return <span>{t('title')}</span>;
```

### 2. Единые locale-файлы (nested JSON)

Файлы конвертированы из плоского формата (`"modules.title": "..."`) в вложенный:
```json
{
  "modules": {
    "title": "Modules",
    "section": {
      "core": "Core Modules",
      "optional": "Optional Modules"
    }
  }
}
```

Оба стека: `apps/admin/locales/` и `apps/next-admin/messages/` — одинаковые файлы.

### 3. FSD-структура компонентов (единая)

```
# Leptos Admin                    # Next.js Admin
features/modules/                  features/modules/
├── api.rs                         ├── api.ts
├── mod.rs                         └── components/
└── components/                        ├── module-card.tsx
    ├── mod.rs                         └── modules-list.tsx
    ├── module_card.rs
    └── modules_list.rs
```

### 4. Матрица соответствия

| i18n Key | Leptos | Next.js |
|---|---|---|
| `modules.section.core` | `t!(i18n, modules.section.core)` | `t('section.core')` |
| `modules.badge.core` | `t!(i18n, modules.badge.core)` | `t('badge.core')` |
| `modules.toast.enabled` | `t_string!(i18n, modules.toast.enabled)` | `t('toast.enabled')` |
| `modules.title` | `t_string!(i18n, modules.title)` | `t('title')` |

> Leptos: абсолютные ключи через `t!()` / `t_string!()`.
> Next.js: относительные ключи через `useTranslations('modules')`.

## Consequences

### Позитивные

- **Compile-time safety** (Leptos): `t!()` проверяет ключи при компиляции.
- **IDE autocompletion** (Next.js): `next-intl` TypeScript поддержка.
- **Единый UX**: одинаковые тексты, одинаковые locale-файлы.
- **Проверенные библиотеки**: не самописные решения, а ecosystem-стандарты.

### Негативные

- **Дублирование JSON**: всё ещё в двух местах.
  Митигация: CI-проверка, или `@rustok/admin-locales` workspace package.
- **Разный API**: `t!(i18n, key)` vs `t('key')` — неизбежно из-за разных языков.

### Follow-up

1. Применить `useTranslations()` к остальным страницам Next.js admin.
2. Все `t_string!()` в Leptos admin уже применены ко всем 16 файлам.
3. CI: проверка совпадения ключей между locales.
4. `leptos_i18n` автоматически сохраняет locale в cookie — синхронизация с `next-intl`.
