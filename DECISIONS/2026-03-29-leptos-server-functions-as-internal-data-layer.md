# Leptos `#[server]` functions как внутренний слой данных для Leptos-приложений

- Date: 2026-03-29
- Status: Accepted
- Supersedes: `2026-03-07-deployment-profiles-and-ui-stack.md` (в части транспорта между Leptos UI и сервером)

## Context

Сейчас оба Leptos-приложения общаются с сервером через HTTP + GraphQL, даже когда
запущены в одном процессе (монолитный деплой):

```
Монолит сейчас:
  browser → HTTP POST /api/graphql → GraphQL resolver → service layer → DB
```

`apps/admin` — чистый CSR (`features = ["csr"]`), WASM-бандл, раздаётся как
статика из сервера. Всегда работает в браузере, всегда HTTP.

`apps/storefront` — чистый SSR (`features = ["ssr"]`), Axum-сервер рендерит HTML,
но за данными ходит в GraphQL через `reqwest`. HTTP присутствует даже в монолите.

Это означает: обещание «монолит — один бинарник» реализовано лишь на уровне
упаковки артефактов, но не на уровне транспорта. Между слоями приложения всё
равно есть HTTP, сериализация GraphQL и resolver-слой.

## Decision

### Принцип

**Leptos `#[server]`-функции — единственный транспорт между Leptos UI и серверным
слоем данных.**

GraphQL остаётся, но только как **внешний API** для headless-клиентов: Next.js,
мобильные приложения, сторонние интеграции. Leptos-приложения GraphQL не используют.

### Как работают `#[server]`-функции в разных режимах

```rust
#[server]
pub async fn list_users(page: u32) -> Result<Vec<User>, ServerFnError> {
    // Этот код выполняется только на сервере.
    // В монолите — in-process вызов сервисного слоя.
    // В headless — HTTP POST на /api/fn/list_users.
    UserService::list(page).await
}
```

| Контекст | Что происходит |
|---|---|
| SSR-рендер в монолите | Прямой in-process вызов, никакого HTTP |
| Гидратация / client navigation | HTTP POST `/api/fn/<name>` (бинарный кодек) |
| Standalone Leptos (headless деплой) | HTTP POST `/api/fn/<name>` к удалённому серверу |

Один и тот же код, разный транспорт в зависимости от топологии — без изменений
в логике приложения.

### Изменения в `apps/admin`

**Было:** `features = ["csr"]`, WASM SPA, GraphQL через `reqwest`.

**Станет:** `features = ["ssr", "hydrate"]` — приложение рендерится на сервере
при первом запросе, гидратируется в браузере для интерактивности. Данные через
`#[server]`-функции.

```
Монолит после:
  HTTP request → Axum → Leptos SSR render → #[server] fn → service layer → DB
                                          ↑ in-process, без HTTP
```

После гидратации навигация внутри admin делает POST на `/api/fn/*` — это HTTP,
но в пределах одного процесса, короткий путь без GraphQL resolver-слоя.

### Изменения в `apps/storefront`

**Было:** `features = ["ssr"]`, GraphQL через `reqwest`, без гидратации.

**Станет:** `features = ["ssr", "hydrate"]` (там где нужна интерактивность),
данные через `#[server]`-функции. Статические страницы могут остаться без
гидратации — только SSR.

### GraphQL — только внешний контракт

GraphQL API (`/api/graphql`) продолжает существовать и поддерживаться. Он нужен:

- `apps/next-admin` — Next.js панель администратора
- `apps/next-frontend` — Next.js storefront
- Мобильные клиенты
- Сторонние интеграции

Leptos-приложения GraphQL больше не используют. Это разделяет две роли:
`#[server]` — внутренний типобезопасный транспорт, GraphQL — внешний контракт.

### Итоговые профили деплоя

**Чистый монолит (WordPress-стиль):**
```toml
[build.server]
embed_admin = true       # Leptos admin встроен, SSR in-process
embed_storefront = true  # Leptos storefront встроен, SSR in-process
```
```
Один бинарник. Ни одного HTTP-вызова между слоями платформы.
Браузер ↔ Axum ↔ #[server] fn ↔ DB — всё в одном процессе.
```

**Сервер + admin вместе, storefront отдельно:**
```toml
[build.server]
embed_admin = true
embed_storefront = false

[[build.storefront]]
stack = "leptos"  # или "next"
```
```
Admin: in-process.
Storefront: отдельный бинарник, ходит на /api/fn/* или /api/graphql.
```

**Чистый headless:**
```toml
[build.server]
embed_admin = false
embed_storefront = false
```
```
Сервер: только API (GraphQL + /api/fn/*).
Admin и storefront: отдельные процессы, любой стек.
```

**Мультисайт:**
```toml
[build.server]
embed_admin = true

[[build.storefront]]
id = "site-eu"
stack = "leptos"

[[build.storefront]]
id = "site-us"
stack = "next"
```

## Consequences

### Позитивные

- **Настоящий монолит**: один бинарник, нулевой HTTP между слоями. Деплой
  буквально как WordPress — скопировал бинарник, настроил БД, запустил.
- **Настоящий headless**: GraphQL как единственный внешний контракт, Leptos
  при раздельном деплое ходит на `/api/fn/*`.
- **Типобезопасность сквозная**: `#[server]`-функции компилируются вместе с
  клиентским кодом — несоответствие типов не скомпилируется.
- **Удаление GraphQL как зависимости из Leptos-приложений**: меньше кода,
  нет `leptos-graphql` crate в admin и storefront.
- **Производительность в монолите**: убирается serialization/deserialization
  GraphQL, resolver-слой, TCP round-trip.

### Негативные

- **Переписывание слоя данных в admin и storefront**: текущий код с
  `request_with_persisted(USERS_QUERY, ...)` заменяется на `#[server]`-функции.
  Это значительный объём работы.
- **SSR + hydrate сложнее чем чистый CSR**: нужно следить чтобы код корректно
  работал в обоих контекстах (сервер и браузер), избегать browser-only API
  в SSR-ветке.
- **`#[server]`-функции — не GraphQL**: для headless-клиентов (Next.js и др.)
  они недоступны напрямую, там по-прежнему GraphQL.

### Follow-up

1. Перевести `apps/admin` с `csr` на `ssr + hydrate`, убрать `leptos-graphql`,
   написать `#[server]`-функции для всех операций.
2. Перевести `apps/storefront` на `#[server]`-функции, убрать GraphQL HTTP-вызовы.
3. Убрать `leptos-graphql` crate из зависимостей admin и storefront.
4. Обновить `apps/server`: `#[server]`-эндпоинты регистрируются через
   `leptos_axum::handle_server_fns()` рядом с GraphQL-роутом.
5. Обновить документацию по деплою: зафиксировать оба крайних случая
   (монолит и headless) и все промежуточные конфигурации.
