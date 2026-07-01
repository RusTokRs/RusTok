# Владение shared API contracts и граница runtime

- Date: 2026-07-01
- Status: Accepted

## Context

`Port*`, permission и locale contracts были определены в `rustok-core`, но
публиковались клиентам через `rustok-api`. Поэтому transport-neutral потребитель
API получал весь core runtime, а feature `server` сохранял обратную зависимость
`rustok-api -> rustok-core`.

## Decision

1. `rustok-api` является единственным владельцем `Port*`, `Permission`, `Action`,
   `Resource`, platform locale normalization/matching/candidates и
   `Accept-Language` parsing.
2. `rustok-api` не зависит от `rustok-core` ни в default, ни в `server` feature.
3. Dependency graph направлен только `rustok-core -> rustok-api`.
4. `rustok-core` владеет runtime policy: `UserRole`, `UserStatus`, `Rbac`,
   `PermissionScope`, `SecurityActorKind`, `SecurityContext` и role inference.
   `SecurityContext::system()` является trusted runtime authority, а anonymous
   storefront/GraphQL reads используют `SecurityContext::public_read()`.
5. Core modules/re-exports и compatibility aliases для перенесённых контрактов удалены.
6. Outbox-specific Loco adapter принадлежит `rustok-outbox` и включается его
   feature `loco-adapter`; `rustok-api` не зависит от `rustok-outbox`.
7. Все module ports используют канонический путь `rustok_api::ports::*` или
   root re-exports `rustok_api::*`.

## Consequences

- Клиенты neutral/default `rustok-api` не компилируют core runtime.
- Dependency graph направлен от runtime-модулей к contract layer без цикла.
- Старые core permission/locale/port пути удалены атомарно и не поддерживаются alias-ами.
- User/service port actors получают authority только после строгого разбора UUID,
  roles и permission claims; system authority разрешён только `PortActorKind::System`.
- Отсутствие `AuthContext` на public read endpoint больше не повышается до system
  authority: такие запросы получают `SecurityActorKind::Public` и проходят только
  через public/published/channel-visible read paths.
- Потребители Loco outbox wiring должны явно подключать
  `rustok-outbox/loco-adapter` и использовать `rustok_outbox::loco`.
