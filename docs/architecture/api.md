# Архитектура API

Политика выбора API-стилей описана в [routing.md](./routing.md).

## Краткое резюме

RusToK использует гибридный подход:

- GraphQL для UI-клиентов;
- REST для интеграций, служебных сценариев и module-owned transport;
- OpenAPI для машиночитаемого REST-контракта;
- health/metrics endpoints для observability.

| API | Endpoint | Назначение |
| --- | --- | --- |
| GraphQL | `/api/graphql` | единая точка для admin/storefront UI |
| GraphQL WS | `/api/graphql/ws` | subscriptions transport |
| REST | `/api/v1/...` | внешние интеграции, webhooks, batch jobs |
| Commerce REST | `/store/...`, `/admin/...` | Medusa-style ecommerce transport |
| OpenAPI | `/api/openapi.json`, `/api/openapi.yaml` | спецификация REST API |
| Health | `/health`, `/health/live`, `/health/ready`, `/health/runtime`, `/health/modules` | runtime health/status |
| Metrics | `/metrics` | Prometheus-метрики |

## Ecommerce transport

Для ecommerce-направления live REST-контрактом считается Medusa-style surface:

- storefront routes под `/store/*`;
- admin routes под `/admin/*`.

Актуальные правила:

- legacy `/api/commerce/*` удалён из runtime router и OpenAPI;
- GraphQL остаётся отдельным transport-слоем, но должен использовать те же application services, что и REST;
- admin ecommerce surface сейчас включает product management, paginated `GET /admin/orders`, `GET /admin/orders/{id}`, explicit order lifecycle routes (`mark-paid`, `ship`, `deliver`, `cancel`) и admin list/detail/lifecycle routes для `payment-collections` и `fulfillments`;
- GraphQL commerce surface для admin уже включает parity-read queries `order`, `orders`, `paymentCollection`, `paymentCollections`, `fulfillment`, `fulfillments` и lifecycle mutations для order/payment-collection/fulfillment поверх тех же сервисов, что и REST;
- storefront GraphQL surface теперь, кроме catalog queries, включает customer-owned read queries `storefrontMe` и `storefrontOrder` поверх тех же `CustomerService`/`OrderService`, что и `/store/customers/me` и `/store/orders/{id}`;
- storefront GraphQL discovery surface теперь включает `storefrontRegions` и `storefrontShippingOptions`, причём `storefrontShippingOptions` уважает cart-context precedence и customer ownership так же, как live `/store/shipping-options`;
- storefront GraphQL surface также включает mutations `createStorefrontPaymentCollection` и `completeStorefrontCheckout`, которые повторяют live semantics store REST для guest/customer cart access и reuse существующей cart-bound payment collection;
- storefront GraphQL surface также покрывает базовый cart lifecycle: `storefrontCart`, `createStorefrontCart`, `addStorefrontCartLineItem`, `updateStorefrontCartLineItem`, `removeStorefrontCartLineItem`, с теми же guest/customer ownership и backend line-item resolution semantics, что и `/store/carts/*`;
- storefront REST и storefront GraphQL теперь channel-aware поверх platform `ChannelContext`: если commerce не включён для request channel через `channel_module_bindings`, storefront surface отвечает как disabled module;
- storefront catalog и shipping discovery используют metadata-based allowlist по `channel_slug`, а checkout/cart mutation path не пропускают товары и shipping options, скрытые для текущего channel context.
- storefront product detail, cart line-item validation и checkout completion теперь также переоценивают product visibility и доступный inventory по видимым stock locations для текущего `channel_slug`.
- storefront shipping discovery, cart context patch и checkout также используют metadata-backed shipping profile compatibility: `shipping_profile.slug` на product и `shipping_profiles.allowed_slugs` на shipping option.
- admin/storefront product read contracts и admin write contracts теперь также знают first-class `shipping_profile_slug`, а shipping option contracts знают first-class `allowed_shipping_profile_slugs`; на текущем этапе оба поля нормализуются в metadata-backed storage без отдельной schema migration.
- admin transport для delivery теперь включает shipping-option management: REST `/admin/shipping-options*` и GraphQL `shippingOptions/shippingOption/createShippingOption/updateShippingOption` работают поверх того же `FulfillmentService`, что и storefront/read path.
- storefront GraphQL cart context patch `updateStorefrontCartContext` использует tri-state input contract (`omitted` vs `null` vs explicit value) и повторяет semantics live `POST /store/carts/{id}` без потери patch-значения;
- storefront locale может приходить через `locale` query param и `x-medusa-locale`;
- storefront cart line items описываются как `variant_id + quantity`, а title/price резолвятся backend-ом;
- storefront cart context обновляется route `POST /store/carts/{id}` и persist'ится в cart snapshot;
- `shipping-options`, `payment-collections` и `checkout` читают storefront context из cart как из source of truth;
- `complete checkout` reuse-ит уже существующий cart-bound payment collection, если storefront ранее создал его через `/store/payment-collections`;
- guest cart может завершать checkout без auth, но customer-owned cart остаётся доступен только matching customer context;
- checkout использует промежуточный статус `checking_out`, а повторные запросы должны стремиться к reuse/recovery existing records.

## GraphQL subscriptions

- HTTP queries/mutations остаются на `/api/graphql`;
- subscriptions идут через `/api/graphql/ws`;
- browser clients передают `token`, `tenantSlug` и `locale` через `connection_init`;
- tenant resolution для WebSocket-пути происходит внутри GraphQL handshake.

## Auth transport consistency

Для auth/user сценариев (`register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile`, `create_user`) REST и GraphQL работают как thin adapters поверх общего `AuthLifecycleService`.

Это даёт:

- единый session lifecycle contract;
- единый error mapping через типизированные ошибки;
- общую observability-поверхность для auth flow.

Перед релизом auth-изменений используется:

```bash
scripts/auth_release_gate.sh --require-all-gates \
  --parity-report <staging-parity-report> \
  --security-signoff <security-signoff>
```

## MCP как отдельный API-surface

Платформа поддерживает MCP через `crates/rustok-mcp`, но локальная документация описывает только RusToK integration layer, а не переопределяет upstream protocol semantics.

Server-side management surface уже включает:

- REST `/api/mcp/*`;
- GraphQL `mcp*`;
- runtime bridge `DbBackedMcpRuntimeBridge` для persisted token/policy/audit resolution.

## Rich-text input contract

Для blog/forum/pages transport-слои поддерживают:

- legacy режим: `markdown`;
- rich режим: `rt_json_v1` с обязательным `content_json`.

Для tenant-by-tenant перевода legacy markdown используется migration job `migrate_legacy_richtext`.

## Связанные документы

- [routing.md](./routing.md)
- [overview.md](./overview.md)
- [UI GraphQL architecture](../UI/graphql-architecture.md)
