# Документация Next Frontend

Локальная документация для `apps/next-frontend`.

## Назначение

`apps/next-frontend` является Next.js storefront host для RusToK. Он даёт React/Next storefront path, работает параллельно с `apps/storefront` и должен сохранять parity с Leptos storefront на уровне transport/auth/i18n/module contracts.

## Границы ответственности

- владеть Next.js storefront host и его route composition;
- использовать shared frontend contracts для GraphQL, auth, forms и state;
- собирать storefront через `src/app`, `src/modules`, `src/shared` и `src/components`;
- не дублировать transport/auth code по страницам;
- не подменять собой module-owned storefront UI contracts.

## Runtime contract

- host следует FSD-ориентиру `src/app`, `src/modules`, `src/shared`, `src/components`;
- shared integration gateways живут в `src/shared/lib/*`;
- backend API идёт через `apps/server`;
- auth и transport contracts переиспользуются через shared packages, а не через ad-hoc clients;
- storefront host должен оставаться синхронизированным с `apps/storefront` по route/i18n/auth contracts.
- query semantics для module-owned storefront surfaces должны оставаться в parity с `apps/storefront`;
  host не inventит отдельную schema/policy поверх backend и Leptos host contract.

## Frontend contract

- GraphQL contract идёт через shared storefront transport layer;
- auth/session contract идёт через shared auth package boundary;
- forms/state contract переиспользует shared frontend packages;
- i18n route/layout contract должен совпадать с platform storefront expectations.
- если module-owned storefront surface использует query-driven state, Next host обязан держать
  те же key semantics и canonical behavior, что и Leptos storefront.

## Взаимодействия

- `apps/server` — backend/API provider;
- `apps/storefront` — параллельный Leptos storefront host для contract parity;
- `crates/rustok-*` и module-owned surfaces подключаются через backend и frontend integration layer, а не через host-local business logic.

## Проверка

- lint/typecheck прогоны по `apps/next-frontend`
- storefront route/i18n contract checks
- сверка shared contract с `docs/UI/storefront.md` и `docs/modules/manifest.md`

## Связанные документы

- [App README](../README.md)
- [Storefront docs](../../../docs/UI/storefront.md)
- [Контракт manifest-слоя](../../../docs/modules/manifest.md)
- [Карта документации](../../../docs/index.md)
