# Документация `rustok-customer`

`rustok-customer` — дефолтный storefront-customer подмодуль семейства `ecommerce`.

## Что сейчас внутри

- схема `customers`;
- `CustomerModule` и `CustomerService`;
- customer profile boundary, отделённый от platform/admin user;
- optional linkage на `user_id` для сценариев `store/customers/me`;
- optional service-level bridge `customer -> user -> profile`, который может вернуть customer вместе с `ProfileSummary`.

## Архитектурная граница

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- customer profile хранится отдельно от auth/user домена;
- связь с пользователем опциональна и не отменяет самостоятельность customer-модели;
- bridge к `profiles` остаётся опциональным read-contract и не превращает customer в канонический public profile;
- GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
