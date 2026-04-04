# Документация по UI пакетам модулей

Обзор документации по установке и использованию модулей с UI пакетами для админки и фронтенда.

## 📚 Быстрый доступ

### Для начинающих
- **[Быстрый старт](UI_PACKAGES_QUICKSTART.md)** — Создайте свой первый модуль с UI за 10 минут

### Для разработчиков
- **[Индекс документации модулей](_index.md)** — Навигация по актуальным документам модульной системы
- **[Манифест модулей](manifest.md)** — Каноническая спецификация формата `modules.toml`

### Для архитекторов
- **[Реестр модулей](registry.md)** — Lifecycle, toggle, guards и ответственные зоны
- **[Система модулей: маркетплейс и жизненный цикл](module-system-plan.md)** — Единый план: install/uninstall, marketplace, tenant toggle
- **[Архитектура модулей](../architecture/modules.md)** — Общая архитектурная модель модульной платформы

## 🎯 Основные концепты

### Что такое UI пакеты модулей?

Каждый модуль RusToK может включать три компонента:

1. **Backend crate** (`rustok-*`) — доменная логика и API
2. **Admin UI пакет** (`leptos-*-admin`) — компоненты админки
3. **Storefront UI пакет** (`leptos-*-storefront`) — компоненты витрины

Для native i18n модуль теперь может дополнительно декларировать bundle contract прямо в
`rustok-module.toml` через `[provides.admin_ui.i18n]` и `[provides.storefront_ui.i18n]`.
Это не заменяет host-level locale contract, а фиксирует, где лежат package-owned `locales/*.json`
и `messages/*.json`, если модуль действительно поставляет собственные переводы.
Модульные UI-пакеты не должны придумывать свою locale-цепочку: effective locale выбирается
сервером и host-приложением, а пакет только потребляет уже переданный язык.
На текущем этапе этот contract уже используется admin-пакетами `workflow`, `rbac`, `tenant`,
`index`, `outbox`, `pages`, `comments`, `channel`, `forum`, `search`, `commerce` и storefront-пакетами `blog`, `pages`,
`commerce`, `forum`, `search`.
Отдельно от manifest-driven модулей тот же runtime locale contract теперь соблюдает capability-owned пакет
`rustok-ai-admin`: он тоже хранит package-owned `admin/locales/*.json`, но не публикуется через
`rustok-module.toml`, потому что относится к capability crate, а не к модульному registry surface.

### Пример структуры

```
rustok-commerce/              # Backend
leptos-commerce-admin/         # Admin UI
leptos-commerce-storefront/    # Storefront UI
```

## ✅ Самописные Leptos библиотеки (использовать в разработке)

Эти библиотеки уже есть в репозитории и должны использоваться агентами при параллельной разработке UI.

**Core crates**
- [leptos-auth](../../crates/leptos-auth/README.md)
- [leptos-forms](../../crates/leptos-forms/README.md)
- [leptos-graphql](../../crates/leptos-graphql/README.md)
- [leptos-hook-form](../../crates/leptos-hook-form/README.md)
- [leptos-shadcn-pagination](../../crates/leptos-shadcn-pagination/README.md)
- [leptos-table](../../crates/leptos-table/README.md)
- [leptos-ui](../../crates/leptos-ui/README.md)
- [leptos-zod](../../crates/leptos-zod/README.md)
- [leptos-zustand](../../crates/leptos-zustand/README.md)

**UI packages (module UI integration)**
- `packages/leptos-auth`
- `packages/leptos-graphql`
- `packages/leptos-hook-form`
- `packages/leptos-zod`
- `packages/leptos-zustand`

## 📖 По сценарию использования

### "Я хочу создать новый модуль с UI"
→ [Быстрый старт](UI_PACKAGES_QUICKSTART.md)

### "Я хочу понять, как работает система установки"
→ [Полное руководство](MODULE_UI_PACKAGES_INSTALLATION.md)

### "Мне нужна спецификация формата манифеста"
→ [Манифест модулей](module-manifest.md)

### "Хочу посмотреть готовые примеры"
→ [Пример modules.toml](../../modules.toml.example)

### "Хочу разобраться в архитектуре"
→ [Полное руководство](MODULE_UI_PACKAGES_INSTALLATION.md#архитектура-системы)

## 🗂️ Структура документации

### UI_PACKAGES_QUICKSTART.md (362 строки)
- Пошаговое создание модуля с UI
- Кодовые примеры для всех этапов
- Использование готовых UI компонентов
- Полезные команды

### MODULE_UI_PACKAGES_INSTALLATION.md (884 строки)
- Обзор архитектуры системы
- Расширенный формат манифеста
- Типы UI пакетов
- Процесс создания модуля (9 шагов)
- API для управления установкой
- Режимы деплоя (monolith/headless)
- Жизненный цикл UI пакетов
- Локальная разработка
- Best Practices
- Troubleshooting
- Примеры готовых модулей

### modules.toml.example
- Пример манифеста с UI пакетами
- Демонстрация различных конфигураций
- Комментарии для каждого параметра

## 🔗 Связанная документация

### Модули
- [Обзор модулей](overview.md) — Карта всех модулей в проекте
- [Реестр модулей](registry.md) — Lifecycle, toggle, guards
- [Реестр crate-модулей](crates-registry.md) — Карта crate-структуры и статусов
- [Система модулей](module-system-plan.md) — Единый план: marketplace, install/uninstall, toggle

### UI и Frontend
- [Admin UI документация](../../docs/UI/README.md) — Админка документация
- [UI компоненты](../../crates/leptos-ui/README.md) — Библиотека компонентов
- [Формы и валидация](../../crates/leptos-forms/README.md) — Система форм
- [GraphQL интеграция](../../crates/leptos-graphql/README.md) — GraphQL хуки

### Архитектура
- [Обзор архитектуры](../architecture/overview.md) — Принципы и решения платформы
- [Манифест платформы](../../RUSTOK_MANIFEST.md) — Философия и стек

## 🚀 Быстрое начало

```bash
# 1. Посмотрите быстрый старт
cat docs/modules/UI_PACKAGES_QUICKSTART.md

# 2. Создайте модуль
cargo new --lib crates/rustok-mymodule

# 3. Создайте Admin UI
cargo new --lib crates/leptos-mymodule-admin

# 4. Обновите modules.toml
# (см. пример в modules.toml.example)

# 5. Сгенерируйте регистрацию
cargo xtask generate-registry

# 6. Соберите проект
cargo build --release --features mymodule
```

## 💡 Советы

- Начните с [Быстрого старта](UI_PACKAGES_QUICKSTART.md)
- Используйте готовые компоненты из `leptos-ui`
- Сверяйтесь с [индексом документации модулей](_index.md), если материал переехал
- Смотрите примеры готовых модулей в [реестре модулей](registry.md)

## 🆘 Нужна помощь?

- **Проблемы с компиляцией** → [Руководство по тестированию и диагностике](../guides/testing.md)
- **Вопросы по API** → [Архитектура API](../architecture/api.md)
- **Архитектурные вопросы** → [Архитектура модулей](../architecture/modules.md)

---

**Последнее обновление:** 16 февраля 2026
