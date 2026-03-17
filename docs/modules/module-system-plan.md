# RusTok — Система модулей: план доработки

> **Статус**: Активный план (только незавершённые задачи)
> **Дата**: 2026-03-17
>
> Справочная документация по реализованной части:
> - `docs/architecture/modules.md` — архитектура модульной системы, два уровня операций
> - `docs/modules/manifest.md` — формат `modules.toml` и `rustok-module.toml`

---

## Что осталось сделать

### ⚠️ Частично реализовано

---

#### 1. Docker deploy в BuildExecutor

**Текущее состояние**: `BuildExecutor` выполняет только `cargo build`. После успешной
компиляции создаётся запись в `releases`, но реального деплоя не происходит.

**Что нужно добавить** в `apps/server/src/services/build_executor.rs`:

```rust
// После успешного cargo build:
// Stage: Deploy (progress 85-99%)

// 1. docker build -t rustok-server:{release_id} .
// 2. docker push {registry}/{image}:{tag}
// 3. Обновить release.container_image
// 4. Rolling restart (или через оркестратор)

// Для monolith-режима: просто перезапуск процесса
// Для K8s: обновить image в deployment через kubectl / Helm
```

Поля `releases.container_image`, `releases.server_artifact_url` — есть в схеме,
но не заполняются. `ReleaseStatus::Deploying` / `Active` — есть, но не используются
корректно.

**Конфигурация** (через env vars, по аналогии с `RUSTOK_BUILD_CARGO_BIN`):
- `RUSTOK_BUILD_DOCKER_BIN` — путь к docker (default: `docker`)
- `RUSTOK_BUILD_REGISTRY` — registry URL для push
- `RUSTOK_DEPLOY_MODE` — `monolith` | `docker` | `k8s`

---

#### 2. Build progress UI — подключить WebSocket subscription

**Текущее состояние**: прогресс-бар в `/modules` обновляется polling'ом раз в 5 секунд
(`use_interval_fn(refresh_live_state, 5000)`). `buildProgress` GraphQL subscription
реализована на бэке (`apps/server/src/graphql/subscriptions.rs`), но UI к ней
не подключён.

**Что нужно** в `apps/admin/src/features/modules/components/modules_list.rs`:

```rust
// Заменить polling на subscription:

// Вместо:
use_interval_fn(refresh_live_state, 5000);

// Использовать:
let _sub = use_graphql_subscription::<BuildProgressSubscription>(
    BuildProgressSubscriptionVariables { build_id: active_build_id },
    move |event| set_build.set(Some(event.build_progress)),
);
```

`leptos-graphql` уже поддерживает subscriptions — инфраструктура есть.

---

#### 3. Semver-валидация зависимостей и конфликтов

**Текущее состояние**: `ManifestManager::validate()` проверяет только факт наличия
зависимости (slug присутствует в манифесте). Диапазоны версий (`>= 1.0.0`, `~2.0`)
и секция `[conflicts]` в `rustok-module.toml` — игнорируются.

**Что нужно** в `apps/server/src/modules/manifest.rs`:

```rust
// Сейчас:
fn validate_dependencies(manifest: &ModulesManifest) -> Result<()> {
    for (slug, spec) in &manifest.modules {
        for dep in &spec.depends_on {
            if !manifest.modules.contains_key(dep) {
                return Err(MissingDependency { slug, dep });
            }
            // ← версия не проверяется
        }
    }
}

// Нужно:
// 1. Парсить version_req из rustok-module.toml [dependencies]
//    например: content = ">= 1.0.0"
// 2. Парсить installed version из модуля
// 3. semver::VersionReq::parse(req)?.matches(&installed_version)
// 4. Проверять [conflicts]: если конфликтующий модуль установлен → ошибка
```

Добавить зависимость `semver = "1"` в `apps/server/Cargo.toml`.

Затронутые места:
- `ManifestManager::validate()` — добавить semver-проверки
- `ManifestManager::install_builtin_module()` — проверять конфликты перед добавлением
- `ManifestManager::upgrade_module()` — проверять, не ломает ли новая версия dependents

---

### ⬜ Не реализовано

---

#### 4. API настроек модуля (`updateModuleSettings`)

**Текущее состояние**: колонка `settings JSON` в `tenant_modules` существует,
`on_enable()` может записать дефолты — но нет GraphQL мутации для обновления
настроек через UI.

**Что нужно**:

```graphql
type Mutation {
  updateModuleSettings(moduleSlug: String!, settings: JSON!): TenantModule!
}
```

Серверная сторона (`apps/server/src/graphql/mutations.rs`):

```rust
async fn update_module_settings(
    &self, ctx: &Context<'_>,
    module_slug: String,
    settings: serde_json::Value,
) -> Result<TenantModule> {
    // 1. Проверить, что модуль включён для тенанта
    // 2. Валидировать settings по JSON Schema из rustok-module.toml [settings]
    // 3. UPDATE tenant_modules SET settings = ? WHERE tenant_id = ? AND module_slug = ?
    // 4. Return updated TenantModule
}
```

UI (`apps/admin/src/features/modules/`):
- Форма настроек генерируется из `[settings]` секции `rustok-module.toml`
- Показывается в детальной панели модуля на `/modules`

---

#### 5. Внешний реестр `modules.rustok.dev`

Скелет `RegistryMarketplaceProvider` готов и делает HTTP-запросы, но сам сервис
реестра не существует.

**Что входит в scope**:

```
modules.rustok.dev
├── GraphQL API (каталог, версии, поиск)
├── Crate Storage (S3: .crate архивы + checksums)
└── Validation Pipeline (static → audit → compile → test → metadata)
```

Аутентификация: см. `docs/concepts/plan-oauth2-app-connections.md` (Приложение A).

**Минимальный V1 реестра** — только read-only каталог для встроенных first-party
модулей. Это позволяет проверить весь `RegistryMarketplaceProvider` → AdminUI flow
без publish pipeline.

**Полный V2** — publish pipeline, `rustok mod publish` CLI, третьесторонние модули.

---

#### 6. Publish pipeline и `rustok mod publish`

Зависит от п.5 (внешний реестр).

```bash
rustok mod init          # Создать шаблон с rustok-module.toml
rustok mod validate      # Локальная проверка манифеста
rustok mod test          # Validation pipeline локально
rustok mod publish       # Опубликовать в реестр
rustok mod yank 1.2.0    # Отозвать версию
```

Validation pipeline (5 стадий):
1. **Static checks** — манифест, slug, semver, license, locales/en.json
2. **Security audit** — cargo-audit, unsafe-check, отсутствие std::process::Command
3. **Compilation** — с rustok_min..rustok_max версиями платформы
4. **Runtime tests** — cargo test, миграции up/down, on_enable/on_disable
5. **Metadata quality** — icon, description length, screenshots

---

## Приоритет

| # | Задача | Сложность | Ценность |
|---|---|---|---|
| 1 | Semver-валидация | Малая | Высокая — защита от broken installs |
| 2 | `updateModuleSettings` | Малая | Высокая — модули уже имеют `[settings]` |
| 3 | Build progress subscription | Малая | Средняя — UX улучшение |
| 4 | Docker deploy | Средняя | Высокая — без этого install не работает в prod |
| 5 | Внешний реестр V1 | Большая | Высокая — основа marketplace |
| 6 | Publish pipeline + CLI | Очень большая | Средняя — нужен только для 3rd party |
