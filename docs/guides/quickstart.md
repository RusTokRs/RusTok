---
id: doc://docs/guides/quickstart.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# RusToK Quickstart Guide

Quick start for local development with two admin panels (Next.js + Leptos) and two storefronts.

## Launch Profile Matrix (canonical truth table)

| Profile | Hosts | Ports | Profile Owner | Canonical source |
|---|---|---|---|---|
| `dev-start:full` | `apps/server`, `apps/next-admin`, `apps/admin`, `apps/next-frontend`, `apps/storefront`, PostgreSQL | `5150`, `3000`, `3001`, `3100`, `3101`, `5432` | Platform + DevEx | `scripts/dev-start.sh`, `docs/guides/quickstart.md` |
| `dev-start:admin` | `apps/server`, `apps/next-admin`, `apps/admin`, PostgreSQL | `5150`, `3000`, `3001`, `5432` | Platform + DevEx | `scripts/dev-start.sh start admin`, `docs/guides/quickstart.md` |
| `local:ssr-install` | `apps/server` (installer/apply pipeline), PostgreSQL | `5150`, `5432` | Platform foundation | `cargo xtask install-dev`, `apps/server/config/development.yaml` |
| `standalone:next-admin` | `apps/server` + `apps/next-admin` | `5150`, `3000` | Frontend admin owner | `apps/next-admin`, `docs/UI/admin-server-connection-quickstart.md` |
| `standalone:leptos-admin` | `apps/server` + `apps/admin` | `5150`, `3001` | Frontend admin owner | `apps/admin/Trunk.toml`, `docs/UI/admin-server-connection-quickstart.md` |
| `headless:next-frontend` | `apps/server` + `apps/next-frontend` | `5150`, `3100` | Frontend storefront owner | `apps/next-frontend`, `docs/UI/storefront.md` |
| `standalone:leptos-storefront` | `apps/server` + `apps/storefront` | `5150`, `3101` | Frontend storefront owner | `apps/storefront`, `docs/UI/storefront.md` |

Notes:
- The source of truth for module composition remains `modules.toml`; launch profiles
  do not change the module contract, they only define the runtime topology and host composition.
- In case of documentation conflict, `scripts/dev-start.sh` (for dev-start profiles)
  and the install/host entrypoints listed in the `Canonical source` column take precedence.

## 🚀 One-Command Launch

```bash
# 1. Clone the repository (if not already done)
git clone <repo-url>
cd RusTok

# 2. Start the entire stack
./scripts/dev-start.sh
```

The script automatically:
- creates `.env.dev` from `.env.dev.example` (if it does not exist);
- starts PostgreSQL;
- launches the backend (`apps/server`);
- launches both admin panels (Next.js on `:3000`, Leptos on `:3001`);
- launches both storefronts (Next.js on `:3100`, Leptos on `:3101`).

Source: [`scripts/dev-start.sh`](../../scripts/dev-start.sh).

## 📱 Service Access

### Backend
- **API Server**: <http://localhost:5150>
- **GraphQL Endpoint**: <http://localhost:5150/api/graphql>
- **Health Check**: <http://localhost:5150/api/health>

### Admin Panels
- **Next.js Admin**: <http://localhost:3000>
- **Leptos Admin**: <http://localhost:3001>

### Storefronts
- **Next.js Storefront**: <http://localhost:3100>
- **Leptos Storefront**: <http://localhost:3101>

### Database
- **PostgreSQL**: `localhost:5432`
- **Database**: `rustok_dev`
- **User**: `rustok`
- **Password**: `rustok`

## 🔑 Test Data

For logging into the dev environment:

```text
Email:    admin@local
Password: admin12345
```

## 🛠 Useful Commands

```bash
# Stop all services
./scripts/dev-start.sh stop

# Restart
./scripts/dev-start.sh restart

# Logs
./scripts/dev-start.sh logs
./scripts/dev-start.sh logs server

# Status
./scripts/dev-start.sh status

# Start admin profile only
./scripts/dev-start.sh start admin

# Help
./scripts/dev-start.sh --help
```

## 🔧 Manual Launch Without Docker

### Installer Preflight / Plan

The product installer is evolving as a hybrid layer on top of `rustok-installer`.
At the current stage, the safe `preflight`/`plan` commands are available, which do not
connect to the DB or run migrations:

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

`preflight` returns a JSON report with warning/error issues. `plan` returns a
redacted snapshot and never prints plaintext secrets.

`apply` runs the current CLI bootstrap end-to-end: preflight, target DB check
via `SELECT 1`, server `Migrator::up`, tenant/module seed, creating or
syncing the superadmin, verify and finalize. The command creates an installer session,
places a lock, writes `Preflight` / `Config` / `Database` /
`Migrate` / `Seed` / `Admin` / `Verify` / `Finalize` receipts and transitions the session to
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

If you need to create the PostgreSQL database/role first, add `--create-database`.
By default, the installer uses the admin URL
`postgres://postgres:postgres@localhost:5432/postgres`; to use a different admin user,
pass it explicitly.

```bash
cargo run -p rustok-server --bin rustok-server -- install apply \
  --database-url postgres://rustok:rustok@localhost:5432/rustok_dev \
  --create-database \
  --pg-admin-url postgres://postgres:<password>@localhost:5432/postgres
```

`install apply` resolves local secret refs without outputting plaintext in receipts:

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

Supported backends for `apply`: `env:<VAR>`, `file:<path>`,
`mounted-file:<path>`, `dotenv:<path>#<VAR>` and `dotenv:<VAR>` for reading from
a local `.env`. External backends like `vault:*`, `kubernetes:*` and cloud
secret managers are currently contract-level refs for `plan`/`preflight`, but
`apply` will fail with an explicit error until the resolver is connected.

### Installer HTTP Adapter

The Leptos wizard should use a thin HTTP adapter rather than duplicating bootstrap
logic in the UI:

- `GET /api/install/status`
- `POST /api/install/plan`
- `POST /api/install/preflight`
- `POST /api/install/apply` — returns `202 Accepted` and `job_id`
- `GET /api/install/jobs/{job_id}` — polling status of background job
- `GET /api/install/sessions/{session_id}/receipts` — persisted step receipts

For mutating HTTP install requests, a setup token can be set:

```powershell
$env:RUSTOK_INSTALL_SETUP_TOKEN="local-setup-token"
```

The client sends it via `x-rustok-setup-token` or
`Authorization: Bearer <token>`. Production HTTP apply without
`RUSTOK_INSTALL_SETUP_TOKEN` is rejected; the CLI remains the canonical path for CI/CD
and headless installs.

Wizard flow: send `plan`, then `preflight`; after a successful preflight,
call `apply`, save `job_id`, poll `/api/install/jobs/{job_id}` until
`succeeded` or `failed`, and build the progress stream from
`/api/install/sessions/{session_id}/receipts`, once `session_id` appears in
the job output or `/api/install/status`.

### Bootstrap Without Docker Compose

The canonical path for local setup without Docker Compose:

```bash
cargo xtask install-dev --create-db
```

If the PostgreSQL admin user is different from `postgres:postgres`, pass it explicitly:

```bash
cargo xtask install-dev --create-db --pg-admin-url postgres://postgres:<password>@localhost:5432/postgres
```

The command checks local tools, prepares `.env.dev`, `apps/next-admin/.env.local`,
creates `modules.local.toml` for standalone UI and delegates bootstrap to
`target/debug/rustok-server install apply`: migrations, dev seed, superadmin,
verify/finalize and installer receipts all go through a single install pipeline.
After bootstrap, the server and admin panels are started separately so that logs and debug sessions do not mix.
The local `development.yaml` retains the full backend surface but disables maintenance workers
`workflow_cron_enabled` and `seo_bulk_enabled`, so that interactive admin debugging does not compete with cron/bulk loops for the DB pool.

If `target/debug/rustok-server` is not yet built, first run:

```bash
cargo build -p rustok-server --bin rustok-server
cargo xtask install-dev
```

### Requirements
- Rust toolchain (see `rust-toolchain.toml`)
- Node.js/Bun for Next.js applications
- PostgreSQL
- Trunk for Leptos applications (`cargo install trunk`)

### Launch

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

`apps/admin/Trunk.toml` proxies `/api/*` to `http://localhost:5150/api/*`, so standalone
CSR-debug should not depend on Leptos `#[server]` endpoints. SSR/monolith profiles continue
to use `/api/fn/*` as the native transport.

## 📚 Related Documents

- [Docs index](../index.md)
- [UI documentation hub](../UI/README.md)
- [Admin ↔ Server connection](../UI/admin-server-connection-quickstart.md)
- [apps/next-admin README](../../apps/next-admin/README.md)
- [apps/admin docs](../../apps/admin/docs/README.md)
