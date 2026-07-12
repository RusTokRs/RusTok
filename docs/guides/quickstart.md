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

### Installer HTTP Adapter

The product installer is a hybrid layer on top of `rustok-installer`.
The production server binary does not parse `install` commands. The typed
platform CLI provides `install plan`, `install preflight`, `install apply`,
`install status`, and `seed apply`. `install apply` uses the shared executor-port
extraction; use the HTTP adapter when an interactive wizard is required.

The Leptos wizard should use a thin HTTP adapter rather than duplicating
bootstrap logic in the UI:

- `GET /api/install/status`
- `POST /api/install/plan`
- `POST /api/install/preflight`
- `POST /api/install/apply` returns `202 Accepted` and `job_id`
- `GET /api/install/jobs/{job_id}` polls the background job
- `GET /api/install/sessions/{session_id}/receipts` reads persisted receipts

`POST /api/install/plan` and `POST /api/install/preflight` accept an
`InstallPlan` JSON body. `POST /api/install/apply` accepts
`{ "plan": <InstallPlan>, "lock_owner": "operator", "lock_ttl_secs": 900 }`.
It records redacted receipts for preflight, config, database, migration, seed,
admin, verify, and finalize stages.

For mutating HTTP install requests, configure a setup token:

```powershell
$env:RUSTOK_INSTALL_SETUP_TOKEN="local-setup-token"
```

The client sends it with `x-rustok-setup-token` or
`Authorization: Bearer <token>`. Production HTTP apply without the token is
rejected.

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
creates `modules.local.toml` for standalone UI, then delegates schema migration
and the development seed profile to `target/debug/rustok-cli`. It is a local
convenience wrapper, not a replacement for the durable installer HTTP pipeline.
After bootstrap, the server and admin panels are started separately so that logs and debug sessions do not mix.
The local `development.yaml` retains the full backend surface but disables maintenance workers
`workflow_cron_enabled` and `seo_bulk_enabled`, so that interactive admin debugging does not compete with cron/bulk loops for the DB pool.

If `target/debug/rustok-cli` is not yet built, first run:

```bash
cargo build -p rustok-cli --bin rustok-cli
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
