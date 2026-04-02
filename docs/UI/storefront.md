# Витрина (Leptos SSR + Next.js)

В RusToK витрина существует в двух вариантах:

- `apps/storefront` — Rust-first Leptos SSR host.
- `apps/next-frontend` — Next.js storefront.

Обе реализации должны держать один и тот же backend contract, но Leptos storefront остаётся эталоном для SSR-host логики внутри Rust runtime.

## Leptos storefront host contract

- Host рендерит shell и generic module pages по маршруту `/modules/{route_segment}`.
- Module-owned storefront packages читают route state через `UiRouteContext`.
- Enabled modules резолвятся отдельно и фильтруют registry до SSR.
- Для async module surfaces используется in-order HTML streaming.

## Data-layer rule для Leptos storefront

- Для `apps/storefront` и module-owned storefront packages путь по умолчанию: `UI -> local api -> #[server] -> service layer`.
- В SSR-монолите это даёт in-process вызов; при hydrate/client navigation и в standalone деплое используется `POST /api/fn/*`.
- GraphQL (`/api/graphql`) остаётся обязательным параллельным контрактом и fallback-веткой; его нельзя удалять при добавлении native path.
- Новый module-owned Leptos storefront UI не должен проектироваться как GraphQL-only, если может жить через `#[server]`.

## Canonical route resolution

- Canonical URL policy и alias storage живут в `rustok-content`, а не в storefront.
- `apps/storefront` не читает БД напрямую и не знает о typed canonical tables.
- Перед SSR module-route storefront делает GraphQL preflight `resolveCanonicalRoute` в `apps/server`.
- Если запрошенный URL оказался alias-ом, storefront отдаёт HTTP redirect на canonical URL до рендера страницы.
- Query-параметр `lang` исключается из canonical lookup key, потому что locale передаётся отдельно.
- При redirect storefront сохраняет исходный `lang` в canonical target URL, чтобы SSR не терял выбранную локаль.

## Next.js parity

- Next.js storefront должен следовать той же canonical routing policy.
- Redirect resolution не должен дублировать storage-логику во frontend-приложении; источником истины остаётся backend/server surface.

## Локальный запуск

```bash
# Leptos SSR
cargo run -p rustok-storefront

# Next.js storefront
cd apps/next-frontend
npm install
npm run dev
```

Leptos SSR сервер слушает `http://localhost:3100`, Next.js storefront — `http://localhost:3000`.
