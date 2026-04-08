# План верификации платформы: API-поверхности

- **Статус:** актуальный детальный чеклист
- **Контур:** GraphQL, REST, `#[server]`, OpenAPI, host/runtime API contract
- **Companion-план:** [Rolling-план RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md)

---

## Актуальный scoped contract

API-слой RusToK должен оставаться согласованным с current-state transport model:

- GraphQL — canonical UI-facing contract
- REST — integration/ops/module-owned HTTP contract
- `#[server]` — internal Leptos data layer
- OpenAPI — машиночитаемый REST contract

Проверка API-поверхностей не должна ломать это разделение.

## Фаза 1. GraphQL

### 1.1 Schema composition

**Файлы:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/mod.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

- [ ] `Query`, `Mutation` и `Subscription` собираются через актуальный composition root.
- [ ] Core surfaces отражают current-state contract хоста и модулей.
- [ ] Optional modules подключаются через актуальный generated/manifest-driven path, без старого ручного drift.
- [ ] GraphQL routing согласован с `apps/server` и `docs/architecture/api.md`.

### 1.2 Module ownership

- [ ] GraphQL-resolvers используют module/service layer, а не host-local shortcuts.
- [ ] Module-owned GraphQL surfaces согласованы с локальными docs модуля.
- [ ] Capability-owned surfaces не выдаются за платформенные модули.

### 1.3 Locale / auth / RBAC

- [ ] GraphQL path использует единый tenant/auth/RBAC context.
- [ ] Locale contract совпадает с `docs/architecture/i18n.md` и `docs/UI/*`.
- [ ] Нет module-local locale fallback chains внутри API contract.

## Фаза 2. REST и HTTP surfaces

### 2.1 REST boundaries

**Файлы:**
- `apps/server/src/controllers/`
- module-owned `controllers/`

- [ ] REST используется для integration/ops/webhook flows и module-owned HTTP contract.
- [ ] REST не дублирует UI-only GraphQL flows без причины.
- [ ] Commerce-compatible routes отражают текущий host/runtime contract.

### 2.2 Module-owned routing

- [ ] HTTP routes модуля согласованы с `rustok-module.toml`, если модуль публикует transport surface.
- [ ] Host application только монтирует routing, а не становится owner transport-логики модуля.
- [ ] Наличие controller-path без manifest/doc contract не считается завершённым wiring.

## Фаза 3. `#[server]` contract

### 3.1 Leptos internal data layer

- [ ] Для Leptos hosts и module-owned Leptos UI `#[server]` functions остаются preferred internal path.
- [ ] GraphQL сохраняется параллельно и не вырезается.
- [ ] `#[server]` functions не используются как внешний integration API.

## Фаза 4. OpenAPI и operational endpoints

### 4.1 Discovery и ops

- [ ] OpenAPI endpoints публикуют актуальный REST contract.
- [ ] health/metrics/ops endpoints соответствуют текущему server/runtime contract.
- [ ] API documentation layer не расходится с реальным routing.

## Фаза 5. Точечные проверки

### 5.1 Локальный минимум

- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] профильные `cargo test` или `xtask module test <slug>` для затронутых API-модулей
- [ ] targeted GraphQL/REST smoke, если менялся routing или schema contract

## Open blockers

- [ ] Зафиксировать отдельно runtime-only blockers, если их нельзя воспроизвести в текущем локальном окружении.
- [ ] Не превращать этот документ в backlog; blockers описывать кратко и привязывать к owning component.

## Связанные документы

- [Архитектура API](../architecture/api.md)
- [Маршрутизация и transport boundaries](../architecture/routing.md)
- [Архитектура модулей](../architecture/modules.md)
- [Главный README по верификации](./README.md)
