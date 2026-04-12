# Документация Next Admin

Локальная документация для `apps/next-admin`.

## Назначение

`apps/next-admin` является Next.js admin host для RusToK. Он даёт React/Next-путь для админки, работает параллельно с `apps/admin` и монтирует module-owned/admin-owned пакеты вместо того, чтобы переносить модульный UI внутрь host.

## Границы ответственности

- владеть Next.js admin host, routing и shared integration layer;
- монтировать package-owned admin surfaces из `packages/*`;
- использовать canonical frontend contracts для auth, GraphQL, forms и shared UI;
- поддерживать parity с `apps/admin` на уровне платформенных контрактов;
- не забирать module-owned business UI в host-код.

## Runtime contract

- canonical FSD-слои для host: `app`, `shared`, `entities`, `features`, `widgets`;
- backend integration идёт через `apps/server` и shared transport packages;
- глобальный admin search использует `rustok-search` как host-level capability;
- legacy import paths допускаются только как временный compatibility layer;
- новый код должен идти через canonical FSD paths и shared package boundaries.

## Ownership contract для module UI

- Если модуль поставляет admin UI, он остаётся module-owned package рядом с модулем или в `packages/*`.
- Host `apps/next-admin` выступает только composition root.
- То же правило действует для core-modules, optional-modules и capability packages.
- Capability-owned surface `rustok-ai` монтируется как package-owned UI, а не как ad-hoc host feature.
- Route-selection contract обязан быть в parity с `apps/admin`: selection state URL-owned,
  используются только typed `snake_case` query keys, invalid key не fallback’ится на first item,
  а локальные Next helpers не inventят отдельную schema/policy поверх `rustok-api` contract.

## Пакеты и интеграции

- shared UI и frontend contracts идут через `UI/next` и внутренние transport/auth packages;
- backend — `apps/server`;
- module-owned Next admin packages живут в `apps/next-admin/packages/*`;
- package naming contract для module-owned admin UI остаётся `@rustok/*-admin`.

## Взаимодействия

- `apps/server` предоставляет API/runtime contract;
- `apps/admin` остаётся primary Leptos admin stack и референсом для parity;
- module-owned UI packages подключаются как внешние surfaces, а не как host-owned business code.

## Проверка

- typecheck/lint прогоны по `apps/next-admin`
- точечные проверки package-owned admin surfaces
- сверка shared contract с `docs/UI/*` и `docs/modules/manifest.md`

## Связанные документы

- [Implementation Plan](./implementation-plan.md)
- [Navigation RBAC](./nav-rbac.md)
- [Clerk setup](./clerk_setup.md)
- [Themes](./themes.md)
- [Карта документации](../../../docs/index.md)
