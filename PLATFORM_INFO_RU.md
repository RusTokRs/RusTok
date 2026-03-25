# <img src="assets/rustok-logo-512x512.png" width="72" align="center" /> RusToK — Краткая справка о платформе

**Event-Driven Headless Platform на Rust**

---

## Что такое RusToK

RusToK — это высокопроизводительная headless-платформа для e-commerce и контентных проектов, написанная на Rust. Платформа сочетает производительность compiled-кода с удобством разработки благодаря фреймворку Loco.rs.

### Ключевые принципы

- **Модульность** — каждый функциональный блок изолирован и микросервис-готов
- **Событийная архитектура** — асинхронная синхронизация между модулями через event bus
- **Multi-tenant** — изоляция данных между арендаторами на уровне платформы
- **CQRS-lite** — разделение write и read моделей для высокой производительности

---

## Архитектура

```
┌─────────────────────────────────────────────────────────────┐
│                      RusToK Platform                        │
├─────────────────────────────────────────────────────────────┤
│  🛍️ Storefront (SSR)  │  ⚙️ Admin Panel  │  📱 Mobile App   │
│      Leptos SSR       │    Leptos CSR    │   GraphQL API    │
├─────────────────────────────────────────────────────────────┤
│                    🔌 GraphQL + REST API                    │
├─────────────────────────────────────────────────────────────┤
│  📦 Commerce  │  📝 Content  │  👥 Community  │ 🏷️ Index     │
├─────────────────────────────────────────────────────────────┤
│                    🧠 Core (Loco.rs)                        │
│          Auth • Tenants • Events • RBAC • Cache             │
├─────────────────────────────────────────────────────────────┤
│     🐘 PostgreSQL (write)  │  🔎 Index Module (read)        │
└─────────────────────────────────────────────────────────────┘
```

---

## Технический стек

| Компонент | Технология |
|-----------|------------|
| **Язык** | Rust 1.80+ |
| **Рантайм** | Tokio (async) |
| **Веб-фреймворк** | Loco.rs + Axum |
| **ORM** | SeaORM |
| **База данных** | PostgreSQL 16 |
| **GraphQL** | async-graphql |
| **REST/OpenAPI** | utoipa + Swagger UI |
| **Frontend** | Leptos (Rust/WASM), Next.js |
| **Кэш** | Redis (опционально) |
| **События** | Outbox pattern + Iggy |
| **Авторизация** | JWT + Argon2 |

---

## Структура проекта

```
RusToK/
├── apps/
│   ├── server/          # 🚀 Backend API (Loco.rs)
│   ├── admin/           # ⚙️ Admin Panel (Leptos CSR)
│   ├── storefront/      # 🛍️ Витрина (Leptos SSR)
│   ├── next-admin/      # ⚙️ Admin (Next.js)
│   ├── next-frontend/   # 🛍️ Витрина (Next.js)
│   └── mcp/             # 🤖 MCP сервер
│
├── crates/
│   ├── rustok-core/     # 🧠 Ядро (Auth, Events, RBAC)
│   ├── rustok-content/  # 📝 CMS (Nodes, Categories)
│   ├── rustok-commerce/ # 🛒 Ecommerce umbrella (корень семейства e-commerce)
│   ├── rustok-blog/     # 📰 Блог
│   ├── rustok-forum/    # 👥 Форум
│   ├── rustok-index/    # 🔎 Поиск и индексы
│   ├── rustok-tenant/   # 🏢 Мультиарендность
│   ├── rustok-rbac/     # 🔐 Права доступа
│   ├── rustok-outbox/   # 📤 Transactional Outbox
│   └── ...
│
└── docs/                # 📚 Документация
```

---

## Быстрый старт

```bash
# Клонирование
git clone https://github.com/RustokCMS/RusToK.git
cd RusToK

# База данных (Docker)
docker run -d --name rustok-db \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=rustok_dev \
  -p 5432:5432 \
  postgres:16

# Запуск бэкенда
cd apps/server
cargo loco db migrate
cargo loco start

# Запуск админки (в другом терминале)
cd apps/admin
RUSTOK_DEMO_MODE=1 trunk serve --open
```

**URL после запуска:**

- API: <http://localhost:3000/api/graphql>
- Admin: <http://localhost:8080>
- Storefront: <http://localhost:3100>

---

## Преимущества

| Метрика | RusToK | WordPress | Strapi |
|---------|--------|-----------|--------|
| Язык | Rust | PHP | Node.js |
| Запросов/сек | 45,000+ | ~60 | ~800 |
| P99 latency | 8ms | 450ms | 120ms |
| Память | 30-50MB | 50-100MB | 200-500MB |
| Type Safety | Полная | Нет | Частичная |
| Multi-tenant | Нативная | Ограниченно | Ограниченно |

---

## Документация

- **[Полная документация](docs/index.md)** — карта всех документов
- **[Архитектура](docs/architecture/overview.md)** — технический обзор
- **[Модули](docs/modules/registry.md)** — реестр модулей
- **[Roadmap](docs/roadmap.md)** — план развития
- **[Changelog](CHANGELOG.md)** — история изменений

---

## Лицензия

MIT License — см. [LICENSE](LICENSE)

---

⬆ [Наверх](#-rustok--краткая-справка-о-платформе)  
<img src="assets/rustok-logo-32x32.png" width="24" align="center" /> Сделано с любовью к Rust
