# API Architecture

РџРѕР»РёС‚РёРєР° РёСЃРїРѕР»СЊР·РѕРІР°РЅРёСЏ API-СЃС‚РёР»РµР№ РѕРїРёСЃР°РЅР° РІ [`docs/architecture/routing.md`](./routing.md).

## РљСЂР°С‚РєРѕРµ СЂРµР·СЋРјРµ

RusToK РёСЃРїРѕР»СЊР·СѓРµС‚ РіРёР±СЂРёРґРЅС‹Р№ РїРѕРґС…РѕРґ: GraphQL РґР»СЏ UI-РєР»РёРµРЅС‚РѕРІ, REST РґР»СЏ РёРЅС‚РµРіСЂР°С†РёР№ Рё СЃР»СѓР¶РµР±РЅС‹С… СЃС†РµРЅР°СЂРёРµРІ.

| API | Endpoint | РќР°Р·РЅР°С‡РµРЅРёРµ |
|-----|----------|-----------|
| GraphQL | `/api/graphql` | Р•РґРёРЅС‹Р№ endpoint РґР»СЏ admin Рё storefront UI |
| REST | `/api/v1/вЂ¦` | Р’РЅРµС€РЅРёРµ РёРЅС‚РµРіСЂР°С†РёРё, webhooks, batch jobs |
| Commerce REST (legacy) | `/api/commerce/…` | РџРµСЂРµС…РѕРґРЅС‹Р№ compatibility-слой ecommerce module |
| Commerce REST (store/admin) | `/store/…`, `/admin/…` | Medusa-style transport surface для storefront/admin ecommerce flows |
| OpenAPI | `/api/openapi.json`, `/api/openapi.yaml` | РњР°С€РёРЅРѕС‡РёС‚Р°РµРјР°СЏ СЃРїРµС†РёС„РёРєР°С†РёСЏ REST API (РіРµРЅРµСЂРёСЂСѓРµС‚СЃСЏ С‡РµСЂРµР· `utoipa`) |
| Health | `/health`, `/health/live`, `/health/ready`, `/health/modules` | РЎС‚Р°С‚СѓСЃ РїСЂРѕС†РµСЃСЃР°, readiness Рё health РјРѕРґСѓР»РµР№ |
| Metrics | /metrics | Prometheus РјРµС‚СЂРёРєРё |

### Ecommerce benchmark

- для ecommerce-направления эталонным функциональным контрактом считается **Medusa REST API**;
- RusToK сознательно поддерживает более широкую transport-модель: `REST + GraphQL`;
- GraphQL в ecommerce не должен вводить отдельную бизнес-semantics и должен переиспользовать те же
  application services, workflows и доменные модули, что и REST `/store/*` + `/admin/*`;
- при конфликте между legacy ecommerce behavior и официальным Medusa contract приоритет у Medusa,
  если нет отдельного RusToK-specific ADR.

### GraphQL subscriptions

- HTTP queries/mutations остаются на `/api/graphql`.
- GraphQL subscriptions идут через отдельный WebSocket endpoint `/api/graphql/ws`.
- Browser clients передают auth/tenant/locale в `connection_init` payload (`token`, `tenantSlug`, `locale`), потому что стандартный browser WebSocket не умеет слать кастомные headers вроде `Authorization` и `X-Tenant-Slug`.
- Для `/api/graphql/ws` tenant context резолвится внутри GraphQL handshake, а не через обычный header-based tenant middleware.

## MCP РєР°Рє РѕС‚РґРµР»СЊРЅС‹Р№ API-surface

РџРѕРјРёРјРѕ GraphQL/REST, РїР»Р°С‚С„РѕСЂРјР° РїРѕРґРґРµСЂР¶РёРІР°РµС‚ MCP С‡РµСЂРµР· `crates/rustok-mcp`. Р’Р°Р¶РЅРѕ: Р»РѕРєР°Р»СЊРЅР°СЏ
РґРѕРєСѓРјРµРЅС‚Р°С†РёСЏ RusToK РЅРµ РґРѕР»Р¶РЅР° РїРµСЂРµРѕРїСЂРµРґРµР»СЏС‚СЊ СЃР°Рј MCP-РїСЂРѕС‚РѕРєРѕР». Р”Р»СЏ protocol semantics, security Рё
authorization flow РёСЃС‚РѕС‡РЅРёРєРѕРј РёСЃС‚РёРЅС‹ СЃС‡РёС‚Р°СЋС‚СЃСЏ РѕС„РёС†РёР°Р»СЊРЅС‹Рµ РґРѕРєСѓРјРµРЅС‚С‹:

- [MCP docs](https://modelcontextprotocol.io/docs)
- [MCP spec](https://modelcontextprotocol.io/specification/2025-03-26)
- [`rmcp` docs](https://docs.rs/rmcp/latest/rmcp/)
- [Authorization guide](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- [Security best practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

Р›РѕРєР°Р»СЊРЅС‹Рµ РґРѕРєСѓРјРµРЅС‚С‹ RusToK С„РёРєСЃРёСЂСѓСЋС‚ С‚РѕР»СЊРєРѕ РЅР°С€ РёРЅС‚РµРіСЂР°С†РёРѕРЅРЅС‹Р№ СЃР»РѕР№, РІРєР»СЋС‡Р°СЏ С‚РµРєСѓС‰РёР№ tool surface,
identity/policy foundation (`McpIdentity`, `McpAccessContext`, `mcp_whoami`), session-start runtime
binding (`McpSessionContext`, `McpAccessResolver`, `McpRuntimeBinding`), РїРµСЂРІС‹Р№ Alloy product-slice
`alloy_scaffold_module`, РµРіРѕ review/apply boundary Рё РѕС‚РєСЂС‹С‚С‹Рµ gap-С‹ РїР»Р°С‚С„РѕСЂРјС‹.

РќР° СЃС‚РѕСЂРѕРЅРµ РїР»Р°С‚С„РѕСЂРјРµРЅРЅРѕРіРѕ server-layer СѓР¶Рµ РїРѕРґРЅСЏС‚ persisted management surface РґР»СЏ MCP:

- REST: `/api/mcp/clients`, `/api/mcp/clients/{id}`, `/api/mcp/clients/{id}/rotate-token`, `/api/mcp/clients/{id}/policy`, `/api/mcp/tokens/{id}/revoke`, `/api/mcp/clients/{id}/deactivate`, `/api/mcp/audit`, `/api/mcp/scaffold-drafts`, `/api/mcp/scaffold-drafts/{id}`, `/api/mcp/scaffold-drafts/{id}/apply`
- GraphQL: `mcpClients`, `mcpClient`, `mcpAuditEvents`, `createMcpClient`, `rotateMcpClientToken`, `updateMcpClientPolicy`, `revokeMcpToken`, `deactivateMcpClient`, `mcpModuleScaffoldDrafts`, `mcpModuleScaffoldDraft`, `stageMcpModuleScaffoldDraft`, `applyMcpModuleScaffoldDraft`
- Runtime bridge: `apps/server/src/services/mcp_runtime.rs` РїРѕРґРЅРёРјР°РµС‚ `DbBackedMcpRuntimeBridge`, РєРѕС‚РѕСЂС‹Р№ СЂРµР·РѕР»РІРёС‚ plaintext MCP token РІ effective `McpAccessContext`, РѕР±РЅРѕРІР»СЏРµС‚ `last_used_at`, РїРёС€РµС‚ runtime audit `allowed/denied` РїРѕ tool invocations Рё РјРѕР¶РµС‚ РІС‹СЃС‚СѓРїР°С‚СЊ persisted `McpScaffoldDraftStore` РґР»СЏ Alloy scaffold tools

Р­С‚РѕС‚ СЃР»РѕР№ С…СЂР°РЅРёС‚ `clients/tokens/policies/audit`, РЅРѕ РїРѕРєР° РЅРµ РїРѕРґРјРµРЅСЏРµС‚ РѕС„РёС†РёР°Р»СЊРЅС‹Р№ MCP authorization flow Рё РЅРµ СЃС‡РёС‚Р°РµС‚СЃСЏ Р·Р°РјРµРЅРѕР№ upstream security model.

Р”Р»СЏ Alloy С‡РµСЂРµР· MCP СЃРµРіРѕРґРЅСЏ СѓР¶Рµ РґРѕСЃС‚СѓРїРµРЅ РїРµСЂРІС‹Р№ СЂРµР°Р»СЊРЅС‹Р№ СЃРѕР·РёРґР°С‚РµР»СЊРЅС‹Р№ СЃС†РµРЅР°СЂРёР№: `alloy_scaffold_module`
stage-РёС‚ draft `crates/rustok-<slug>` module scaffold, `alloy_review_module_scaffold` РґР°С‘С‚ review,
Р° `alloy_apply_module_scaffold` РїРёС€РµС‚ РµРіРѕ РІ workspace С‚РѕР»СЊРєРѕ СЃ `confirm=true`. РџР°СЂР°Р»Р»РµР»СЊРЅРѕ
`apps/server` СѓР¶Рµ С…СЂР°РЅРёС‚ persisted scaffold drafts, РїСѓР±Р»РёРєСѓРµС‚ РґР»СЏ РЅРёС… management API Рё РјРѕР¶РµС‚
РїРѕРґРєР»СЋС‡Р°С‚СЊ РёС… РѕР±СЂР°С‚РЅРѕ РІ live MCP runtime С‡РµСЂРµР· `DbBackedMcpRuntimeBridge`. Р­С‚Рѕ РІСЃС‘ РµС‰С‘ РЅРµ РїРѕР»РЅС‹Р№
codegen/publish pipeline Рё РїРѕРєР° РµС‰С‘ РЅРµ server-owned remote MCP transport/bootstrap.

## Auth transport consistency

Р”Р»СЏ auth/user СЃС†РµРЅР°СЂРёРµРІ (`register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile`) REST Рё GraphQL СЂР°Р±РѕС‚Р°СЋС‚ РєР°Рє thin adapters Рё РёСЃРїРѕР»СЊР·СѓСЋС‚ РѕР±С‰РёР№ application service `AuthLifecycleService` (`apps/server/src/services/auth_lifecycle.rs`).

Р­С‚Рѕ СЃРЅРёР¶Р°РµС‚ РґСѓР±Р»РёСЂРѕРІР°РЅРёРµ Р±РёР·РЅРµСЃ-Р»РѕРіРёРєРё РјРµР¶РґСѓ transport-СЃР»РѕСЏРјРё Рё С„РёРєСЃРёСЂСѓРµС‚ РµРґРёРЅС‹Рµ policy РґР»СЏ session invalidation.

## Auth lifecycle consistency Рё release-gate

### Р•РґРёРЅС‹Р№ application service

Auth/user СЃС†РµРЅР°СЂРёРё (`register/sign_in`, `login/sign_in`, `refresh`, `change_password`, `reset_password`, `update_profile`, `create_user`) СЂРµР°Р»РёР·РѕРІР°РЅС‹ С‡РµСЂРµР· РѕР±С‰РёР№ `AuthLifecycleService` (`apps/server/src/services/auth_lifecycle.rs`), Р° transport-СЃР»РѕРё REST/GraphQL РІС‹СЃС‚СѓРїР°СЋС‚ thin adapters.

### Р•РґРёРЅР°СЏ policy СЃРµСЃСЃРёР№

- `reset_password` / `confirm_reset` РѕС‚Р·С‹РІР°СЋС‚ РІСЃРµ Р°РєС‚РёРІРЅС‹Рµ СЃРµСЃСЃРёРё РїРѕР»СЊР·РѕРІР°С‚РµР»СЏ.
- `change_password` РѕС‚Р·С‹РІР°РµС‚ РІСЃРµ СЃРµСЃСЃРёРё, РєСЂРѕРјРµ С‚РµРєСѓС‰РµР№ (С‡РµСЂРµР· `except_session_id`).
- `sign_out` РёСЃРїРѕР»СЊР·СѓРµС‚ soft-revoke (`sessions.revoked_at`) РІРјРµСЃС‚Рѕ hard delete.

### Transport-РєРѕРЅС‚СЂР°РєС‚С‹ РѕС€РёР±РѕРє

Р”Р»СЏ РєР»СЋС‡РµРІС‹С… auth-РѕС€РёР±РѕРє РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ С‚РёРїРёР·РёСЂРѕРІР°РЅРЅС‹Р№ РєРѕРЅС‚СЂР°РєС‚ `AuthLifecycleError` СЃ РµРґРёРЅРѕРѕР±СЂР°Р·РЅС‹Рј mapping РІ REST/GraphQL (РІ С‚.С‡. `InvalidResetToken`, `UserInactive`, `UserNotFound`, `InvalidCredentials`).

### Observability

`/metrics` РїСѓР±Р»РёРєСѓРµС‚ auth lifecycle counters:

- `auth_password_reset_sessions_revoked_total`
- `auth_change_password_sessions_revoked_total`
- `auth_flow_inconsistency_total`
- `auth_login_inactive_user_attempt_total`

### Pre-release gate (РѕРїРµСЂР°С†РёРѕРЅРЅС‹Р№)

РџРµСЂРµРґ РІС‹РєР»Р°РґРєРѕР№ РѕР±СЏР·Р°С‚РµР»РµРЅ Р·Р°РїСѓСЃРє:

```bash
scripts/auth_release_gate.sh --require-all-gates \
  --parity-report <staging-parity-report> \
  --security-signoff <security-signoff>
```

РЎРєСЂРёРїС‚:

- Р·Р°РїСѓСЃРєР°РµС‚ Р»РѕРєР°Р»СЊРЅС‹Рµ integration auth-СЃСЂРµР·С‹ (`cargo test -p rustok-server auth_lifecycle` + `cargo test -p rustok-server auth`),
- С„РѕСЂРјРёСЂСѓРµС‚ markdown gate-report Рё Р»РѕРіРё,
- Р·Р°РІРµСЂС€Р°РµС‚ РїСЂРѕРіРѕРЅ СЃ non-zero exit code РїСЂРё РїР°РґРµРЅРёРё Р»СЋР±РѕРіРѕ Р»РѕРєР°Р»СЊРЅРѕРіРѕ auth-СЃСЂРµР·Р° РёР»Рё РїСЂРё РЅРµР·Р°РєСЂС‹С‚С‹С… РѕР±СЏР·Р°С‚РµР»СЊРЅС‹С… gate.

## Rich-text input contract (blog/forum/pages)

Р”Р»СЏ create/update РѕРїРµСЂР°С†РёР№ РІ blog/forum/pages transport-СЃР»РѕРё (GraphQL/REST) РїРѕРґРґРµСЂР¶РёРІР°СЋС‚:

- legacy СЂРµР¶РёРј: `body_format`/`content_format = "markdown"` + С‚РµРєСЃС‚РѕРІРѕРµ `body`/`content`;
- rich СЂРµР¶РёРј: `body_format`/`content_format = "rt_json_v1"` + РѕР±СЏР·Р°С‚РµР»СЊРЅРѕРµ `content_json`.

Р”Р»СЏ `rt_json_v1` backend РІС‹РїРѕР»РЅСЏРµС‚ РѕР±СЏР·Р°С‚РµР»СЊРЅСѓСЋ server-side РІР°Р»РёРґР°С†РёСЋ Рё sanitize С‡РµСЂРµР· RT JSON pipeline РїРµСЂРµРґ Р·Р°РїРёСЃСЊСЋ.

Р”Р»СЏ РїРѕСЌС‚Р°РїРЅРѕРіРѕ РїРµСЂРµРІРѕРґР° legacy-РєРѕРЅС‚РµРЅС‚Р° (markdown) РІ `rt_json_v1` РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ server migration job `cargo run -p rustok-server --bin migrate_legacy_richtext -- --tenant-id=<uuid> [--dry-run]`:

- РІС‹Р±РёСЂР°РµС‚ С‚РѕР»СЊРєРѕ tenant-scoped Р·Р°РїРёСЃРё `post/comment/forum_topic/forum_reply` СЃ `format=markdown`;
- РєРѕРЅРІРµСЂС‚РёСЂСѓРµС‚ markdown -> `rt_json_v1`, Р·Р°С‚РµРј РїСЂРѕРіРѕРЅСЏРµС‚ С‡РµСЂРµР· С‚РѕС‚ Р¶Рµ server-side sanitize/validation gate (`validate_and_sanitize_rt_json`);
- РІС‹РїРѕР»РЅСЏРµС‚ safe update СЃ retry (optimistic guard РїРѕ `id + updated_at + format`), С‡С‚РѕР±С‹ РЅРµ РїРµСЂРµС‚РёСЂР°С‚СЊ РєРѕРЅРєСѓСЂРµРЅС‚РЅС‹Рµ РёР·РјРµРЅРµРЅРёСЏ;
- РїСѓР±Р»РёРєСѓРµС‚ СЃС‡С‘С‚С‡РёРєРё РїСЂРѕРіРѕРЅР°: `processed/succeeded/failed/skipped`;
- РїРѕРґРґРµСЂР¶РёРІР°РµС‚ idempotent restart С‡РµСЂРµР· checkpoint-С„Р°Р№Р» (`--checkpoint-file`, РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ `scripts/checkpoints/legacy_richtext.json`).

### Rollout/rollback РґР»СЏ migration job (tenant-by-tenant)

1. `dry-run` РґР»СЏ РѕРґРЅРѕРіРѕ tenant, РїСЂРѕРІРµСЂРёС‚СЊ `failed=0` Рё РІС‹Р±РѕСЂРєСѓ С‚РѕР»СЊРєРѕ РѕР¶РёРґР°РµРјС‹С… kind/locale.
2. apply РґР»СЏ СЌС‚РѕРіРѕ Р¶Рµ tenant СЃ РІС‹РґРµР»РµРЅРЅС‹Рј checkpoint-С„Р°Р№Р»РѕРј.
3. smoke-read (GraphQL/API) РїРѕ post/comment/topic/reply Рё РїСЂРѕРІРµСЂРєР° С„РѕСЂРјР°С‚Р° `rt_json_v1`.
4. РїРѕРІС‚РѕСЂРёС‚СЊ С†РёРєР» РґР»СЏ СЃР»РµРґСѓСЋС‰РµРіРѕ tenant; РЅРµ Р·Р°РїСѓСЃРєР°С‚СЊ multi-tenant bulk Р±РµР· checkpoint isolation.

Rollback СЃС‚СЂР°С‚РµРіРёСЏ:

- РєРѕРґРѕРІС‹Р№ rollback: РѕСЃС‚Р°РЅРѕРІРёС‚СЊ job Рё РІРµСЂРЅСѓС‚СЊ read/write С‚СЂР°С„РёРє РЅР° legacy markdown (РѕР±СЂР°С‚РЅР°СЏ СЃРѕРІРјРµСЃС‚РёРјРѕСЃС‚СЊ СЃРѕС…СЂР°РЅСЏРµС‚СЃСЏ РєРѕРЅС‚СЂР°РєС‚РѕРј API);
- data rollback: РІРѕСЃСЃС‚Р°РЅРѕРІРёС‚СЊ Р·Р°РїРёСЃРё РєРѕРЅРєСЂРµС‚РЅРѕРіРѕ tenant РёР· DB backup/snapshot, СЃРЅСЏС‚РѕРіРѕ РїРµСЂРµРґ apply;
- РїСЂРё С‡Р°СЃС‚РёС‡РЅРѕРј РїР°РґРµРЅРёРё РІРѕР·РѕР±РЅРѕРІР»СЏС‚СЊ СЃ С‚РѕРіРѕ Р¶Рµ checkpoint (РёРґРµРјРїРѕС‚РµРЅС‚РЅРѕ) РІРјРµСЃС‚Рѕ СЂСѓС‡РЅРѕРіРѕ РјР°СЃСЃРѕРІРѕРіРѕ rewrite.

## GraphQL СЃС…РµРјР°

GraphQL СЃС…РµРјР° С„РѕСЂРјРёСЂСѓРµС‚СЃСЏ РёР· per-domain РѕР±СЉРµРєС‚РѕРІ С‡РµСЂРµР· `MergedObject`:

- `CommerceQuery` / `CommerceMutation` вЂ” `rustok-commerce`
- `ContentQuery` / `ContentMutation` вЂ” `rustok-content`
- `BlogQuery` / `BlogMutation` вЂ” `rustok-blog`
- `ForumQuery` / `ForumMutation` вЂ” `rustok-forum`
- `AlloyQuery` / `AlloyMutation` вЂ” `alloy` (transport) РїРѕРІРµСЂС… `alloy-scripting` (runtime)

РўРѕС‡РєР° СЃР±РѕСЂРєРё СЃС…РµРјС‹: `apps/server/src/graphql/schema.rs`

## РЎРІСЏР·Р°РЅРЅС‹Рµ РґРѕРєСѓРјРµРЅС‚С‹

- [Routing policy](./routing.md) вЂ” РґРµС‚Р°Р»СЊРЅР°СЏ policy GraphQL vs REST
- [Architecture overview](./overview.md)
- [UI GraphQL architecture](../UI/graphql-architecture.md)

### Medusa-style ecommerce transport

- storefront locale может приходить не только через `locale` query param, но и через header `x-medusa-locale`, как в Medusa Store API;
- storefront cart line items для `/store/carts/{id}/line-items` должны описываться клиентом как `variant_id + quantity`, а title/price backend резолвит из catalog/pricing слоя;
- update/remove line items идут через `/store/carts/{id}/line-items/{line_id}`.
