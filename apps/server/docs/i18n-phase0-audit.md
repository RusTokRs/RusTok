# Фаза 0: Аудит i18n для системных строк (error messages, UI labels)

> **Дата:** 2026-03-16
> **Ветка:** `claude/review-loco-integration-CCJIx`
> **Цель:** Инвентаризация существующего `rustok-core::i18n` и захардкоженных строк в `apps/server`.
> **Scope:** Системные строки — сообщения об ошибках, статусные ответы. *Не* переводы контентных сущностей (те покрыты `docs/architecture/i18n.md`).

---

## 1. Текущее состояние `rustok-core::i18n`

### Что есть

Файл `crates/rustok-core/src/i18n.rs`:

| Компонент | Состояние |
|---|---|
| `Locale` enum | En, Ru, Es, De, Fr, Zh — 6 локалей |
| `translate(locale, key)` | Публичный API |
| `extract_locale_from_header(header)` | Парсит `Accept-Language` |
| Количество ключей | **13 ключей** × 6 локалей = 78 записей |
| Хранилище | `static HashMap<(Locale, &'static str), &'static str>` |
| Поиск | O(n) полный перебор (без `HashMap::get` — ключ `&str`, не `&'static str`) |
| Экспорт из crate | `pub use i18n::{extract_locale_from_header, translate, Locale}` |

### Существующие ключи (контентная валидация)

```
invalid_kind, invalid_format, invalid_locale_length, invalid_locale_format,
position_must_be_non_negative, position_too_large, depth_must_be_non_negative,
depth_too_large, reply_count_must_be_non_negative,
slug_empty, slug_too_long, slug_invalid_characters, slug_hyphen_boundary
```

Все 13 ключей — это валидационные ошибки для полей контентных сущностей (узлы, товары, форум). Переводы предоставлены на все 6 локалей.

### Проблемы текущей реализации

1. **O(n) lookup**: `translate()` итерирует всю HashMap вместо `map.get(&(locale, key))`. Это работает, но при 100+ ключах станет заметно.
2. **Ключи как `&'static str`**: нельзя использовать строки из переменных — только литералы. Ограничивает использование в динамическом контексте.
3. **Строки жёстко в коде**: добавление перевода = изменение Rust-файла. Переводчики не могут работать без знания Rust.
4. **Нет поддержки плюрализации**: для русского, немецкого, китайского нужны формы (`1 файл`, `2 файла`, `5 файлов`).

---

## 2. Инвентаризация захардкоженных строк в `apps/server`

### 2.1 Использование `rustok-core::i18n` в server

**Нулевое.** Ни один файл в `apps/server/src` не импортирует и не вызывает `rustok_core::i18n::translate()`. Модуль i18n существует в core, но в server не подключён.

### 2.2 Пользовательские строки (требуют перевода)

Строки, которые попадают в API-ответ и видны пользователю:

| Категория | Уникальные строки | Файлы |
|---|---|---|
| Auth (регистрация, вход, выход) | 10 | `controllers/auth.rs`, `graphql/auth/mutation.rs`, `services/auth_lifecycle.rs` |
| OAuth | 5 | `controllers/auth.rs`, `services/oauth_app.rs` |
| Итого user-facing | **15** | — |

**Полный список (15 уникальных ключей для перевода):**

```
auth.email_already_exists      → "A user with this email already exists" / "Email already exists"
auth.invalid_credentials       → "Invalid credentials"
auth.user_inactive             → "User is inactive"
auth.invalid_refresh_token     → "Invalid refresh token" / "Invalid or expired refresh token"
auth.session_expired           → "Session expired"
auth.user_not_found            → "User not found"
auth.invalid_reset_token       → "Invalid reset token"
auth.invalid_invite_token      → "Invalid invite token"
auth.invalid_verification_token → "Invalid verification token"
auth.invalid_or_expired_code   → "Invalid or expired code"
oauth.auth_config_error        → "Auth config error"
oauth.pkce_invalid             → "PKCE code verifier is invalid"
oauth.redirect_uri_mismatch    → "Redirect URI mismatch"
oauth.refresh_no_user          → "Refresh token has no associated user"
```

> **Замечание о дублировании:** `"Email already exists"` (в `graphql/`) и `"A user with this email already exists"` (в `controllers/`) — это один и тот же смысловой ключ `auth.email_already_exists`, но с разными строками. После i18n должна остаться одна строка на один ключ.

### 2.3 Инфраструктурные строки (перевод не нужен)

~174 из 189 `Error::` конструкторов используют `format!()` с техническими деталями. Эти строки:
- Предназначены для разработчиков и DevOps (логи, дашборды)
- Содержат runtime-значения (id, имена файлов, коды ошибок)
- **Не попадают в user-facing ответы** (оборачиваются в `500 Internal Server Error`)

Примеры:
```rust
Error::BadRequest(format!("Invalid rustok settings: {error}"))
Error::Message(format!("Failed to serialize OpenAPI spec: {e}"))
Error::string(&format!("rbac report serialization failed: {error}"))
```

### 2.4 GraphQL-строки

GraphQL errors в `graphql/auth/mutation.rs` используют те же 10 auth-ключей через отдельную функцию `map_auth_lifecycle_error()`. После внедрения i18n эта функция должна принимать `Locale` и возвращать переведённую строку.

---

## 3. Решение: формат переводов

### Варианты

| Формат | Плюсы | Минусы | Вывод |
|---|---|---|---|
| **Static HashMap** (текущий) | Нет зависимостей, компилируется, быстро | Нет плюрализации, переводчики не могут редактировать, масштаб ограничен | Только для текущего объёма |
| **Fluent `.ftl`** | ICU плюрализация, экосистема переводчиков, runtime-гибкость | Новая зависимость `fluent-bundle`, runtime file loading или `include_str!` | Правильный выбор для масштаба |
| **JSON файлы** | Просто, поддерживается всеми инструментами | Нет плюрализации built-in, нужна кастомная логика | Промежуточный вариант |
| **TOML файлы** | Уже используется в проекте | Те же проблемы что у JSON | Нет преимуществ перед JSON |

### Решение: двухфазовый подход

**Фаза 0 (сейчас):** расширить существующий `HashMap` в `rustok-core::i18n` — добавить 15 auth/oauth ключей. Это требует нуль новых зависимостей и позволяет подключить `translate()` в server немедленно.

**Фаза 2+ (при росте объёма / работе с переводчиками):** мигрировать на Fluent `.ftl`. Fluent выбирается как целевой формат потому что:
- Русский язык требует 4 формы плюрализации (нет/один/мало/много)
- Немецкий, китайский тоже нестандартны
- Fluent нативно решает `gender`, `plurality`, `select` без кастомного кода
- `fluent-bundle` crate стабилен и широко используется

**Интерфейс остаётся тем же** (`translate(locale, key)`) — внутренняя реализация может смениться с HashMap на Fluent без изменений в коде вызовов.

---

## 4. Конвенция для модулей (до Фазы 4)

До формализации через `RusToKModule::translations()` (Фаза 4):

### Структура файлов

```
crates/{rustok-module}/src/i18n.rs     # Ключи и переводы модуля
crates/{rustok-module}/src/lib.rs       # pub mod i18n; pub use i18n::*;
```

### Правила именования ключей

```
{module_slug}.{entity}.{error_or_label}
```

Примеры:
```
auth.email_already_exists
commerce.product.not_found
forum.topic.closed
content.node.invalid_kind   ← уже есть как "invalid_kind"
```

### Регистрация в `rustok-core::i18n`

До Фазы 4 модули регистрируют ключи напрямую в `crates/rustok-core/src/i18n.rs`. После Фазы 4 — через `translations()` метод трейта, который агрегируется в core.

---

## 5. Что нужно сделать для завершения Фазы 0

### Минимальный набор (активация i18n в server)

| Шаг | Файл | Описание |
|---|---|---|
| 1 | `rustok-core/src/i18n.rs` | Добавить 15 auth/oauth ключей; исправить O(n) lookup |
| 2 | `apps/server/src/middleware/locale.rs` | Axum middleware: извлечь `Locale` из `Accept-Language` → `Extension<Locale>` |
| 3 | `apps/server/src/auth.rs` | `auth_config_from_ctx` — добавить `locale` параметр; использовать `translate()` в `auth_err()` |
| 4 | `apps/server/src/graphql/auth/mutation.rs` | `map_auth_lifecycle_error(error, locale)` — переведённые сообщения |
| 5 | `apps/server/src/app.rs` | Зарегистрировать locale middleware в `after_routes` |

### Цепочка локалей (locale resolution chain)

```
Accept-Language header
  → ctx.shared_store.get::<TenantSettings>().default_locale
    → "en"
```

Tenant default locale будет доступен в Фазе 1 (Settings API). До этого — только `Accept-Language → "en"`.

### Что НЕ входит в Фазу 0

- Перевод инфраструктурных строк (format! с техническими деталями) — не нужен
- Migrация на Fluent — Фаза 2+
- `RusToKModule::translations()` трейт — Фаза 4
- Admin UI language switcher — Фаза 1.5+
- Полная замена всех 189 `Error::` конструкторов — не нужна (большинство инфраструктурные)

---

## 6. Итог аудита

| Метрика | Значение |
|---|---|
| Существующих i18n ключей в core | 13 (валидация контента) |
| Пользовательских строк в server, требующих перевода | **15** (auth/oauth) |
| Инфраструктурных строк (перевод не нужен) | ~174 |
| Текущее использование `translate()` в server | **0** |
| Выбранный формат (Phase 0) | Static HashMap (расширенный) |
| Целевой формат (Phase 2+) | Fluent `.ftl` |
| Усилие на Phase 0 минимальный набор | ~1 день |
