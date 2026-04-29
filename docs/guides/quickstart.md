# RusToK Quickstart Guide

Быстрый старт для локальной разработки с двумя админками (Next.js + Leptos) и двумя витринами.

## 🚀 Запуск одной командой

```bash
# 1. Клонировать репозиторий (если еще не сделано)
git clone <repo-url>
cd RusTok

# 2. Запустить весь стек
./scripts/dev-start.sh
```

Скрипт автоматически:
- создаст `.env.dev` из `.env.dev.example` (если не существует);
- поднимет PostgreSQL;
- запустит backend (`apps/server`);
- запустит обе админки (Next.js на `:3000`, Leptos на `:3001`);
- запустит обе витрины (Next.js на `:3100`, Leptos на `:3101`).

Источник: [`scripts/dev-start.sh`](../../scripts/dev-start.sh).

## 📱 Доступ к сервисам

### Backend
- **API Server**: <http://localhost:5150>
- **GraphQL Endpoint**: <http://localhost:5150/api/graphql>
- **Health Check**: <http://localhost:5150/api/health>

### Админки
- **Next.js Admin**: <http://localhost:3000>
- **Leptos Admin**: <http://localhost:3001>

### Витрины
- **Next.js Storefront**: <http://localhost:3100>
- **Leptos Storefront**: <http://localhost:3101>

### База данных
- **PostgreSQL**: `localhost:5432`
- **Database**: `rustok_dev`
- **User**: `rustok`
- **Password**: `rustok`

## 🔑 Тестовые данные

Для входа в dev-окружение:

```text
Email:    admin@local
Password: admin12345
```

## 🛠 Полезные команды

```bash
# Остановить все сервисы
./scripts/dev-start.sh stop

# Перезапустить
./scripts/dev-start.sh restart

# Логи
./scripts/dev-start.sh logs
./scripts/dev-start.sh logs server

# Статус
./scripts/dev-start.sh status

# Запуск только админ-профиля
./scripts/dev-start.sh start admin

# Помощь
./scripts/dev-start.sh --help
```

## 🔧 Ручной запуск без Docker

### Installer preflight / plan

Product installer развивается как гибридный слой поверх `rustok-installer`.
На текущем этапе доступны безопасные `preflight`/`plan` команды, которые не
подключаются к БД и не запускают миграции:

```bash
cargo run -p rustok-server --bin rustok-server -- install preflight \
  --environment local \
  --profile dev-local \
  --database-engine postgres \
  --database-url postgres://rustok:rustok@localhost:5432/rustok_dev \
  --admin-email admin@local \
  --admin-password admin12345 \
  --tenant-slug demo \
  --tenant-name "Demo Workspace" \
  --seed-profile dev \
  --secrets-mode dotenv-file

cargo run -p rustok-server --bin rustok-server -- install plan \
  --environment production \
  --profile monolith \
  --database-engine postgres \
  --database-secret-ref vault:rustok/database-url \
  --admin-email admin@example.com \
  --admin-password-ref vault:rustok/admin-password \
  --tenant-slug default \
  --tenant-name "Default Workspace" \
  --seed-profile minimal \
  --secrets-mode external-secret
```

`preflight` возвращает JSON report с warning/error issues. `plan` возвращает
redacted snapshot и никогда не печатает plaintext secrets.

`apply` выполняет текущий CLI bootstrap end-to-end: preflight, проверку target DB
через `SELECT 1`, server `Migrator::up`, tenant/module seed, создание или
синхронизацию superadmin, verify и finalize. Команда создаёт installer session,
ставит lock, записывает receipts `Preflight` / `Config` / `Database` /
`Migrate` / `Seed` / `Admin` / `Verify` / `Finalize` и переводит session в
`completed`.

```bash
cargo run -p rustok-server --bin rustok-server -- install apply \
  --environment local \
  --profile dev-local \
  --database-engine postgres \
  --database-url postgres://rustok:rustok@localhost:5432/rustok_dev \
  --admin-email admin@local \
  --admin-password admin12345 \
  --tenant-slug demo \
  --tenant-name "Demo Workspace" \
  --seed-profile dev \
  --secrets-mode dotenv-file \
  --lock-owner local-cli
```

Если нужно сначала создать PostgreSQL database/role, добавьте `--create-database`.
По умолчанию installer использует admin URL
`postgres://postgres:postgres@localhost:5432/postgres`; для другого admin-пользователя
передайте его явно.

```bash
cargo run -p rustok-server --bin rustok-server -- install apply \
  --database-url postgres://rustok:rustok@localhost:5432/rustok_dev \
  --create-database \
  --pg-admin-url postgres://postgres:<password>@localhost:5432/postgres
```

`install apply` резолвит локальные secret refs без вывода plaintext в receipts:

```bash
cargo run -p rustok-server --bin rustok-server -- install apply \
  --database-secret-ref env:DATABASE_URL \
  --admin-password-ref env:SUPERADMIN_PASSWORD \
  --admin-email admin@local \
  --tenant-slug demo \
  --tenant-name "Demo Workspace" \
  --seed-profile minimal \
  --secrets-mode env

cargo run -p rustok-server --bin rustok-server -- install apply \
  --database-secret-ref dotenv:.env.dev#DATABASE_URL \
  --admin-password-ref file:/run/secrets/rustok_admin_password \
  --admin-email admin@local \
  --tenant-slug demo \
  --tenant-name "Demo Workspace" \
  --seed-profile minimal \
  --secrets-mode mounted-file
```

Поддержанные backends для `apply`: `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` и `dotenv:<VAR>` для чтения из
локального `.env`. External backends вроде `vault:*`, `kubernetes:*` и cloud
secret managers пока являются contract-level refs для `plan`/`preflight`, но
`apply` завершится с явной ошибкой до подключения resolver-а.

### Installer HTTP adapter

Leptos wizard должен использовать тонкий HTTP adapter, а не повторять bootstrap
logic в UI:

- `GET /api/install/status`
- `POST /api/install/plan`
- `POST /api/install/preflight`
- `POST /api/install/apply` — возвращает `202 Accepted` и `job_id`
- `GET /api/install/jobs/{job_id}` — polling статуса background job
- `GET /api/install/sessions/{session_id}/receipts` — persisted step receipts

Для mutating HTTP install requests можно задать setup token:

```powershell
$env:RUSTOK_INSTALL_SETUP_TOKEN="local-setup-token"
```

Клиент передаёт его через `x-rustok-setup-token` или
`Authorization: Bearer <token>`. Production HTTP apply без
`RUSTOK_INSTALL_SETUP_TOKEN` отклоняется; CLI остаётся canonical путь для CI/CD
и headless installs.

Wizard flow: отправить `plan`, затем `preflight`; после успешного preflight
вызвать `apply`, сохранить `job_id`, poll-ить `/api/install/jobs/{job_id}` до
`succeeded` или `failed`, а progress-ленту строить из
`/api/install/sessions/{session_id}/receipts`, когда `session_id` появился в
job output или `/api/install/status`.

### Bootstrap без Docker Compose

Канонический путь локальной установки без Docker Compose:

```bash
cargo xtask install-dev --create-db
```

Если PostgreSQL admin-пользователь отличается от `postgres:postgres`, передайте его явно:

```bash
cargo xtask install-dev --create-db --pg-admin-url postgres://postgres:<password>@localhost:5432/postgres
```

Команда проверяет локальные инструменты, готовит `.env.dev`, `apps/next-admin/.env.local`,
создаёт `modules.local.toml` для standalone UI и делегирует bootstrap в
`target/debug/rustok-server install apply`: миграции, dev seed, superadmin,
verify/finalize и installer receipts проходят через один install pipeline.
После bootstrap сервер и админки запускаются отдельно, чтобы логи и debug-сессии не смешивались.
Локальный `development.yaml` при этом оставляет full backend surface, но отключает maintenance workers
`workflow_cron_enabled` и `seo_bulk_enabled`, чтобы интерактивная отладка админок не конкурировала с cron/bulk loops за DB pool.

Если `target/debug/rustok-server` ещё не собран, сначала выполните:

```bash
cargo build -p rustok-server --bin rustok-server
cargo xtask install-dev
```

### Требования
- Rust toolchain (см. `rust-toolchain.toml`)
- Node.js/Bun для Next.js приложений
- PostgreSQL
- Trunk для Leptos приложений (`cargo install trunk`)

### Запуск

```bash
# backend
cd apps/server
cargo run

# next admin
cd apps/next-admin
bun install
bun run dev

# leptos admin
cd apps/admin
trunk serve --port 3001
```

`apps/admin/Trunk.toml` проксирует `/api/*` в `http://localhost:5150/api/*`, поэтому standalone
CSR-debug не должен зависеть от Leptos `#[server]` endpoints. SSR/monolith профили продолжают
использовать `/api/fn/*` как native transport.

## 📚 Связанные документы

- [Docs index](../index.md)
- [UI documentation hub](../UI/README.md)
- [Admin ↔ Server connection](../UI/admin-server-connection-quickstart.md)
- [apps/next-admin README](../../apps/next-admin/README.md)
- [apps/admin docs](../../apps/admin/docs/README.md)
