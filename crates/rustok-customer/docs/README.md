# Документация `rustok-customer`

`rustok-customer` — дефолтный storefront-customer подмодуль семейства `ecommerce`.

## Назначение

- схема `customers`;
- `CustomerModule` и `CustomerService`;
- module-owned admin UI пакет `rustok-customer/admin`;
- customer profile boundary, отделённый от platform/admin user;
- optional linkage на `user_id` для сценариев `store/customers/me`;
- optional service-level bridge `customer -> user -> profile`, который может вернуть customer вместе с `ProfileSummary`.

## Зона ответственности

- модуль не зависит от `rustok-commerce` umbrella, чтобы не создавать цикл;
- customer profile хранится отдельно от auth/user домена;
- связь с пользователем опциональна и не отменяет самостоятельность customer-модели;
- bridge к `profiles` остаётся опциональным read-contract и не превращает customer в канонический public profile;
- admin UI ownership теперь живёт в `rustok-customer/admin`, а storefront GraphQL и REST transport пока остаются в фасаде `rustok-commerce`.

## Интеграция

- модуль входит в ecommerce family и должен сохранять собственную storage/runtime-границу без возврата ответственности в umbrella `rustok-commerce`;
- storefront transport и GraphQL по-прежнему публикуются через `rustok-commerce`, но admin UI-поверхность уже зафиксирована как отдельный module-owned surface в `rustok-customer/admin`;
- изменения cross-module контракта нужно синхронизировать с `rustok-commerce` и соседними split-модулями.

## Проверка

- cargo xtask module validate customer
- cargo xtask module test customer
- targeted commerce tests для customer-домена при изменении runtime wiring

## Связанные документы

- [README crate](../README.md)
- [План распила commerce](../../rustok-commerce/docs/implementation-plan.md)
