# Unification of UI modules between Next.js and Leptos Admin

- Date: 2026-03-07
- Status: Accepted & Implemented (v2 — with i18n libraries)

## Context

The module management page (Modules page) was implemented in two admin panels
(Next.js and Leptos), but with divergences in i18n and component structure.

In iteration v1, custom i18n solutions were added. In v2, they were replaced with
**full-featured libraries** that were already present in the project's dependencies:

- `leptos_i18n` = 0.6.0 (already in workspace Cargo.toml)
- `next-intl` = 4.0.0 (already in next-frontend)

## Decision

### 1. i18n Libraries

| Admin | Library | API | Locale storage |
|---|---|---|---|
| Leptos | `leptos_i18n` 0.6 | `t!(i18n, key.sub)`, `t_string!()` | Cookie (library) |
| Next.js | `next-intl` 4.0 | `useTranslations('ns')`, `getTranslations()` | Cookie `rustok-admin-locale` |

**Leptos admin**:
```
build.rs                     — i18n module generation (Config + TranslationsInfos)
src/lib.rs                   — include!(concat!(env!("OUT_DIR"), "/i18n/mod.rs"))
locales/en.json, ru.json     — nested JSON (source of truth)
```

Components use:
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

Components use:
```tsx
import { useTranslations } from 'next-intl';
const t = useTranslations('modules');
return <span>{t('title')}</span>;
```

### 2. Unified locale files (nested JSON)

Files are converted from flat format (`"modules.title": "..."`) to nested:
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

Both stacks: `apps/admin/locales/` and `apps/next-admin/messages/` — identical files.

### 3. FSD component structure (unified)

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

### 4. Correspondence matrix

| i18n Key | Leptos | Next.js |
|---|---|---|
| `modules.section.core` | `t!(i18n, modules.section.core)` | `t('section.core')` |
| `modules.badge.core` | `t!(i18n, modules.badge.core)` | `t('badge.core')` |
| `modules.toast.enabled` | `t_string!(i18n, modules.toast.enabled)` | `t('toast.enabled')` |
| `modules.title` | `t_string!(i18n, modules.title)` | `t('title')` |

> Leptos: absolute keys via `t!()` / `t_string!()`.
> Next.js: relative keys via `useTranslations('modules')`.

## Consequences

### Positive

- **Compile-time safety** (Leptos): `t!()` checks keys at compile time.
- **IDE autocompletion** (Next.js): `next-intl` TypeScript support.
- **Unified UX**: same texts, same locale files.
- **Proven libraries**: not custom solutions, but ecosystem standards.

### Negative

- **JSON duplication**: still in two places.
  Mitigation: CI check, or `@rustok/admin-locales` workspace package.
- **Different API**: `t!(i18n, key)` vs `t('key')` — unavoidable due to different languages.

### Follow-up

1. Apply `useTranslations()` to the remaining Next.js admin pages.
2. All `t_string!()` in Leptos admin are already applied to all 16 files.
3. CI: check key matching between locales.
4. `leptos_i18n` automatically saves locale in cookie — synchronization with `next-intl`.
