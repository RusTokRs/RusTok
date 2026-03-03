# План подгрузки и компиляции при включении/отключении модулей

> Статус: RFC / Дорожная карта
> Дата: 2026-03-03

## Контекст

RusTok использует **compile-time модульность**: все модули линкуются в бинарник
при сборке через `ModuleRegistry`. Включение/отключение модуля из админки на
уровне tenant — это **runtime-операция** (запись в `tenant_modules`), которая не
требует пересборки. Установка/удаление модуля из состава платформы — это
**build-time операция**, требующая пересборки бинарника.

Нужно различать два уровня:

| Уровень | Действие | Требует пересборки |
|---|---|---|
| **Tenant-level** | Включить/отключить модуль для конкретного тенанта | Нет |
| **Platform-level** | Установить/удалить модуль из состава платформы | Да |

---

## Часть 1: Tenant-level toggle (текущая реализация)

### Как это работает сейчас

1. Админ нажимает Switch в UI модулей (`/modules`).
2. Leptos-клиент отправляет GraphQL мутацию `toggleModule(moduleSlug, enabled)`.
3. Бэкенд (`ModuleLifecycleService::toggle_module`):
   - Проверяет существование модуля в `ModuleRegistry`.
   - Проверяет, что модуль не `Core`.
   - Проверяет зависимости (при включении) / зависимых (при отключении).
   - Персистит состояние в `tenant_modules` (транзакция).
   - Вызывает `on_enable()`/`on_disable()` хук модуля.
   - При ошибке хука — откат состояния.
4. UI получает обновлённый статус и обновляет карточку модуля.

### Влияние на UI (Leptos admin + storefront)

- **Навигация**: сайдбар может фильтровать пункты меню на основе `enabledModules`
  (GraphQL query). Если модуль отключён — его nav items скрыты.
- **Слоты виджетов**: `components_for_slot()` может фильтровать по enabled
  модулям (требует расширения реестра).
- **Роутинг**: защита маршрутов — если модуль отключён, middleware/guard
  перенаправляет на 404.

### Что нужно доработать (Tenant-level)

1. **`enabledModules` контекст в Leptos admin/storefront**:
   - Создать `EnabledModulesProvider` — загружает список при старте через
     `enabledModules` query.
   - Предоставляет `use_enabled_modules()` хук.
   - Используется в sidebar для условного рендера nav items.

2. **Guard для маршрутов**:
   - `<ModuleGuard slug="blog">` — компонент-обёртка, который проверяет
     enabled статус и показывает контент или 404/placeholder.

3. **Фильтрация слотов по enabled модулям**:
   - Расширить `AdminComponentRegistration` полем `module_slug: Option<&'static str>`.
   - `components_for_slot()` фильтрует по `enabled_modules` из контекста.

---

## Часть 2: Platform-level install/uninstall (rebuild pipeline)

### Архитектура

```
┌──────────┐     ┌──────────────┐     ┌───────────────┐     ┌──────────┐
│  Admin   │────>│ Build Service│────>│ Artifact Store │────>│  Deploy  │
│  UI      │     │ (CI/CD)      │     │ (Registry)     │     │ (K8s)    │
└──────────┘     └──────────────┘     └───────────────┘     └──────────┘
      │                 │                                          │
      │  1. Обновить    │  2. cargo build                         │
      │  modules.toml   │  3. docker build                        │
      │                 │  4. push image                          │
      └─────────────────┴──────────────────────────────────────────┘
                        5. deploy & smoke check
```

### Этапы реализации

#### Этап 1: Build Service API (GraphQL)

Все API — через GraphQL, без REST эндпоинтов.

```graphql
type BuildJob {
  id: ID!
  status: BuildStatus!
  stage: String
  progress: Int
  logsUrl: String
  startedAt: DateTime
  finishedAt: DateTime
  manifestHash: String!
  modulesDelta: String!
  requestedBy: String!
  reason: String!
}

enum BuildStatus {
  QUEUED
  BUILDING
  TESTING
  DEPLOYING
  COMPLETED
  FAILED
  ROLLED_BACK
}

type Mutation {
  # Инициировать сборку после изменения modules.toml
  requestBuild(input: RequestBuildInput!): BuildJob!

  # Деплой конкретной сборки
  deployBuild(buildId: ID!, environment: String!): BuildJob!

  # Откат к предыдущему релизу
  rollbackBuild(buildId: ID!, targetRelease: String!): BuildJob!
}

type Query {
  # Статус сборки
  buildJob(id: ID!): BuildJob

  # История сборок
  buildJobs(limit: Int, offset: Int): [BuildJob!]!

  # Каталог доступных модулей (marketplace)
  availableModules: [AvailableModule!]!
}

type Subscription {
  # Реальтайм обновления статуса сборки
  buildProgress(buildId: ID!): BuildJob!
}
```

#### Этап 2: Manifest Manager

Сервис для работы с `modules.toml`:

1. **Чтение** текущего манифеста.
2. **Добавление** модуля: парсинг → добавление записи → валидация → запись.
3. **Удаление** модуля: проверка зависимостей → удаление → запись.
4. **Diff**: сравнение двух манифестов для отображения в UI.

```rust
pub struct ManifestManager;

impl ManifestManager {
    /// Добавить модуль в манифест
    pub fn install_module(
        manifest: &mut Manifest,
        slug: &str,
        spec: ModuleSpec,
    ) -> Result<ManifestDiff>;

    /// Удалить модуль из манифеста
    pub fn uninstall_module(
        manifest: &mut Manifest,
        slug: &str,
    ) -> Result<ManifestDiff>;

    /// Валидировать граф зависимостей
    pub fn validate(manifest: &Manifest) -> Result<()>;
}
```

#### Этап 3: Build Orchestrator

Оркестратор сборки (может быть отдельным микросервисом или интеграцией с CI):

1. **Queue**: принимает запрос на сборку, помещает в очередь.
2. **Build**: клонирует repo, применяет манифест, запускает `cargo build`.
3. **Test**: запускает smoke-тесты.
4. **Package**: собирает Docker-образ.
5. **Deploy**: обновляет deployment.

Варианты реализации:
- **GitHub Actions / GitLab CI**: запускать pipeline через API.
- **Встроенный**: минимальный build worker на базе `tokio::process::Command`.
- **Kubernetes Job**: запускать build как K8s Job.

#### Этап 4: UI для install/uninstall в Leptos admin

Новая страница/секция в `/modules`:

1. **Каталог модулей** — список доступных для установки модулей (из registry/marketplace).
2. **Установленные модули** — текущий состав из `modules.toml`.
3. **Install/Uninstall** — кнопки с подтверждением.
4. **Build Progress** — realtime отображение статуса сборки (GraphQL Subscription).
5. **История сборок** — лог предыдущих install/uninstall операций.

---

## Часть 3: Leptos Storefront — модульные слоты

### Текущий механизм

Storefront использует `StorefrontSlot` enum для регистрации компонентов:
- `HomeAfterHero` — слот после hero-секции на главной.

### Расширение

1. **Больше слотов**: `ProductPageSidebar`, `CartSummary`, `Footer`, etc.
2. **Условная регистрация**: модуль регистрирует компоненты только если enabled.
3. **SSR-совместимость**: storefront рендерится на сервере (Axum), поэтому
   enabled-check должен быть на серверной стороне.

```rust
pub fn register_components(enabled_modules: &HashSet<String>) {
    if enabled_modules.contains("blog") {
        register_component(StorefrontComponentRegistration {
            id: "blog-latest-posts",
            slot: StorefrontSlot::HomeAfterHero,
            order: 20,
            render: blog_latest_posts_widget,
        });
    }
}
```

---

## Часть 4: Cargo Features (опциональная оптимизация)

Для уменьшения размера бинарника можно использовать Cargo features:

```toml
# apps/server/Cargo.toml
[features]
default = ["mod-content", "mod-commerce", "mod-blog", "mod-pages"]
mod-content = ["dep:rustok-content"]
mod-commerce = ["dep:rustok-commerce"]
mod-blog = ["dep:rustok-blog", "mod-content"]
mod-pages = ["dep:rustok-pages"]
mod-forum = ["dep:rustok-forum", "mod-content"]
```

Build service активирует features на основе `modules.toml`:

```bash
cargo build --release --no-default-features \
  --features "mod-content,mod-blog,mod-pages"
```

Это позволяет:
- Не включать код неиспользуемых модулей в бинарник.
- Уменьшить время компиляции.
- Уменьшить размер Docker-образа.

---

## Приоритеты реализации

| Приоритет | Задача | Сложность |
|---|---|---|
| P0 | Tenant-level toggle (уже работает) | Готово |
| P0 | Leptos admin: страница модулей с toggle | Готово |
| P1 | `EnabledModulesProvider` + conditional nav | Средняя |
| P1 | `ModuleGuard` для маршрутов | Низкая |
| P2 | Manifest Manager | Средняя |
| P2 | Build Service API (GraphQL) | Высокая |
| P3 | Build Orchestrator (CI integration) | Высокая |
| P3 | UI install/uninstall + build progress | Средняя |
| P4 | Cargo features optimization | Низкая |
| P4 | Module marketplace/catalog | Высокая |

---

## Безопасность

- **Нет runtime-подгрузки нативного кода** — все модули компилируются в бинарник.
- **RBAC**: `modules:manage` permission для toggle; `modules:install` для
  platform-level операций.
- **Audit log**: все операции с модулями логируются.
- **Rollback**: каждый деплой имеет `release_id` для отката.
- **Валидация зависимостей**: перед install/uninstall проверяется граф зависимостей.
