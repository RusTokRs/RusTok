# План подгрузки и компиляции при включении/отключении модулей

> Статус: RFC / Дорожная карта
> Дата: 2026-03-03

> Source of truth:
> - Этот документ ведёт текущую реализацию и статус delivery по server/admin/storefront.
> - `docs/modules/marketplace-plan.md` остаётся архитектурным RFC для долгосрочного marketplace/registry контракта.
> - Если документы расходятся, для текущей codebase приоритет у этого файла; marketplace RFC задаёт целевую форму API и источников модулей.
> - Никакого legacy/public API здесь не предполагается: все GraphQL/catalog/build contracts считаются внутренними и эволюционными частями новой модульной системы платформы.

## Философия

Для администратора управление модулями выглядит **одинаково** — как в WordPress:
одна страница, кнопка "Включить/Отключить", кнопка "Установить/Удалить".
Неважно, как собрана платформа (единый бинарник, разнесённые фронтенды, K8s).

Единственное видимое отличие от WordPress — **время на пересборку** при
установке/удалении модуля (Rust компилирует AOT, PHP подгружает файлы JIT).
Это компенсируется прогресс-баром и уведомлением по завершении.

```
┌─────────────────────────────────────────────────────┐
│  WordPress      │  RusTok                           │
├─────────────────┼───────────────────────────────────┤
│  Включить       │  Toggle switch → мгновенно        │
│  Отключить      │  Toggle switch → мгновенно        │
│  Установить     │  Install → пересборка (минуты)    │
│  Удалить        │  Uninstall → пересборка (минуты)  │
│  Маркетплейс    │  Каталог → стандарт module.toml   │
└─────────────────┴───────────────────────────────────┘
```

Два уровня операций — один UX:

| Уровень | Действие | Время | UX |
|---|---|---|---|
| **Tenant-level** | Включить/отключить для тенанта | Мгновенно | Toggle switch |
| **Platform-level** | Установить/удалить из платформы | 2-5 мин (сборка) | Кнопка + прогресс-бар |

---

## Часть 1: Tenant-level toggle (реализовано)

### Текущий flow

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

### Что нужно доработать (Tenant-level)

1. **`EnabledModulesProvider` контекст в Leptos admin/storefront**:
   - Загружает `enabledModules` query при старте.
   - `use_enabled_modules()` хук для всех компонентов.
   - Sidebar фильтрует nav items по enabled модулям.

2. **`<ModuleGuard slug="blog">`** — компонент-обёртка для маршрутов:
   - Проверяет enabled статус → показывает контент или 404/placeholder.

3. **Фильтрация слотов** — `components_for_slot()` фильтрует по `enabled_modules`.

---

## Часть 2: Platform-level install/uninstall (rebuild pipeline)

### Архитектура — WordPress-подобный UX

Для пользователя install/uninstall выглядит как одна кнопка:

```
┌──────────────────────────────────────────────────────────────┐
│  Admin UI                                                     │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Blog Module v1.2.0                     [Установить]    │ │
│  │  Блоговый движок с категориями и тегами                 │ │
│  └─────────────────────────────────────────────────────────┘ │
│                         ↓ клик                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Blog Module v1.2.0                                     │ │
│  │  ████████████░░░░░░░░░░░░  45%  Компиляция...           │ │
│  └─────────────────────────────────────────────────────────┘ │
│                         ↓ готово                             │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │  Blog Module v1.2.0            [●] Вкл    [Удалить]     │ │
│  │  Блоговый движок с категориями и тегами                 │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

### Pipeline: что происходит за кулисами

```
Admin UI → GraphQL → Build Service → CI/CD → Artifact → Deploy
   │                     │                       │          │
   │ 1. installModule    │ 2. update             │          │
   │    slug + version   │    modules.toml       │          │
   │                     │ 3. cargo build         │          │
   │ 4. subscription     │    --features=...     │          │
   │    buildProgress    │ 5. docker build        │          │
   │    ██████░░ 60%     │ 6. push image          │          │
   │                     │                       │          │
   │ 7. completed        │                       │ 8. deploy│
   │    toast: "ready"   │                       │ rolling  │
   └─────────────────────┴───────────────────────┴──────────┘
```

### GraphQL API (Build Service)

```graphql
type BuildJob {
  id: ID!
  status: BuildStatus!
  stage: String            # "compiling", "testing", "packaging", "deploying"
  progress: Int            # 0-100
  logsUrl: String
  startedAt: DateTime
  finishedAt: DateTime
  manifestHash: String!
  modulesDelta: String!    # "+blog,-forum"
  requestedBy: String!
  reason: String!
}

enum BuildStatus {
  QUEUED
  COMPILING
  TESTING
  PACKAGING
  DEPLOYING
  COMPLETED
  FAILED
  ROLLED_BACK
}

type Mutation {
  # Установить модуль из маркетплейса → запускает сборку
  installModule(slug: String!, version: String!): BuildJob!

  # Удалить модуль → запускает сборку без него
  uninstallModule(slug: String!): BuildJob!

  # Обновить модуль до новой версии
  upgradeModule(slug: String!, version: String!): BuildJob!

  # Откат к предыдущему релизу
  rollbackBuild(buildId: ID!): BuildJob!
}

type Query {
  # Текущая сборка
  activeBuild: BuildJob

  # История сборок
  buildHistory(limit: Int, offset: Int): [BuildJob!]!

  # Каталог доступных модулей (маркетплейс)
  marketplace(search: String, category: String): [MarketplaceModule!]!

  # Установленные модули платформы
  installedModules: [InstalledModule!]!
}

type Subscription {
  # Реальтайм прогресс сборки
  buildProgress(buildId: ID!): BuildJob!
}
```

### Manifest Manager

Сервис для работы с `modules.toml` — единый source of truth для состава платформы:

```rust
pub struct ManifestManager;

impl ManifestManager {
    /// Добавить модуль в манифест (из маркетплейса или локально)
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

    /// Обновить версию модуля
    pub fn upgrade_module(
        manifest: &mut Manifest,
        slug: &str,
        new_version: &str,
    ) -> Result<ManifestDiff>;

    /// Валидировать граф зависимостей
    pub fn validate(manifest: &Manifest) -> Result<()>;
}
```

### Build Orchestrator

Варианты реализации (выбор зависит от deployment profile):

| Deployment | Build strategy |
|---|---|
| **Self-hosted** | Build worker (tokio::process) на том же сервере |
| **CI/CD** | GitHub Actions / GitLab CI через API |
| **Kubernetes** | Build как K8s Job с kaniko |
| **Cloud** | Managed build service (Cloud Build, CodeBuild) |

### Cargo Features — автоматическая оптимизация

Build service автоматически мапит `modules.toml` в cargo features:

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

```bash
# Build service генерирует на основе modules.toml:
cargo build --release --no-default-features \
  --features "mod-content,mod-blog,mod-pages"
```

---

## Часть 3: Маркетплейс модулей (единый стандарт)

### Стандарт модуля — `rustok-module.toml`

Каждый модуль (внутренний или сторонний) описывается единым манифестом:

```toml
[module]
slug = "blog"
name = "Blog"
version = "1.2.0"
description = "Blogging engine with categories, tags, and SEO"
authors = ["RusTok Team"]
license = "MIT"
repository = "https://github.com/rustok/rustok-blog"
homepage = "https://rustok.dev/modules/blog"

# Иконка и скриншоты для маркетплейса
icon = "assets/icon.svg"
screenshots = ["assets/screenshot-1.png", "assets/screenshot-2.png"]

[module.categories]
primary = "content"       # content, commerce, analytics, social, dev-tools, integrations
tags = ["blog", "cms", "seo", "markdown"]

[compatibility]
rustok_min = "0.5.0"      # Минимальная версия платформы
rustok_max = "1.x"        # Максимальная совместимая версия
rust_edition = "2024"

[dependencies]
# Зависимости от других RusTok-модулей
content = ">= 1.0.0"

[crate]
# Rust crate — source of truth для компиляции
name = "rustok-blog"
source = "registry"       # "registry" | "git" | "path"
registry = "https://modules.rustok.dev/api/v1/crates"
# Или для git:
# source = "git"
# git = "https://github.com/rustok/rustok-blog.git"
# branch = "main"

[provides]
# Что модуль предоставляет платформе

# Слоты в admin UI (FSD: features layer)
admin_nav = [
  { label_key = "blog.nav.posts", href = "/posts", icon = "pencil" },
  { label_key = "blog.nav.categories", href = "/categories", icon = "folder" },
]

# Слоты в storefront
storefront_slots = [
  { id = "blog-latest-posts", slot = "HomeAfterHero", order = 20 },
  { id = "blog-sidebar-widget", slot = "Sidebar", order = 10 },
]

# GraphQL расширения
graphql_types = ["Post", "Category", "Tag"]
graphql_queries = ["posts", "post", "categories"]
graphql_mutations = ["createPost", "updatePost", "deletePost"]

# Миграции БД
migrations = true

# Permissions для RBAC
permissions = [
  "blog:read",
  "blog:write",
  "blog:delete",
  "blog:publish",
]

# Event listeners
events = ["content.created", "content.updated"]

[settings]
# Настройки модуля (JSON schema для UI генерации)
[settings.posts_per_page]
type = "integer"
default = 10
min = 1
max = 100
label = "Posts per page"

[settings.enable_comments]
type = "boolean"
default = true
label = "Enable comments"

[settings.default_status]
type = "enum"
values = ["draft", "published"]
default = "draft"
label = "Default post status"
```

### Реестр маркетплейса — GraphQL API

```graphql
type MarketplaceModule {
  slug: String!
  name: String!
  version: String!
  description: String!
  longDescription: String
  authors: [String!]!
  license: String!
  repository: String
  homepage: String

  # Визуальные элементы
  iconUrl: String
  screenshots: [String!]!

  # Категоризация
  category: String!
  tags: [String!]!

  # Совместимость
  rustokMinVersion: String!
  rustokMaxVersion: String

  # Зависимости от других модулей
  dependencies: [ModuleDependency!]!

  # Статистика (будущее)
  downloads: Int
  rating: Float
  reviewCount: Int

  # Версии
  versions: [ModuleVersion!]!
  latestVersion: String!

  # Статус на текущей платформе
  installed: Boolean!
  installedVersion: String
  updateAvailable: Boolean!
}

type ModuleDependency {
  slug: String!
  versionConstraint: String!
  satisfied: Boolean!        # Выполнена ли зависимость на текущей платформе
}

type ModuleVersion {
  version: String!
  releasedAt: DateTime!
  changelog: String
  compatible: Boolean!       # Совместима с текущей версией RusTok
}

type Query {
  # Каталог маркетплейса с поиском и фильтрацией
  marketplace(
    search: String
    category: String
    tag: String
    compatible: Boolean      # Только совместимые с текущей версией
    limit: Int
    offset: Int
  ): MarketplaceConnection!

  # Детали модуля
  marketplaceModule(slug: String!): MarketplaceModule

  # Категории маркетплейса
  marketplaceCategories: [MarketplaceCategory!]!

  # Проверить совместимость перед установкой
  checkCompatibility(slug: String!, version: String!): CompatibilityReport!
}

type CompatibilityReport {
  compatible: Boolean!
  issues: [CompatibilityIssue!]!
  missingDependencies: [ModuleDependency!]!
  willAutoInstall: [String!]!    # Зависимости, которые установятся автоматически
}
```

### Архитектура реестра

```
┌─────────────────────────────────────────────────────────────┐
│  Marketplace Registry (modules.rustok.dev)                   │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Module Index  │  │ Crate Mirror │  │  Validation  │      │
│  │ (metadata)    │  │ (artifacts)  │  │  Pipeline    │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                  │                  │               │
│         ▼                  ▼                  ▼               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                 GraphQL API                           │   │
│  │  marketplace { ... }                                  │   │
│  │  publishModule { ... }                                │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
         ▲                          ▲
         │ query                    │ publish
         │                          │
┌────────┴────────┐        ┌───────┴────────┐
│  Admin UI       │        │  Module Author │
│  (Leptos/Next)  │        │  (CLI)         │
└─────────────────┘        └────────────────┘
```

### Публикация модуля (для авторов)

```bash
# CLI для публикации модулей в маркетплейс
rustok module init                  # Scaffold rustok-module.toml
rustok module validate              # Проверить манифест и зависимости
rustok module test                  # Запустить тесты совместимости
rustok module publish               # Опубликовать в маркетплейс
rustok module publish --dry-run     # Проверить без публикации
```

### Validation Pipeline

При публикации модуля в маркетплейс, pipeline проверяет:

1. **Manifest** — валидный `rustok-module.toml`.
2. **Compilation** — модуль компилируется с минимальной и максимальной версией RusTok.
3. **Tests** — тесты модуля проходят.
4. **Security** — cargo-audit, no unsafe без обоснования.
5. **API contract** — реализует `RusToKModule` trait корректно.
6. **Migrations** — миграции идемпотентны.
7. **Metadata** — icon, description, license заполнены.

---

## Часть 4: Реализация по всем приложениям

План покрывает **все 4 приложения**. Ниже — что реализовано, что нужно
доработать, и как компоненты переиспользуются между стеками.

### 4.1. Матрица текущего состояния

| Функциональность | Next.js Admin | Next.js Frontend | Leptos Admin | Leptos Storefront |
|---|---|---|---|---|
| **Toggle UI** (`/modules`) | ✅ | — | ✅ | — |
| **GraphQL toggle API** | ✅ | — | ✅ | — |
| **Slot-реестр** | ✅ nav-based | ✅ `home:afterHero` | ✅ `DashboardSection` | ✅ `HomeAfterHero` |
| **Shared пакеты** (`@rustok/*`) | ✅ `blog-admin` | ✅ `blog-frontend` | — (compile-time) | — (compile-time) |
| **`EnabledModulesProvider`** | ❌ | ❌ | ❌ | ❌ |
| **`ModuleGuard`** | ❌ | ❌ | ❌ | ❌ |
| **Фильтрация слотов по enabled** | ❌ | ❌ | ❌ | ❌ |
| **Marketplace каталог UI** | ❌ | — | ❌ | — |
| **Build progress UI** | ❌ | — | ❌ | — |
| **Updates tab** | ❌ | — | ❌ | — |

### 4.2. P1 — EnabledModulesProvider (все 4 приложения)

Каждое приложение должно знать, какие модули включены для текущего тенанта,
чтобы условно показывать/скрывать UI-элементы.

**Next.js Admin:**
```tsx
// app/providers/enabled-modules-provider.tsx
// Server Component → загружает через GraphQL enabledModules query
// Передаёт через React Context
export function EnabledModulesProvider({ children }) {
  const modules = await getEnabledModules(); // GraphQL
  return (
    <EnabledModulesContext.Provider value={modules}>
      {children}
    </EnabledModulesContext.Provider>
  );
}

// hooks/use-enabled-modules.ts
export function useEnabledModules(): string[] {
  return useContext(EnabledModulesContext);
}
```

**Next.js Frontend (storefront):**
```tsx
// Аналогичный провайдер, но данные берёт из серверной сессии
// или из API при SSR. Используется для фильтрации слотов.
export function EnabledModulesProvider({ children }) {
  const modules = await getEnabledModules(); // GraphQL / cookie
  return <EnabledModulesContext.Provider value={modules}>{children}</EnabledModulesContext.Provider>;
}
```

**Leptos Admin:**
```rust
// shared/context/enabled_modules.rs
pub fn provide_enabled_modules_context() {
    let modules = Resource::new(|| (), |_| async { fetch_enabled_modules().await });
    provide_context(EnabledModulesContext { modules });
}

pub fn use_enabled_modules() -> Memo<HashSet<String>> { ... }
```

**Leptos Storefront:**
```rust
// Аналогично, но в SSR-контексте (Axum middleware передаёт enabled modules):
pub fn provide_enabled_modules(enabled: HashSet<String>) {
    provide_context(EnabledModulesContext { enabled });
}
```

### 4.3. P1 — ModuleGuard (все 4 приложения)

Компонент-обёртка для маршрутов/секций, привязанных к модулю.

**Next.js:**
```tsx
function ModuleGuard({ slug, children, fallback }: { slug: string; children: ReactNode; fallback?: ReactNode }) {
  const enabled = useEnabledModules();
  if (!enabled.includes(slug)) return fallback ?? <NotFound />;
  return <>{children}</>;
}

// Использование в layout:
<ModuleGuard slug="blog">
  <BlogPage />
</ModuleGuard>
```

**Leptos:**
```rust
#[component]
fn ModuleGuard(slug: &'static str, children: Children) -> impl IntoView {
    let enabled = use_enabled_modules();
    move || {
        if enabled.get().contains(slug) {
            children().into_any()
        } else {
            view! { <NotFound /> }.into_any()
        }
    }
}
```

### 4.4. P1 — Conditional Sidebar Nav

Сайдбар в обоих админках должен фильтровать nav items по enabled модулям.

**Next.js Admin:**
- `getAdminNavItems()` уже собирает nav из зарегистрированных модулей.
- Нужно добавить фильтрацию: `navItems.filter(item => enabledModules.includes(item.moduleSlug))`.

**Leptos Admin:**
- Sidebar сейчас хардкодит nav items.
- Нужно: модули регистрируют свои nav items через `AdminComponentRegistration`.
- Sidebar вызывает `components_for_slot(AdminSlot::NavItem)` и фильтрует по enabled.

### 4.5. P1 — Storefront Slot Filtering

Оба storefront'а (Next.js и Leptos) должны фильтровать слоты по enabled модулям.

**Next.js Frontend:**
```tsx
// Сейчас:
const modules = getModulesForSlot("home:afterHero");
// Нужно:
const enabled = useEnabledModules();
const modules = getModulesForSlot("home:afterHero")
  .filter(m => enabled.includes(m.moduleSlug));
```

**Leptos Storefront:**
```rust
// Сейчас:
let modules = components_for_slot(StorefrontSlot::HomeAfterHero);
// Нужно:
let enabled = use_enabled_modules();
let modules = components_for_slot(StorefrontSlot::HomeAfterHero)
    .into_iter()
    .filter(|m| enabled.get().contains(m.module_slug))
    .collect::<Vec<_>>();
```

Для этого `StorefrontComponentRegistration` расширяется полем:
```rust
pub struct StorefrontComponentRegistration {
    pub id: &'static str,
    pub module_slug: Option<&'static str>,  // ← NEW
    pub slot: StorefrontSlot,
    pub order: usize,
    pub render: fn() -> AnyView,
}
```

### 4.6. P3 — Marketplace UI (обе админки)

Страница `/modules` расширяется вкладками в **обоих** админках:

| Вкладка | Описание | Next.js Admin | Leptos Admin |
|---|---|---|---|
| Installed | Текущие модули + toggle | ✅ есть | ✅ есть |
| Catalog | Каталог маркетплейса | Нужно | Нужно |
| Updates | Доступные обновления | Нужно | Нужно |
| History | История сборок | Нужно | Нужно |

Оба админа используют **те же GraphQL queries** (`marketplace`, `availableUpdates`,
`buildHistory`), поэтому backend — один, а UI дублируется на React и Leptos.

### 4.7. Shared Packages — стратегия переиспользования

**Next.js экосистема** — npm пакеты:
- `@rustok/blog-admin` — React-компоненты для админки blog.
- `@rustok/blog-frontend` — React-компоненты для storefront blog.
- Пакеты самостоятельно регистрируются в реестре через side-effect import.

**Leptos экосистема** — Rust crate compile-time:
- `rustok-blog` crate содержит `admin/` и `storefront/` директории.
- Компоненты компилируются в бинарник.
- Регистрация через `init_modules()` в `main.rs`.

**Единый crate `rustok-blog`** содержит:
```
rustok-blog/
├── src/lib.rs              # Backend: RusToKModule impl
├── ui/
│   ├── admin/              # → @rustok/blog-admin (Next.js)
│   │   └── ...             # → apps/admin/src/... (Leptos, compile-time)
│   └── frontend/           # → @rustok/blog-frontend (Next.js)
│       └── ...             # → apps/storefront/src/... (Leptos, compile-time)
```

---

## Часть 5: Storefront — модульные слоты (Next.js + Leptos)

### Текущий механизм

Оба storefront'а используют slot-based registry:
- **Next.js**: `StorefrontSlot = "home:afterHero"` (строки, `getModulesForSlot()`).
- **Leptos**: `StorefrontSlot::HomeAfterHero` (enum, `components_for_slot()`).

### Расширение слотов (оба storefront'а)

Набор слотов одинаков для обоих стеков. В Next.js — строковые ключи,
в Leptos — enum. Должны совпадать 1:1.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StorefrontSlot {
    // Layout
    HeaderAfterNav,
    FooterBefore,

    // Home page
    HomeAfterHero,
    HomeBeforeFooter,

    // Product page
    ProductPageSidebar,
    ProductPageAfterDescription,

    // Cart
    CartSummaryAfter,
    CartSidebarWidget,

    // Checkout
    CheckoutPaymentMethods,
    CheckoutAfterOrder,

    // Blog
    BlogSidebar,

    // Global
    GlobalNotificationBar,
}
```

### Условная регистрация

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
    if enabled_modules.contains("commerce") {
        register_component(StorefrontComponentRegistration {
            id: "featured-products",
            slot: StorefrontSlot::HomeAfterHero,
            order: 30,
            render: featured_products_widget,
        });
    }
}
```

---

## Часть 6: Единый стандарт модуля — checklist для авторов

### Структура модуля

```
rustok-blog/
├── rustok-module.toml            # Единый манифест (обязательно)
├── Cargo.toml                    # Rust crate
├── src/
│   ├── lib.rs                    # impl RusToKModule for BlogModule
│   ├── entities/                 # SeaORM entities
│   ├── migration/                # Миграции
│   ├── graphql/                  # GraphQL types, queries, mutations
│   └── events/                   # Event handlers
├── admin/                        # FSD-компоненты для admin UI (опционально)
│   ├── features/
│   │   └── blog/
│   │       ├── components/       # UI компоненты
│   │       └── api.rs            # GraphQL запросы
│   └── pages/                    # Страницы для admin routing
├── storefront/                   # Компоненты для storefront (опционально)
│   └── widgets/
│       └── latest_posts.rs       # Виджет для слотов
├── locales/                      # i18n
│   ├── en.json
│   └── ru.json
├── assets/                       # Для маркетплейса
│   ├── icon.svg
│   └── screenshot-1.png
└── tests/
    └── integration.rs
```

### Контракт модуля — что обязательно

| Требование | Описание |
|---|---|
| `RusToKModule` trait | slug, name, description, version, kind, dependencies |
| `MigrationSource` trait | Список миграций для SeaORM |
| `rustok-module.toml` | Манифест с метаданными для маркетплейса |
| `on_enable` / `on_disable` | Lifecycle hooks (могут быть no-op) |
| Permissions | Список permissions для RBAC |
| i18n | Локализации как минимум en.json |

### Что опционально

| Компонент | Описание |
|---|---|
| Admin UI components | Leptos-компоненты для FSD layers |
| Storefront widgets | Компоненты для slot-системы |
| Event listeners | Обработчики доменных событий |
| Settings schema | JSON Schema для настроек модуля |
| GraphQL extensions | Дополнительные типы, запросы, мутации |

---

## Приоритеты реализации

| Приоритет | Задача | Приложения | Сложность | Статус |
|---|---|---|---|---|
| P0 | Tenant-level toggle (backend) | Server | — | Готово |
| P0 | Leptos admin: страница модулей с toggle | Leptos Admin | — | Готово |
| P0 | Next.js admin: страница модулей с toggle | Next.js Admin | — | Готово |
| P1 | `EnabledModulesProvider` | **Все 4** | Средняя | — |
| P1 | `ModuleGuard` для маршрутов | **Все 4** | Низкая | — |
| P1 | Conditional sidebar nav по enabled модулям | Leptos Admin, Next.js Admin | Низкая | — |
| P1 | Фильтрация storefront слотов по enabled | Leptos Storefront, Next.js Frontend | Низкая | — |
| P2 | `rustok-module.toml` стандарт + валидатор | Server, CLI | Средняя | Частично (Server + xtask baseline) |
| P2 | Manifest Manager (CRUD для modules.toml) | Server | Средняя | — |
| P2 | Build Service API (GraphQL) | Server | Высокая | — |
| P3 | Build Orchestrator (CI/CD интеграция) | Server, CI | Высокая | — |
| P3 | Marketplace каталог UI | Leptos Admin, Next.js Admin | Средняя | — |
| P3 | Build progress UI | Leptos Admin, Next.js Admin | Средняя | Частично (live polling baseline + server subscription contract) |
| P3 | Updates tab UI | Leptos Admin, Next.js Admin | Низкая | — |
| P3 | Cargo features авто-генерация | Server, CI | Низкая | — |
| P4 | Маркетплейс: реестр + GraphQL API | Server (отдельный сервис) | Высокая | — |
| P4 | CLI `rustok module publish` | CLI | Средняя | — |
| P4 | Validation pipeline для публикации | Server (отдельный сервис) | Высокая | — |

---

## Безопасность

- **Нет runtime-подгрузки нативного кода** — все модули компилируются в бинарник.
- **RBAC**: `modules:toggle` для tenant-level; `modules:install` для platform-level.
- **Audit log**: все операции с модулями логируются с автором и причиной.
- **Rollback**: каждый деплой имеет `release_id` для отката через `rollbackBuild`.
- **Валидация зависимостей**: перед install/uninstall проверяется граф.
- **Маркетплейс**: модули проходят validation pipeline перед публикацией.
- **Sandbox**: сторонние модули не имеют доступа к fs/network напрямую — только через platform API.

## Implementation Log (2026-03-08)

- Module detail state on `/modules` is now deep-linkable in both admin apps: the selected catalog entry is mirrored into the `module` query param, so direct links like `/modules?module=blog` reopen the same detail panel and the detail fetch path is unified between click-open and URL-open flows.
- Marketplace catalog filtering was extended across the shared backend and both admin apps: `marketplace(...)` now accepts source-aware filtering on top of trust/compatibility/install-state, and `/modules` exposes `source` plus `installed only` controls while keeping live build refresh pinned to the applied filter set.
- Both admin apps now have a shared catalog filtering baseline on `/modules`: Marketplace and Updates can be narrowed by search, category, trust level, and compatibility-only mode, and live orchestration refresh now respects the active filter set instead of snapping back to an unfiltered catalog snapshot.
- `/modules` in both admin apps now exposes a dedicated module detail panel backed by `marketplaceModule(slug)`: Installed, Marketplace, and Updates cards all share a Details action, and the panel centralizes package metadata, admin-surface policy, compatibility, provenance, and short version history for review.
- Admin P1 completed in both admin apps: EnabledModulesProvider, ModuleGuard, module-aware navigation filtering, and live toggle state sync.
- Storefront P1 completed in both storefront apps: EnabledModulesProvider, ModuleGuard primitives, and slot filtering by enabled modules.
- Shared module registration was updated where needed: @rustok/blog-frontend now registers moduleSlug = "blog" so module-owned widgets disappear when the tenant disables Blog.
- Platform-level backend now has a concrete ManifestManager for modules.toml: install/uninstall/upgrade diffing, deployment-profile derivation, default-enabled sync, and relaxed registry validation so optional modules can be removed before the rebuild deploys.
- Server GraphQL now exposes the first platform lifecycle surface for both admin stacks: installedModules, activeBuild, buildHistory, installModule, uninstallModule, and upgradeModule, all wired to the existing BuildService and current modules:read/list/manage permissions.
- Both admin apps now consume that lifecycle on /modules: they show installed-vs-runtime state, active build status, recent build history, and platform install/uninstall actions next to the existing tenant toggle UX.
- Remaining work is now concentrated in build orchestration/subscriptions and rollback/log streaming UX on top of the new API surface; the shared admin workspace now has Installed, Marketplace, and Updates tabs in both stacks.
- Admin P3 workspace UI completed in both admin apps: /modules now uses consistent Installed, Marketplace, and Updates tabs, backed by installed module metadata and upgrade actions on top of the shared GraphQL lifecycle surface.
- Build progress UX was expanded in both admin apps with progress cards, rebuild history visibility, and queue-state locking around install/uninstall/upgrade actions.
- Rollback/read-side lifecycle is now exposed on the shared GraphQL surface: activeRelease and rollbackBuild are available, and build payloads now carry releaseId, logsUrl, errorMessage, and manifest metadata for richer admin recovery UX.
- Both admin apps now surface active release context in /modules, expose rollback for the currently active releasable build, and show error/log metadata directly inside build history cards.
- Marketplace planning is now explicitly split into two layers: this file tracks execution in the monorepo, while `marketplace-plan.md` owns the future registry/network architecture.
- Both admin apps now use a dedicated `marketplace` catalog query instead of deriving the Marketplace tab from `moduleRegistry`; the server catalog is backed by `modules.toml` as source of truth and enriched with `ModuleRegistry` runtime metadata for current built-in modules.
- The new marketplace catalog contract already carries module source semantics (`path`/future `registry`/future `git`) and installed/update status, so the current UI stays consistent with the long-term marketplace RFC even before the external registry service exists.
- Module catalog metadata now carries ownership/trust/admin-surface policy: first-party vs third-party, trust level, recommended admin surfaces, and showcase-only admin surfaces.
- `/modules` in both admin apps now makes the product policy explicit: Leptos admin is the canonical module surface, while Next admin is treated as a showcase surface and only modules explicitly marked for Next showcase support advertise that path.
- The ownership/trust/admin-surface policy was moved out of GraphQL resolver hardcode into `modules.toml`/manifest metadata, so future first-party and third-party modules can be reviewed directly in the manifest and later mirrored into `rustok-module.toml` or external registry records.
- First-party path modules now carry their own `rustok-module.toml`, while `modules.toml` is back to being the platform composition manifest instead of the place where ownership/trust/admin-surface policy is duplicated.
- Server-side validation now treats `rustok-module.toml` as a real contract for path modules: the catalog fails fast if the file is missing or if ownership/trust/admin-surface metadata is invalid or contradictory.
- `cargo xtask validate-manifest` now validates both `modules.toml` and module-level `rustok-module.toml` files with the same ownership/trust/admin-surface rules, so the contract is no longer runtime-only.
- `/modules` in both admin apps now auto-refreshes orchestration state every 5 seconds while a build is active, so progress/history/release state keeps moving without manual refresh even before a dedicated GraphQL/WebSocket subscription transport lands.
- Server-side build lifecycle now has a dedicated realtime contract: `BuildService` publishes build state changes into a shared build event hub, GraphQL schema exposes `buildProgress`, and module mutations no longer request builds through a noop publisher path.
- Загрузка marketplace-каталога теперь идет через отдельный provider service на сервере с базовым `local-manifest` provider; это оставляет внутренний platform contract свободно меняемым и уже создает точку расширения под будущие `registry`, `git` и private providers из marketplace RFC.
- В плане отдельно зафиксировано, что дальнейшая работа идёт не от "старого API", а от новой эволюционной системы; следующий marketplace-aligned execution step — добавить внутренний `registry` provider skeleton рядом с `local-manifest`, чтобы federated chain проверялась не только по RFC, но и по коду.
- Этот следующий шаг выполнен на execution-слое: `MarketplaceCatalogService::evolutionary_defaults()` теперь поднимает chain `local-manifest -> registry`, `registry` provider читает внутреннюю конфигурацию `RUSTOK_MARKETPLACE_REGISTRY_URL` и пока осознанно работает как skeleton без fetch-логики, а unit-test закрепляет приоритет первого provider по slug и наличие обоих source-слоев в default chain.
- `registry` provider больше не пустой: он умеет делать HTTP fetch по внутреннему endpoint `{registry_url}/catalog`, кэширует каталог в памяти (`RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS`, default 60s), использует таймаут `RUSTOK_MARKETPLACE_REGISTRY_TIMEOUT_MS` (default 3000ms) и при ошибке мягко деградирует в локальный каталог без падения `/modules`.
- Internal registry contract стал versioned: provider теперь ожидает `schema_version = 1`, registry-модули могут нести `rustok_min_version` / `rustok_max_version`, а GraphQL marketplace projection уже рассчитывает `compatible` на стороне платформы по текущей версии RusTok вместо слепого списка модулей.
- Catalog provenance тоже начал жить в internal contract: registry payload и GraphQL projection теперь готовы к `publisher`, `checksum_sha256`, `signature_present` и `versions[]`, so future admin/detail UI сможет показывать verification trail и history без нового перелома схемы.
- GraphQL marketplace contract перестал быть list-only: `marketplace(...)` теперь поддерживает richer optional filters (`trustLevel`, `onlyCompatible`, `installedOnly`), а `marketplaceModule(slug)` уже отдает detail-ready projection поверх того же catalog/provider слоя.
- `/modules` в обеих админках теперь реально подхватывает новый catalog detail surface: карточки marketplace/install/update показывают compatibility, publisher, checksum/signature provenance и короткий version trail, так что verification/history metadata уже дошла до UI, а не осталась только в schema.
