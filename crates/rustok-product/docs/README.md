# Документация `rustok-product`

`rustok-product` — дефолтный каталоговый подмодуль семейства `ecommerce`.

## Назначение

- каталог товаров;
- варианты, опции, переводы и публикация;
- taxonomy-backed product tags через shared `rustok-taxonomy` и product-owned relation `product_tags`;
- product-owned migrations;
- `ProductModule`, `CatalogService`, module-owned admin UI пакет `rustok-product/admin` и module-owned storefront UI пакет `rustok-product/storefront`.

## Зона ответственности

- GraphQL transport публикуется через `rustok-commerce`, а module-owned Leptos admin использует параллельные native `#[server]` functions как основной внутренний путь.
- storefront read-side для published catalog уже живёт в `rustok-product/storefront` и использует native Leptos server functions поверх `CatalogService`, сохраняя GraphQL storefront contract как fallback.
- product CRUD в admin UI уже вынесен из `rustok-commerce-admin`
  в module-owned route `product`, но transport-контракт для этих форм по-прежнему
  приходит через umbrella `rustok-commerce` GraphQL surface;
- generic GraphQL roots `product` / `storefrontProduct`, на которые пока опираются
  module-owned product UI packages, считаются catalog-authoritative surface:
  `variants.prices` в них остаётся compatibility snapshot без explicit
  currency/region/price-list/channel resolution и не должен трактоваться как
  pricing source of truth рядом с `adminPricingProduct` / `storefrontPricingProduct`;
- module-owned `rustok-product/admin` и `rustok-product/storefront` теперь тоже
  синхронизированы с этим split: UI больше не показывает generic catalog
  `variants.prices` как resolved price, а держит отдельный pricing-module preview
  hook для `adminPricingProduct` / `storefrontPricingProduct`; admin list/status/filter,
  shipping-profile, pricing-preview и pricing deep-link helpers живут в
  framework-agnostic `admin/src/core.rs` (включая `SelectedProductSummaryViewModel`),
  admin GraphQL операции проходят через module-owned facade `admin/src/transport.rs`,
  а Leptos render/effect adapter изолирован в `admin/src/ui/leptos.rs`;
- storefront FFA slices вынесли route/query normalization, typed fetch request shape,
  shell copy, selected-product view-model composition, selected-card labels/empty
  state, catalog rail presentation, pricing/seller labels, pricing-context
  sanitization/defaulting и pricing deep-link state в `storefront/src/core.rs`;
  native/GraphQL storefront fetch paths оформлены как `storefront/src/transport/`
  adapters, а Leptos `ProductView`/`SelectedProductCard`/`CatalogRail` живут в
  `storefront/src/ui/leptos.rs` как тонкий host-context/render слой поверх
  подготовленного core-состояния;
- Общие DTO, entities и error surface приходят из `rustok-commerce-foundation`.
- canonical vocabulary и attach semantics для product tags живут в
  `rustok-taxonomy` + `product_tags`, а public contract использует first-class
  поле `tags` вместо legacy `metadata.tags`.
- shipping profile для товара и варианта теперь имеет first-class typed surface в
  product DTO (`shipping_profile_slug`) и typed persistence в
  `products.shipping_profile_slug` / `product_variants.shipping_profile_slug`; metadata-backed
  `shipping_profile.slug` остаётся только backward-compatible формой нормализации для старых
  read/write-path consumer'ов.
- multivendor foundation теперь тоже начинается на product boundary: create/update/read contract
  включает nullable `seller_id`, который считается canonical seller identity key для downstream
  cart/order/fulfillment orchestration; merchandising/display поля вроде `vendor` не должны
  использоваться как seller identity.
- effective shipping profile для deliverability теперь разрешается как
  `variant.shipping_profile_slug -> product.shipping_profile_slug -> default`, а omission
  first-class поля на write-path не должен затирать уже существующую typed binding/compatibility
  normalization.
- transport-level validation для `shipping_profile_slug` теперь живёт в фасаде
  `rustok-commerce` и проверяет ссылку против active shipping profiles из typed
  registry `shipping_profiles`, чтобы product write-path не принимал произвольные slug'и.

## Нативные атрибуты каталога

- `product_attributes` является единым справочником ecommerce-атрибутов.
- `catalog_categories` хранит structural, collection и virtual категории; `products.primary_category_id` определяет product form только через structural category.
- `product_attribute_schemas` являются опциональными reusable templates, а category bindings/groups дают inheritance, clone snapshot, custom и local override сценарии.
- `product_categories` хранит дополнительные навигационные/витринные привязки и не меняет форму товара.
- Значения живут в typed product/variant attribute value tables; localized labels и text-like values вынесены в translation tables.
- Product-level значения читаются и изменяются через owner-owned typed read/patch contract: omitted атрибут не меняется, `clear` удаляет значение, пустой multiselect очищает значение, options и effective schema проверяются до транзакционной записи, а detached values сохраняются и возвращаются отдельным маркером.
- Detached values отображаются в product admin отдельным review-блоком и очищаются только через owner-owned `clear_detached_product_attribute_values`; сервис проверяет, что каждый удаляемый attribute действительно находится вне текущей effective schema, native `#[server]` остаётся основным путём, GraphQL поддерживается параллельно.
- Publish validation выполняется в owner-owned `ProductCatalogSchemaService`: required effective attributes должны быть заполнены до перехода товара в `Active`, localized text-like values требуют явную непустую translation row, option attributes требуют сохранённые option relations, а create-with-publish отклоняется для категорий с required typed attributes.
- Effective form загружает локализованные options одним ограниченным запросом по effective attribute ids; schema/category groups возвращают локализованный `group_label` по host locale, binding использует стабильный `group_code`, а product admin группирует поля по label/code, отображает typed controls и отправляет только dirty patches после сохранения товара и его primary category.
- `rustok-index` при индексации товара материализует tenant/locale-scoped строки категорий и нормализованные facet/search/sort значения. Multiselect раскладывается по одной строке на option, localized labels берутся только из явной строки запрошенной locale, а effective attribute ids вычисляются read-only resolver-ом `rustok-product`, поэтому detached values в read model не попадают.
- `rustok-search` читает эти проекции напрямую для category/virtual-category filters, channel-scoped attribute facets и attribute sorting. Write model остаётся у `rustok-product`; search не пересобирает schema inheritance и не читает detached values как effective.
- Visibility flags вычисляются с приоритетом `global attribute defaults < schema/category overrides < channel settings`. Overrides являются tri-state: отсутствующее поле наследует предыдущее значение, явный `false` отключает поведение. Resolver сохраняет overrides через live inheritance и clone snapshots, а indexer создаёт отдельные rows для каждого active channel с effective facet/search/sort, comparison, storefront и admin-grid flags. Если у tenant нет active channels, создаётся один global row с `channel_id = null`.
- Virtual category использует bounded V1 rule contract. Все заполненные предикаты объединяются через AND: `statuses`, `primary_category_subtree_id`, пересекающийся диапазон `price_min`/`price_max`, `in_stock` и список `attributes` с операторами `eq`/`range`. Attribute rules работают только со стабильными option codes и locale-neutral product values; localized и variant-only attributes отклоняются на write-side.

```json
{
  "version": 1,
  "statuses": ["active"],
  "primary_category_subtree_id": "00000000-0000-0000-0000-000000000000",
  "price_min": 1000,
  "price_max": 5000,
  "in_stock": true,
  "attributes": [
    { "code": "brand", "operator": "eq", "value": "rustok" },
    { "code": "weight", "operator": "range", "min": "1.0", "max": "2.5" }
  ]
}
```

При переиндексации товара `rustok-index` сначала полностью заменяет его строки в `virtual_category_product_assignments`, затем строит локализованные category projections. Некорректные старые rule payloads не останавливают reindex: категория пропускается с warning; новые некорректные payloads сервис создания не принимает.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- transport, GraphQL и UI-поверхности публикуются через `rustok-commerce`, пока для домена не зафиксирован отдельный module-owned surface;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce` и соседними split-модулями.

## SEO ownership

- `rustok-product/admin` уже держит owner-side product SEO panel через
  `rustok-seo-admin-support`, не вынося product metadata editing в `rustok-seo-admin`.

## Проверка

- cargo xtask module validate product
- cargo xtask module test product
- targeted commerce tests для product-домена при изменении runtime wiring
## Связанные документы

- [README crate](../README.md)
- [README admin UI](../admin/README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
