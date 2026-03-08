# RusToK Leptos Storefront

`apps/storefront` вЂ” Leptos SSR РІРёС‚СЂРёРЅР° RusToK (Rust-first РІР°СЂРёР°РЅС‚ storefront).

## Р РѕР»СЊ РІ РїР»Р°С‚С„РѕСЂРјРµ

- SSR storefront РґР»СЏ РІРёС‚СЂРёРЅРЅС‹С… СЃС†РµРЅР°СЂРёРµРІ;
- РїР°СЂР°Р»Р»РµР»СЊРЅР°СЏ СЂРµР°Р»РёР·Р°С†РёСЏ Рє `apps/next-frontend` РґР»СЏ С‚РµС…РЅРѕР»РѕРіРёС‡РµСЃРєРѕРіРѕ РїР°СЂРёС‚РµС‚Р°;
- РїСЂРѕРІРµСЂРєР° Rust UI/SSR РїР°Р№РїР»Р°Р№РЅР° РІ РµРґРёРЅРѕР№ РїР»Р°С‚С„РѕСЂРјРµ.

## РђСЂС…РёС‚РµРєС‚СѓСЂРЅС‹Р№ РєРѕРЅС‚СѓСЂ

- entrypoint: `src/main.rs`
- РјРѕРґСѓР»СЊРЅС‹Рµ СЂР°СЃС€РёСЂРµРЅРёСЏ РІРёС‚СЂРёРЅС‹: `src/modules/*` (registry/slots)
- СЃС‚РёР»Рё: Tailwind + СЃС‚Р°С‚РёС‡РµСЃРєР°СЏ СЃР±РѕСЂРєР° `static/app.css`
- FSD РЅРµ РїСЂРёРјРµРЅСЏРµС‚СЃСЏ (single-file storefront), РЅРѕ РјРѕРґСѓР»СЊРЅР°СЏ СЂР°СЃС€РёСЂСЏРµРјРѕСЃС‚СЊ С‡РµСЂРµР· slot-СЃРёСЃС‚РµРјСѓ

## РЎРѕРіР»Р°С€РµРЅРёСЏ РѕР± РёРјРµРЅРѕРІР°РЅРёРё (Naming Conventions)

Р’ РїСЂРѕРµРєС‚Рµ РїСЂРёРЅСЏС‚С‹ СЃР»РµРґСѓСЋС‰РёРµ СЃРѕРіР»Р°С€РµРЅРёСЏ РґР»СЏ РѕР±РµСЃРїРµС‡РµРЅРёСЏ С‡РёСЃС‚РѕС‚С‹ РєРѕРґР° Рё СЃРѕР±Р»СЋРґРµРЅРёСЏ СЃС‚Р°РЅРґР°СЂС‚РѕРІ Rust:

- **РљРѕРјРїРѕРЅРµРЅС‚С‹ (С„СѓРЅРєС†РёРё)**: Р’СЃРµ Leptos-РєРѕРјРїРѕРЅРµРЅС‚С‹ РёРјРµРЅСѓСЋС‚СЃСЏ РІ `snake_case` (РЅР°РїСЂРёРјРµСЂ, `storefront_shell`, `product_card`). РСЃРїРѕР»СЊР·РѕРІР°РЅРёРµ `PascalCase` РґР»СЏ С„СѓРЅРєС†РёР№-РєРѕРјРїРѕРЅРµРЅС‚РѕРІ РЅРµ СЂРµРєРѕРјРµРЅРґСѓРµС‚СЃСЏ.
- **Shared UI**: РћР±С‰РёРµ UI-РєРѕРјРїРѕРЅРµРЅС‚С‹ РІ `shared/ui/` (РµСЃР»Рё РїРѕСЏРІСЏС‚СЃСЏ) РёРјРµСЋС‚ РїСЂРµС„РёРєСЃ `ui_` (РЅР°РїСЂРёРјРµСЂ, `ui_button`).
- **РњРѕРґСѓР»Рё**: РљРѕРјРїРѕРЅРµРЅС‚С‹ РІ `src/modules/` С‚Р°РєР¶Рµ РёСЃРїРѕР»СЊР·СѓСЋС‚ `snake_case`.

## Р‘РёР±Р»РёРѕС‚РµРєРё

### РЇРґСЂРѕ

- `leptos`, `leptos_router` вЂ” UI Рё SSR-СЂРµРЅРґРµСЂРёРЅРі (С‡РёСЃС‚С‹Р№ SSR, Р±РµР· hydration)
- `axum`, `tokio` вЂ” HTTP СЃРµСЂРІРµСЂ

### i18n

- `leptos_i18n` 0.6 (feature `ssr`) вЂ” compile-time РјРЅРѕРіРѕСЏР·С‹С‡РЅРѕСЃС‚СЊ С‡РµСЂРµР· `t_string!()` РјР°РєСЂРѕСЃ;
- `leptos_i18n_build` вЂ” РєРѕРґРѕРіРµРЅРµСЂР°С†РёСЏ i18n-РјРѕРґСѓР»СЏ РёР· `locales/*.json` С‡РµСЂРµР· `build.rs`;
- С„Р°Р№Р»С‹ Р»РѕРєР°Р»РµР№: `locales/en.json`, `locales/ru.json`;
- РІС‹Р±РѕСЂ СЏР·С‹РєР°: query-РїР°СЂР°РјРµС‚СЂ `?lang=ru`.

### Р’РЅСѓС‚СЂРµРЅРЅРёРµ crates

- `leptos-auth`, `leptos-graphql` вЂ” auth/GraphQL РєРѕРЅС‚СЂР°РєС‚С‹
- `leptos-table`, `leptos-hook-form`, `leptos-zod`, `leptos-zustand` вЂ” С„РѕСЂРјС‹/СЃРѕСЃС‚РѕСЏРЅРёРµ
- `leptos-shadcn-pagination`, `leptos-next-metadata`, `leptos_query` вЂ” UI-СѓС‚РёР»РёС‚С‹

## Р’Р·Р°РёРјРѕРґРµР№СЃС‚РІРёРµ

- `apps/server` (API)
- РґРѕРјРµРЅРЅС‹Рµ `crates/rustok-*` С‡РµСЂРµР· backend
- РѕР±С‰РёР№ UI-РєРѕРЅС‚СѓСЂ СЃ `apps/admin` / `apps/next-frontend`

## Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ

- РџР»Р°С‚С„РѕСЂРјРµРЅРЅС‹Р№ РєРѕРЅС‚РµРєСЃС‚: `docs/UI/storefront.md`
- РћР±С‰Р°СЏ РєР°СЂС‚Р°: `docs/index.md`

## Module-aware storefront notes

- Storefront SSR now fetches `enabledModules` before rendering and provides them through `EnabledModulesProvider`.
- `src/modules/registry.rs` filters slot components by `module_slug` so optional widgets disappear when a tenant disables the owning module.
- The standalone Leptos storefront keeps the same module contract as the Next storefront: core widgets use `module_slug = None`, optional module widgets must declare their owning slug.
