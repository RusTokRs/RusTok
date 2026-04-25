# Admin ↔ Server: quickstart

Этот документ фиксирует минимальный runtime contract между UI host-приложениями и `apps/server`. Он не заменяет полноценные deployment runbook и не дублирует инструкции по конкретным окружениям.

## Базовая схема

Рекомендуемый базовый путь для UI hosts:

- browser открывает host-приложение;
- UI обращается к `apps/server`;
- backend публикует `/api/graphql`, `/api/fn/*`, `/api/auth/*` и связанные runtime surfaces;
- reverse proxy или host runtime скрывает лишнюю cross-origin сложность там, где это возможно.

## Профили Leptos admin

`apps/admin` разделяет transport по runtime profile. Продуктовый target для Leptos admin — SSR-first monolith/hydrate, а standalone CSR нужен для local debug и compatibility проверки module-owned UI packages:

- `csr`: standalone Trunk/WASM host. Critical paths идут в `apps/server` напрямую через `/api/graphql`,
  `/api/auth/*` и REST. `/api/fn/*` не обязателен для базового shell/debug и не должен быть единственным transport.
- `hydrate`: browser half для SSR/monolith. UI может вызывать `#[server]`, потому что backend origin
  должен обслуживать `/api/fn/*`.
- `ssr`: server half или monolith. `#[server]` доступен как native transport и может быть preferred path
  для server-side surfaces.

Правило: `#[server]` не заменяет GraphQL/REST. Если surface нужна в standalone `csr`, у неё должен быть
рабочий GraphQL/REST path или явно документированный fallback.

## Предпочтительный local/dev path

Для локальной отладки предпочтителен same-origin или proxy-aware режим, где UI и backend выглядят как один origin для браузера. Это уменьшает CORS-ошибки, упрощает auth/session flows и делает transport contract предсказуемым.

Минимум, который должен быть доступен:

- UI host;
- `apps/server`;
- рабочий auth path;
- рабочий GraphQL path;
- если host Leptos в `ssr`/`hydrate` profile — рабочий `#[server]` path;
- если host Leptos в standalone `csr` debug profile — рабочий GraphQL/REST fallback без обязательного `/api/fn/*`.

## Минимальный runtime contract

UI host должен уметь достучаться до:

- `/api/graphql`
- `/api/auth/*`
- `/api/fn/*` для Leptos `ssr`/`hydrate` hosts
- health/runtime surfaces при operator-level диагностике

Если UI и backend находятся на разных origins, backend обязан явно поддерживать требуемый CORS и auth contract. Если это не нужно, same-origin схема остаётся предпочтительной.

## Что проверить после подключения

Минимальный smoke:

1. Открывается login surface host-приложения.
2. Работает вход и загрузка текущего пользователя/сессии.
3. Успешны запросы к `/api/auth/*`.
4. Успешен запрос к `/api/graphql`.
5. Для Leptos `ssr`/`hydrate` hosts, если затронут native path, успешны вызовы `/api/fn/*`.
6. Для Leptos standalone `csr`, если затронут module-owned UI package, тот же экран работает через GraphQL/REST fallback.

Если эти шаги проходят, host ↔ server contract собран корректно.

## Route-selection contract для admin hosts

Для module-owned admin surfaces runtime contract включает не только transport, но и routing:

1. selection state хранится в URL;
2. module-owned admin UI читает его через host route context;
3. valid user-driven select/open пишет canonical typed `snake_case` key обратно в query;
4. reset/delete/archive/close очищают соответствующий key;
5. invalid или удалённый entity id даёт empty state и не оставляет stale detail/form state.

Для Leptos host этот contract проходит через `UiRouteContext` + host-provided policy для
`leptos-ui-routing`. Для `apps/next-admin` действует тот же schema-level contract через локальные
Next helpers. Legacy keys вроде `id`, `pageId`, `topicId` не поддерживаются.

## Диагностика

### `401 Unauthorized`

Проверить:

- auth token или session transport;
- tenant/channel headers, если они обязательны для конкретного сценария;
- не сломан ли backend-side auth/runtime contract.

### CORS ошибки

Обычно это означает, что UI и backend работают cross-origin без нужной backend-конфигурации. Предпочтительный фикс — same-origin/proxy path, а не рост ad hoc исключений.

### `404` на `/api/graphql` или `/api/fn/*`

Проверить:

- что reverse proxy действительно пробрасывает `/api/*`;
- что `apps/server` поднят на ожидаемом порту;
- что выбранный UI host использует корректный transport contract для текущего runtime mode.

## Локальный debug-stack без Docker

Для локальной отладки без `docker compose` минимальный стек поднимается как отдельные процессы:

```powershell
# 1. apps/server
$env:RUSTOK_MODULES_MANIFEST = (Resolve-Path .\modules.local.toml)
target\debug\rustok-server.exe start --no-banner --binding localhost --port 5150

# 2. apps/next-admin
cd apps\next-admin
npm.cmd run dev -- --hostname localhost --port 3000 --webpack

# 3. apps/admin
cd ..\admin
trunk serve --address ::1 --port 3001
```

Для локального debug без Docker сервер должен читать `modules.local.toml`, где embedded admin/storefront выключены.
Корневой `modules.toml` описывает monolith/release composition и требует `embed-admin`/`embed-storefront`.
В текущем Windows debug-окружении сборка `apps/admin` как SSR embedded artifact падает по памяти (`rustc-LLVM ERROR: out of memory`),
поэтому внешний стек `apps/server -> apps/next-admin -> apps/admin` запускается через `modules.local.toml`.

Tenant contract для standalone admin hosts slug-based: UI отправляет `X-Tenant-Slug`, backend в header-mode обязан принимать этот
header как публичный admin contract. `X-Tenant-ID` остаётся допустимым внутренним/legacy header, но не должен требоваться от UI host.

Решение по binding: canonical URL остаётся `http://localhost:5150`. На этой Windows-машине `127.0.0.1`
даёт зависание HTTP-ответов даже для простого Node server, а `localhost` уходит в `::1` и работает стабильно.
Поэтому локальный debug-stack должен использовать `localhost`/`::1`, а не `127.0.0.1`.

Решение по Next admin: для Next.js 16 локальный `next dev` на Turbopack зависал на компиляции
`/auth/sign-in`, поэтому debug-команда использует `--webpack`. Это startup/debug choice, не изменение
публичного API.

Решение по Leptos admin: standalone `csr` профиль нужен для debug и headless parity, но продуктовый target
остаётся SSR-first/hydrate. `#[server]` остаётся preferred internal path в SSR/monolith, GraphQL/REST остаётся
обязательным fallback для headless/CSR. `trunk serve` должен собирать binary artifact `rustok-admin`, потому что
library artifact `rustok_admin` не запускает `main()` и не монтирует shell.

Визуальный контракт: Leptos admin и Next admin не должны расходиться как независимые продукты. Host-level auth shell,
navigation, route-selection UX и module-owned surface containers должны идти через общий admin UI contract/tokens.
Next admin может оставаться React/Next host для Next-пакетов, но Leptos admin является canonical operator surface для
монолитного/SSR пути; расхождения фиксируются как parity debt, а не как допустимый fork дизайна.

Для standalone `trunk serve` CSS является частью startup contract. `apps/admin/input.css` использует Tailwind v4
`@import "tailwindcss"` + `@source`, а `tailwind.config.js` обязан сканировать не только `apps/admin/src`, но и
module-owned admin UI packages в `crates/**/admin/src/**/*.rs`, а также shared Leptos UI crates. Иначе host shell может
загрузиться, но module-owned страницы останутся без spacing/layout utilities и визуально разойдутся с Next admin.
Trunk post-build hook `scripts\tailwind-build.cmd` кладёт `output.css` в staging/dist; отсутствие `dist/output.css`
считается startup blocker для Leptos admin debug.

## Scope этого quickstart

Этот документ намеренно не хранит:

- длинные инструкции по Docker Compose, VPS, Kubernetes или PaaS;
- install-скрипты и bootstrap-runbook;
- подробные production rollout steps.

Такие инструкции должны жить в отдельных deployment/runbook документах, а здесь остаётся только живой UI ↔ server contract.

## Связанные документы

- [UI index](./README.md)
- [GraphQL и Leptos server functions](./graphql-architecture.md)
- [Документация `apps/admin`](../../apps/admin/docs/README.md)
- [Документация `apps/server`](../../apps/server/docs/README.md)
- [ADR: SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
- [Health и runtime guardrails](../../apps/server/docs/health.md)
- [Карта документации](../index.md)
