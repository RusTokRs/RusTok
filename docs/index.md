# RusTok: карта документации

Этот файл является канонической точкой входа в документацию репозитория. С него нужно начинать работу по правилам [AGENTS.md](../AGENTS.md).

Документация в `docs/` описывает платформу целиком. Локальные документы приложений и crate-ов лежат в `apps/*/docs/`, `crates/*/docs/` и `README.md` рядом с кодом.

## Как пользоваться картой

1. Сначала откройте обзор платформы и нужный архитектурный раздел.
2. Для изменений в модульной системе переходите в `docs/modules/*`.
3. Для UI-срезов используйте `docs/UI/*` и локальные docs приложений.
4. Для периодической верификации и quality-gates используйте `docs/verification/*` и `docs/guides/*`.
5. Для остаточного и будущего scope по platform contracts сверяйтесь с профильными live docs в `docs/architecture/*`, `docs/UI/*` и `apps/*/docs/*`, не смешивая это с периодической верификацией.
6. Для изменений конкретного модуля сверяйтесь с `docs/modules/registry.md` и локальными docs соответствующего crate.

## Обязательные стартовые документы

- [Обзор платформы](./architecture/overview.md)
- [Архитектурные принципы](./architecture/principles.md)
- [API и surface-контракты](./architecture/api.md)
- [Маршрутизация](./architecture/routing.md)
- [Модульная архитектура](./architecture/modules.md)
- [Карта модулей и владельцев](./modules/registry.md)

## Модульная система

- [Обзор модульной платформы](./modules/overview.md)
- [План и текущее состояние модульной системы](./modules/module-system-plan.md)
- [Контракт `rustok-module.toml`](./modules/manifest.md) — включая capability-only ghost modules вроде `alloy` и `flex`
- [Реестр модулей и приложений](./modules/registry.md)
- [Реестр crate-ов модульной платформы](./modules/crates-registry.md)
- [Индекс документации по модулям](./modules/_index.md)
- [Шаблон документации модуля](./templates/module_contract.md)
- [Индекс UI-пакетов модулей](./modules/UI_PACKAGES_INDEX.md)
- [Быстрый старт по UI-пакетам](./modules/UI_PACKAGES_QUICKSTART.md)
- UI split ecommerce family уже начат: `rustok-product/admin` стал первым
  module-owned admin route, `rustok-fulfillment/admin` забрал shipping options,
  `rustok-order/admin` забрал order operations, `rustok-inventory/admin` забрал
  inventory visibility, `rustok-pricing/admin` забрал pricing visibility,
  `rustok-customer/admin` забрал customer operations, `rustok-region/admin`
  забрал region CRUD, storefront-side split уже идёт через `rustok-region/storefront`
  , `rustok-product/storefront`, `rustok-pricing/storefront` и `rustok-cart/storefront`,
  `rustok-commerce-admin` очищен до shipping-profile registry, а
  `rustok-commerce-storefront` уже сжат до aggregate checkout workspace с seller-aware delivery-group shipping selection, а admin `create fulfillment` уже валидирует typed items по order-line ownership и remaining quantity.
- Текущий `Phase 7` уже дошёл до explicit post-order recovery semantics: `fulfillment_items`
  держат `shipped_quantity` / `delivered_quantity`, audit trail в metadata fulfillment/item'ов
  остаётся language-agnostic и не дублирует свободный текст вроде `delivered_note`, а admin
  REST/GraphQL теперь уже умеют не только partial item-level `ship` / `deliver`, но и
  explicit `reopen` / `reship` для post-order delivery corrections.
- Cross-cutting трек `Marketplace Foundations` тоже уже начат: `seller_id` стал canonical
  multivendor key в product/cart/order/checkout/fulfillment contract, а `seller_scope`
  оставлен только как transitional compatibility field для legacy snapshot'ов.
- `Phase 8` тоже уже начат с pricing foundation: `rustok-pricing` получил typed
  `currency + region + quantity` resolver поверх base-price rows, пока без активации
  полноценного price-list/promotions слоя; `rustok-pricing/storefront` и
  `rustok-pricing/admin` уже протянули этот effective-price context в
  module-owned UI surfaces через native-first `#[server]` transport с GraphQL fallback,
  а resolver теперь ещё и умеет explicit active `price_list_id` overlay поверх base prices,
  плюс уже начал channel-aware slice через host-provided `channel_id/channel_slug`,
  channel-scoped base rows и channel-filtered active price lists,
  причём pricing-owned read-side уже отдаёт active price lists как selector, а не
  заставляет UI жить на raw UUID-only вводе; кроме того, `rustok-pricing/admin`
  уже перестал быть чисто read-only route и получил module-owned base-price write path
  для variant prices, включая минимальный quantity-tier authoring по `min_quantity` /
  `max_quantity`, active price-list override authoring поверх base rows и selected
  active `price_list` rule/scope editing; targeted SSR tests уже покрывают этот admin
  transport path, а read-side contract теперь ещё и отдаёт typed `discount_percent`
  для sale rows/effective prices. Параллельно legacy `apply_discount`
  уже переведён на typed percentage-adjustment helper поверх canonical base-price row, а
  `rustok-pricing/admin` уже даёт operator-side preview/apply flow для такого adjustment path
  без смешивания его с quantity tiers; теперь этот flow уже умеет target'ить и выбранный
  active `price_list` override. Следующий promotion-ready слой тоже уже начат:
  active `price_list` может держать typed percentage rule, а resolver fallback'ится к
  base row через него, если explicit override row не задан; cart/order promotion snapshot тоже
  получил typed foundation через `cart_adjustments` / `order_adjustments`, `subtotal_amount`,
  `adjustment_total` и net `total_amount` без хранения localized display labels в ecommerce storage;
  pricing-authoritative GraphQL reads теперь живут в dedicated roots `adminPricingProduct` /
  `storefrontPricingProduct`, где explicit resolution modifiers требуют валидный
  трёхбуквенный `currencyCode`, malformed explicit `channel_id` отклоняется и такие inputs
  не игнорируются молча, а pricing UI wrappers валидируют этот contract ещё до
  fallback с native `#[server]` transport на GraphQL; параллельный admin GraphQL
  transport теперь уже умеет и `updateAdminPricingVariantPrice`,
  `previewAdminPricingVariantDiscount`, `applyAdminPricingVariantDiscount`,
  `updateAdminPricingPriceListRule`, `updateAdminPricingPriceListScope`,
  так что pricing write path больше не ограничен только server-function слоем;
  generic `product` / `storefrontProduct` с `variants.prices`
  остаются только catalog compatibility snapshot contract; последний широкий
  parity sweep также зафиксировал, что admin-side `price_list` rule/scope mutation
  paths режут future/expired lists без hidden fallback, clear rule metadata не
  оставляет stale selector state, pricing-focused GraphQL helper/read roots
  сохраняют rule/channel parity отдельно от остального storefront suite, а storefront
  cart/checkout GraphQL path теперь ещё и фиксирует typed adjustment snapshots и net
  payment amount поверх `cart_adjustments` / `order_adjustments`.
- `Phase 9` тоже уже начат: cart/order получили tax lines + `tax_total`/`tax_included` snapshot, checkout переносит tax lines в order, расчёт пока опирается на `region.tax_rate` / `tax_included` без provider seam.
- REST-side parity для этого snapshot layer теперь тоже закреплена: storefront/admin controller tests
  фиксируют typed adjustment snapshots, sanitized metadata и live shipping-selection semantics, а
  текущий verification baseline для `rustok-commerce` снова включает полный `cargo test -p rustok-commerce --lib`.
- storefront GraphQL add-to-cart теперь берёт `unit_price` через pricing resolver (currency + region + channel + quantity),
  а не из raw `price` row, чтобы cart pricing semantics у store GraphQL и REST совпадали.
- storefront cart quantity update теперь переоценивает `unit_price` через pricing resolver,
  чтобы quantity tiers применялись и при изменении количества.
- storefront cart context update теперь перепрайсит line items через pricing resolver,
  чтобы смена региона/контекста не оставляла stale `unit_price`.
- storefront payment-collection и complete-checkout paths перепрайсят line items перед созданием
  payment collection и перед `complete checkout`, чтобы price-list/quantity-tier изменения не
  оставляли stale `unit_price`.
- [Спец-план rich-text и визуального page builder](./modules/tiptap-page-builder-implementation-plan.md)

## UI и клиентские поверхности

- [Обзор UI](./UI/README.md)
- [GraphQL и Leptos server functions](./UI/graphql-architecture.md)
- [Контракт storefront](./UI/storefront.md)
- [Быстрый старт для Admin ↔ Server](./UI/admin-server-connection-quickstart.md)
- Route-selection contract для module-owned admin UI теперь жёстко закреплён как URL-owned:
  typed `snake_case` query keys живут в `rustok-api`, `leptos-ui-routing` остаётся только generic
  Leptos route/query plumbing, а `apps/admin` и `apps/next-admin` обязаны держать parity по этому
  контракту без legacy `id`/camelCase key compatibility и без hidden auto-select-first.
- Тот же `leptos-ui-routing` теперь используется и в module-owned Leptos storefront packages:
  storefront query/state reads не inventят второй helper layer поверх `UiRouteContext`, а `apps/storefront`
  и `apps/next-frontend` должны держать parity по тому же host-owned route/query contract.
- [Каталог Rust UI-компонентов](./UI/rust-ui-component-catalog.md)
- [Трек rich-text и визуального page builder](./modules/tiptap-page-builder-implementation-plan.md)
- [Архитектура i18n](./architecture/i18n.md) — request locale chain, shared locale normalization/validation contract, `verify:i18n:ui` + `verify:i18n:contract` gates, storefront locale-prefixed routes, outbound built-in auth email locale contract, manifest-level module UI bundle contract, временно без ecommerce locale alignment

## Архитектура и foundation

- [Диаграмма платформы](./architecture/diagram.md)
- [База данных](./architecture/database.md) — live DB/i18n storage contract: `base + translations + optional bodies`, `VARCHAR(32)` locale storage, `tenant_locales` policy layer, `flex` standalone schema translations, shared attached localized Flex values, live donor paths for `user`, `product`, `order`, and `topic`
- [Каналы](./architecture/channels.md)
- [DataLoader](./architecture/dataloader.md)
- [Контракт event flow](./architecture/event-flow-contract.md)
- [Matryoshka / модель композиции](./architecture/matryoshka.md)
- [Базовая производительность](./architecture/performance-baseline.md)

## Руководства и стандарты

- [Быстрый старт](./guides/quickstart.md)
- [Тестирование](./guides/testing.md)
- [Быстрый старт по observability](./guides/observability-quickstart.md)
- [Runtime guardrails](./guides/runtime-guardrails.md)
- [Валидация входных данных](./guides/input-validation.md)
- [Обработка ошибок](./guides/error-handling.md)
- [Аудит безопасности](./guides/security-audit.md)
- [Логирование](./standards/logging.md)
- [Ошибки](./standards/errors.md)
- [Безопасность](./standards/security.md)
- [Правила кодирования](./standards/coding.md)
- [Стандарт RT JSON v1](./standards/rt-json-v1.md)

## Проверка платформы

- [Инструмент workspace CLI `xtask`](../xtask/README.md)
- [Главный README по верификации](./verification/README.md)
- Flex multilingual contract теперь имеет отдельный repo-side guardrail:
  `node scripts/verify/verify-flex-multilingual-contract.mjs`
- [Сводный план верификации](./verification/PLATFORM_VERIFICATION_PLAN.md)
- [Верификация foundation-слоя](./verification/platform-foundation-verification-plan.md)
- [Верификация API-поверхностей](./verification/platform-api-surfaces-verification-plan.md)
- [Верификация frontend-поверхностей](./verification/platform-frontend-surfaces-verification-plan.md)
- [Верификация целостности ядра](./verification/platform-core-integrity-verification-plan.md)
- [Верификация качества и эксплуатации](./verification/platform-quality-operations-verification-plan.md)

## AI, исследования и шаблоны

- [Контекст для AI](./AI_CONTEXT.md)
- [Шаблон AI-сессии](./ai/SESSION_TEMPLATE.md)
- [Известные pitfalls](./ai/KNOWN_PITFALLS.md)
- [Индекс MCP reference](./references/mcp/README.md)
- [Сравнение архитектуры RusTok и Medusa](./research/medusa-vs-rustok-architecture.md)
- [Исследования и ADR-черновики](./research/ADR-xxxx-grpc-adoption.md)

## Документация приложений

- [Документация Server](../apps/server/docs/README.md)
  Server docs теперь фиксируют live `flex` standalone GraphQL + REST surfaces и их tenant-scoped RBAC contract.
- [Документация Admin](../apps/admin/docs/README.md)
- [Документация Storefront](../apps/storefront/docs/README.md)
- [Документация Next Admin](../apps/next-admin/docs/README.md)
- [Документация Next Frontend](../apps/next-frontend/docs/README.md)

## Документация crate-ов

- Для platform modules: `crates/rustok-*` согласно [реестру модулей и приложений](./modules/registry.md).
- Для foundation и shared libraries: `crates/rustok-core`, `crates/rustok-api`, `crates/rustok-events`, `crates/rustok-storage`, `crates/rustok-test-utils`, `crates/rustok-commerce-foundation`.
- Для infrastructure и capability crates: `crates/rustok-iggy`, `crates/rustok-iggy-connector`, `crates/rustok-telemetry`, `crates/rustok-mcp`, `crates/rustok-ai`, `crates/alloy`, `crates/flex`.
- Для UI-библиотек и host-shared UI support: `crates/leptos-*`, `crates/leptos-ui`.
- У каждого crate должен быть актуальный `README.md`, а при необходимости и `docs/`.

## Правила поддержки актуальности

- Центральные документы в `docs/` ведутся на русском языке.
- `README.md`, `AGENTS.md`, `CONTRIBUTING.md` и публичные контрактные документы ведутся на английском.
- Один файл — один язык.
- Не создавайте новый документ, если подходящий уже существует: расширяйте текущий.
- При изменении архитектуры, API, tenancy, routing, observability или модульной системы обновляйте и локальные docs компонента, и центральные документы в `docs/`.
- Любая новая схема проходит i18n-аудит: локализованные строки не храним в base-таблицах, display-поля живут только в `*_translations`. Module-owned UI пакеты не вводят package-local locale override и гидратят edit/detail формы по host-provided effective locale, а не по `first()` переводу сущности.
- Read-side/runtime locale resolution тоже живёт по общему contract: locale matching идёт через shared normalization (`requested -> tenant default -> first available`), а не через raw string equality вроде `ru` vs `ru-RU`.
- Любой новый module-owned admin UI обязан пройти route-selection audit: selection state хранится в URL,
  используются только typed `snake_case` query keys, локальный state остаётся производным от URL,
  а invalid/missing selection не должен silently fallback’иться на first-item auto-open.

## Architecture Decisions

- [Индекс ADR](../DECISIONS/README.md)
