# Р”РѕРєСѓРјРµРЅС‚Р°С†РёСЏ РїРѕ РјРѕРґСѓР»СЏРј RusToK

Р­С‚РѕС‚ РґРѕРєСѓРјРµРЅС‚ С„РёРєСЃРёСЂСѓРµС‚ Р°РєС‚СѓР°Р»СЊРЅСѓСЋ РјРѕРґСѓР»СЊРЅСѓСЋ РјРѕРґРµР»СЊ RusToK Р±РµР· СЃРјРµС€РµРЅРёСЏ
Р°СЂС…РёС‚РµРєС‚СѓСЂРЅС‹С… С‚РµСЂРјРёРЅРѕРІ Рё С‚РµС…РЅРёС‡РµСЃРєРѕР№ СѓРїР°РєРѕРІРєРё.

## Р‘Р°Р·РѕРІР°СЏ РјРѕРґРµР»СЊ

Р”Р»СЏ platform modules СЃСѓС‰РµСЃС‚РІСѓРµС‚ С‚РѕР»СЊРєРѕ РґРІР° СЃС‚Р°С‚СѓСЃР°:

- `Core`
- `Optional`

РСЃС‚РѕС‡РЅРёРє РёСЃС‚РёРЅС‹ РїРѕ РЅРёРј: `modules.toml`.

РџСЂРё СЌС‚РѕРј:

- `crate` вЂ” СЌС‚Рѕ С‚РµСЂРјРёРЅ Cargo Рё СЃРїРѕСЃРѕР± СѓРїР°РєРѕРІРєРё;
- РЅРµ РєР°Р¶РґС‹Р№ crate РІ `crates/` СЏРІР»СЏРµС‚СЃСЏ platform module;
- СЂСЏРґРѕРј СЃ module-crates Р»РµР¶Р°С‚ shared libraries Рё infrastructure/support crates.

## Р“РґРµ СЃРјРѕС‚СЂРµС‚СЊ РІ РєРѕРґРµ

- СЃРѕСЃС‚Р°РІ platform modules: `modules.toml`
- runtime registry: `apps/server/src/modules/mod.rs`
- СЃРІРµСЂРєР° manifest Рё registry: `apps/server/src/modules/manifest.rs`
- Р±Р°Р·РѕРІС‹Рµ РјРѕРґСѓР»СЊРЅС‹Рµ РєРѕРЅС‚СЂР°РєС‚С‹: `crates/rustok-core/src/module.rs`
- РєР°С‚РµРіРѕСЂРёРё `Core` / `Optional`: `crates/rustok-core/src/registry.rs`

## РўРµРєСѓС‰РёР№ СЃРѕСЃС‚Р°РІ platform modules

### Core

| Slug | Crate |
|---|---|
| `auth` | `rustok-auth` |
| `cache` | `rustok-cache` |
| `channel` | `rustok-channel` |
| `email` | `rustok-email` |
| `index` | `rustok-index` |
| `search` | `rustok-search` |
| `outbox` | `rustok-outbox` |
| `tenant` | `rustok-tenant` |
| `rbac` | `rustok-rbac` |

### Optional

| Slug | Crate | Depends on |
|---|---|---|
| `content` | `rustok-content` | вЂ” |
| `cart` | `rustok-cart` | вЂ” |
| `customer` | `rustok-customer` | вЂ” |
| `product` | `rustok-product` | `taxonomy` |
| `profiles` | `rustok-profiles` | `taxonomy` |
| `region` | `rustok-region` | вЂ” |
| `pricing` | `rustok-pricing` | `product` |
| `inventory` | `rustok-inventory` | `product` |
| `order` | `rustok-order` | вЂ” |
| `payment` | `rustok-payment` | вЂ” |
| `fulfillment` | `rustok-fulfillment` | вЂ” |
| `commerce` | `rustok-commerce` | `cart`, `customer`, `product`, `region`, `pricing`, `inventory`, `order`, `payment`, `fulfillment` |
| `blog` | `rustok-blog` | `content`, `comments`, `taxonomy` |
| `forum` | `rustok-forum` | `content`, `taxonomy` |
| `comments` | `rustok-comments` | вЂ” |
| `pages` | `rustok-pages` | `content` |
| `taxonomy` | `rustok-taxonomy` | `content` |
| `media` | `rustok-media` | вЂ” |
| `workflow` | `rustok-workflow` | вЂ” |

## Р’Р°Р¶РЅРѕРµ СѓС‚РѕС‡РЅРµРЅРёРµ РїРѕ wiring

`ModuleRegistry` вЂ” СЌС‚Рѕ runtime composition point, Р° РЅРµ РєР»Р°СЃСЃРёС„РёРєР°С‚РѕСЂ
Р°СЂС…РёС‚РµРєС‚СѓСЂРЅС‹С… СЂРѕР»РµР№.

РР· СЌС‚РѕРіРѕ СЃР»РµРґСѓСЋС‚ РґРІР° РїСЂР°РІРёР»Р°:

1. Р•СЃР»Рё РєРѕРјРїРѕРЅРµРЅС‚ РѕР±СЉСЏРІР»РµРЅ РєР°Рє platform module РІ `modules.toml`, РѕРЅ РѕР±СЏР·Р°РЅ Р±С‹С‚СЊ
   Р»РёР±Рѕ `Core`, Р»РёР±Рѕ `Optional`.
2. РўРµС…РЅРёС‡РµСЃРєРёР№ СЃРїРѕСЃРѕР± РїРѕРґРєР»СЋС‡РµРЅРёСЏ РјРѕРґСѓР»СЏ РјРѕР¶РµС‚ РѕС‚Р»РёС‡Р°С‚СЊСЃСЏ:
   - СЂРµРіРёСЃС‚СЂР°С†РёСЏ РІ `ModuleRegistry`
   - bootstrap/runtime wiring
   - generated host wiring

`rustok-outbox` вЂ” С…РѕСЂРѕС€РёР№ РїСЂРёРјРµСЂ. РћРЅ СЏРІР»СЏРµС‚СЃСЏ `Core` module Рё РѕРґРЅРѕРІСЂРµРјРµРЅРЅРѕ
РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ РЅР°РїСЂСЏРјСѓСЋ event runtime СЃР»РѕРµРј. Р­С‚Рѕ РЅРµ РґРµР»Р°РµС‚ РµРіРѕ РѕС‚РґРµР»СЊРЅС‹Рј
"С‚СЂРµС‚СЊРёРј С‚РёРїРѕРј".

## Р§С‚Рѕ Р»РµР¶РёС‚ СЂСЏРґРѕРј СЃ РјРѕРґСѓР»СЏРјРё

Р’ `crates/` С‚Р°РєР¶Рµ Р¶РёРІСѓС‚ РєРѕРјРїРѕРЅРµРЅС‚С‹, РєРѕС‚РѕСЂС‹Рµ РЅРµ РІС…РѕРґСЏС‚ РІ taxonomy
`Core/Optional`:

- shared libraries: `rustok-core`, `rustok-api`, `rustok-events`,
  `rustok-storage`, `rustok-test-utils`
- infra/capability crates: `rustok-iggy`, `rustok-iggy-connector`,
  `rustok-telemetry`, `rustok-mcp`, `alloy`, `flex`

РРјРµРЅРЅРѕ РїРѕСЌС‚РѕРјСѓ РЅРµР»СЊР·СЏ Р°РІС‚РѕРјР°С‚РёС‡РµСЃРєРё РїСЂРёСЂР°РІРЅРёРІР°С‚СЊ "Р»СЋР±РѕР№ crate РІ `crates/`" Рє
platform module.

## UI composition policy

Р•СЃР»Рё Сѓ РјРѕРґСѓР»СЏ РµСЃС‚СЊ UI, РѕРЅ РґРѕР»Р¶РµРЅ РїРѕСЃС‚Р°РІР»СЏС‚СЊСЃСЏ СЃР°РјРёРј РјРѕРґСѓР»РµРј:

- Leptos: С‡РµСЂРµР· sub-crates `admin/` Рё `storefront/`
- Next.js: С‡РµСЂРµР· РїР°РєРµС‚С‹ РІ `apps/next-admin/packages/*` Рё
  `apps/next-frontend/packages/*`

Host-РїСЂРёР»РѕР¶РµРЅРёСЏ РґРѕР»Р¶РЅС‹ РјРѕРЅС‚РёСЂРѕРІР°С‚СЊ СЌС‚Рё РїРѕРІРµСЂС…РЅРѕСЃС‚Рё С‡РµСЂРµР· manifest-driven
wiring, Р° РЅРµ С‡РµСЂРµР· Р¶С‘СЃС‚РєРѕ РїСЂРёС€РёС‚С‹Рµ module-specific РІРµС‚РєРё.

## РЎРІСЏР·Р°РЅРЅС‹Рµ РґРѕРєСѓРјРµРЅС‚С‹

- [Р РµРµСЃС‚СЂ РјРѕРґСѓР»РµР№ Рё РїСЂРёР»РѕР¶РµРЅРёР№](./registry.md)
- [Р РµРµСЃС‚СЂ crate-РѕРІ RusToK](./crates-registry.md)
- [РњР°РЅРёС„РµСЃС‚ РјРѕРґСѓР»РµР№](./manifest.md)
- [РђСЂС…РёС‚РµРєС‚СѓСЂР° РјРѕРґСѓР»РµР№](../architecture/modules.md)

