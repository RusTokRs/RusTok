# План верификации платформы: API-поверхности

- **Статус:** Актуализированный детальный чеклист
- **Контур:** GraphQL и REST контракты платформы
- **Companion-план:** [Rolling-план RBAC для server и runtime-модулей](./rbac-server-modules-verification-plan.md)

---

## Фаза 8: API — GraphQL

GraphQL остаётся обязательным transport surface даже там, где Leptos UI добавил native `#[server]` path. Проверка GraphQL не пропускается из-за наличия `/api/fn/*`.

### 8.1 Сборка schema и composition-root

**Файлы:**
- `apps/server/src/graphql/schema.rs`
- `apps/server/src/graphql/mod.rs`
- `apps/server/src/graphql/queries.rs`
- `apps/server/src/graphql/mutations.rs`

- [ ] `Query`, `Mutation` и `Subscription` собираются через актуальный composition root.
- [ ] В schema отражены current core surfaces: `Root`, `Auth`, `OAuth`, `MCP`, `Settings`, `System`, `Flex`.
- [ ] Optional modules добавляются через codegen/feature-gated composition и не описаны вручную там, где уже используется generated wiring.
- [ ] Alloy GraphQL surface документируется только при активном feature/runtime path.
- [ ] Playground/WebSocket endpoints соответствуют текущему server routing.

### 8.2 Auth / Users / Settings / Flex

- [ ] GraphQL auth surface отражает текущие mutations и queries (`sign_in`, `sign_up`, refresh, sessions, logout, profile/password flows).
- [ ] User-management mutations в `RootMutation` не расходятся с текущей RBAC-моделью.
- [ ] Settings GraphQL surface отражает текущие platform settings queries/mutations.
- [ ] Flex GraphQL surface отражает текущие field-definition queries/mutations и typed permission gates.

### 8.3 OAuth и MCP

- [ ] OAuth GraphQL surface отражает текущие app-management и consent scenarios.
- [ ] MCP GraphQL surface отражает текущие client/policy/token/scaffold-draft scenarios.
- [ ] Документация не обещает GraphQL operations, которых в коде нет.

### 8.4 Optional modules

- [ ] Content GraphQL surface соответствует текущему crate/server composition.
- [ ] Commerce GraphQL surface соответствует текущему crate/server composition.
- [ ] Blog GraphQL surface соответствует текущему crate/server composition.
- [ ] Forum GraphQL surface соответствует текущему crate/server composition.
- [ ] Pages GraphQL surface соответствует текущему crate/server composition.
- [ ] Media GraphQL surface соответствует текущему crate/server composition.
- [ ] Workflow GraphQL surface соответствует текущему crate/server composition.

### 8.5 Dataloaders и observability

- [ ] Registered dataloaders соответствуют текущему набору batch loaders.
- [ ] GraphQL observability extension соответствует текущему tracing/metrics contract.
- [ ] Subscription surface отражает текущий build progress/event flow path.

---

## Фаза 9: API — REST

### 9.1 Базовые platform endpoints

- [ ] `/health`, `/health/live`, `/health/ready`, `/health/runtime`, `/health/modules` отражены в плане.
- [ ] `/metrics` отражён в плане.
- [ ] `/api/openapi.json` и `/api/openapi.yaml` отражены в плане.
- [ ] `/api/graphql` и `/api/graphql/ws` отражены в плане как transport endpoints, а не domain REST.
- [ ] `/api/fn/*` отражён в плане как Leptos server-functions transport endpoint, который живёт параллельно `/api/graphql`.

### 9.2 Auth и users

- [ ] Auth REST surface отражает текущие endpoints: register/login/refresh/logout/me/invite/reset/verify/sessions/profile/history/change-password.
- [ ] Users REST surface отражает текущие list/get endpoints и их RBAC requirements.

### 9.3 OAuth и metadata

- [ ] OAuth REST surface отражает authorize/consent/token/userinfo/revoke/browser-session сценарии.
- [ ] OIDC/OAuth metadata endpoints из `oauth_metadata.rs` отражены в плане.
- [ ] Browser consent flow не описан как чисто machine-to-machine сценарий.

### 9.4 MCP и admin operational endpoints

- [ ] MCP REST surface отражает clients/policy/token/scaffold-draft/audit endpoints.
- [ ] Admin DLQ/replay endpoints отражены в плане как operational/admin-only surface.

### 9.5 Optional module REST surfaces

- [ ] Content REST
- [ ] Commerce REST
- [ ] Blog REST
- [ ] Forum REST
- [ ] Pages REST
- [ ] Media REST
- [ ] Workflow REST

Для каждого:
- [ ] путь и route prefix соответствуют текущему router registration;
- [ ] RBAC-gates соответствуют текущим extractors/guards;
- [ ] tenant scoping соответствует текущему `TenantContext` usage;
- [ ] OpenAPI annotations не расходятся с фактическими handlers.

### 9.6 Alloy и capability surfaces

- [ ] REST surface Alloy/capability endpoints документируется только если реально подключена в server router/current feature set.
- [ ] Capability endpoints не смешаны с API platform modules.

### 9.7 Rate limiting и transport guards

- [ ] Auth rate limiting отражает текущие middleware и path list.
- [ ] Общий API rate limiting отражает текущую конфигурацию.
- [ ] Transport-level guards и error mapping соответствуют реальному коду.
