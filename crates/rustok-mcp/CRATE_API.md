# rustok-mcp / CRATE_API

## Публичные модули
`access`, `alloy_tools`, `runtime`, `server`, `tools`.

## Основные публичные типы и сигнатуры
- `pub async fn serve_stdio(config: McpServerConfig) -> Result<...>`
- `pub struct McpServerConfig`
- `pub struct RusToKMcpServer`
- `pub struct McpIdentity`
- `pub struct McpAccessContext`
- `pub struct McpAccessPolicy`
- `pub struct McpToolRequirement`
- `pub struct McpWhoAmIResponse`
- `pub struct McpSessionContext`
- `pub struct McpRuntimeBinding`
- `pub struct McpScaffoldDraftRuntimeContext`
- `pub struct McpToolCallAuditEvent`
- `pub struct ScaffoldModuleRequest`
- `pub struct ScaffoldModuleFile`
- `pub struct ScaffoldModulePreview`
- `pub struct StageModuleScaffoldResponse`
- `pub struct ReviewModuleScaffoldRequest`
- `pub struct ReviewModuleScaffoldResponse`
- `pub struct ApplyModuleScaffoldRequest`
- `pub struct ApplyModuleScaffoldResponse`
- `pub struct StagedModuleScaffold`
- `pub enum ModuleScaffoldDraftStatus`
- `pub fn generate_module_scaffold(request: &ScaffoldModuleRequest) -> Result<ScaffoldModulePreview, ...>`
- `pub fn apply_staged_scaffold(draft: &StagedModuleScaffold, workspace_root: &str) -> Result<ApplyModuleScaffoldResponse, ...>`
- `pub const TOOL_ALLOY_SCAFFOLD_MODULE: &str`
- `pub const TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD: &str`
- `pub const TOOL_ALLOY_APPLY_MODULE_SCAFFOLD: &str`
- `pub trait McpAccessResolver`
- `pub trait McpAuditSink`
- `pub trait McpScaffoldDraftStore`
- `pub trait McpManagementPort`
- `pub struct StageMcpScaffoldDraftCommand`
- `pub struct ApplyMcpScaffoldDraftCommand`
- `pub struct McpScaffoldDraftRecord`
- Публичные MCP tools из `tools::*` и `alloy_tools::*`.

## События
- Публикует: N/A (RPC/MCP адаптер).
- Потребляет: команды/запросы MCP клиента.

## Зависимости от других rustok-крейтов
- `rustok-core`

## Частые ошибки ИИ
- Путает MCP runtime и crate `rustok-mcp`.
- Считает `enabled_tools` полноценной authz-моделью, хотя теперь это только compatibility shim.
- Документирует MCP spec локально вместо ссылки на официальный upstream.
- Считает `alloy_scaffold_module` генерацией готового production-модуля, хотя это только staged draft scaffold.
- Пытается регистрировать scaffold-модуль в runtime без собственного permission surface в `rustok-core`.

## Минимальный набор контрактов

### Входные DTO/команды
- Входной контракт формируется публичными DTO/командами из crate и соответствующими `pub`-экспортами в `src/lib.rs`.
- Все изменения публичных полей DTO считаются breaking-change и требуют синхронного обновления transport-адаптеров и MCP-клиентов, которые на них опираются.
- Для access-layer breaking-change также считаются изменения в `McpIdentity`, `McpAccessContext`, `McpAccessPolicy`, `McpToolRequirement`, `McpWhoAmIResponse`, `McpSessionContext`, `McpRuntimeBinding`, `McpToolCallAuditEvent`.
- Для Alloy module scaffolding breaking-change считаются изменения в `ScaffoldModuleRequest`, `ScaffoldModulePreview`, `StageModuleScaffoldResponse`, `ReviewModuleScaffoldRequest`, `ReviewModuleScaffoldResponse`, `ApplyModuleScaffoldRequest`, `ApplyModuleScaffoldResponse`, `StagedModuleScaffold` и семантике `TOOL_ALLOY_SCAFFOLD_MODULE` / `TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD` / `TOOL_ALLOY_APPLY_MODULE_SCAFFOLD`.

### Доменные инварианты
- Инварианты multi-tenant boundary (tenant/resource isolation, auth context) считаются обязательной частью контракта.
- Tool authorization в `rustok-mcp` сначала проверяет coarse-grained legacy allow-list, затем MCP access policy/permissions/scopes.
- Persisted MCP auth bind выполняется на старте сессии через `McpAccessResolver`; `rustok-mcp` не тащит внутрь себя server-specific ORM/runtime код.
- Persisted Alloy draft flow может быть подключён через `McpScaffoldDraftStore`; crate не должен жёстко зависеть от server-specific DB/ORM реализации.
- `mcp_health` остаётся операционным introspection tool и не должен ломаться от отсутствия доменных permission mapping.
- `alloy_scaffold_module` может только stage preview draft crate skeleton и не должен:
  - перезаписывать существующий crate;
  - автоматически регистрировать модуль в runtime;
  - подменять review/apply границу для generated code.
- `alloy_apply_module_scaffold` должен требовать явное подтверждение `confirm=true` и не должен обходить предшествующий review step.
- Persisted scaffold draft control plane живёт в `apps/server` (`mcp_scaffold_drafts`, REST `/api/mcp/scaffold-drafts*`, GraphQL `mcpModuleScaffoldDraft*`) и не подменяет локальный crate API `rustok-mcp`.
- GraphQL и Leptos native adapters обязаны делегировать через `McpManagementPort` в server-owned `McpManagementService`; UI package и owner GraphQL не содержат scaffold persistence, filesystem apply или audit SQL.

### События / outbox-побочные эффекты
- Если модуль публикует доменные события, публикация должна идти через транзакционный outbox/transport-контракт без локальных обходов.
- Формат event payload и event-type должен оставаться обратно-совместимым для межмодульных потребителей.

### Ошибки / коды отказов
- Публичные `*Error`/`*Result` типы модуля определяют контракт отказов и не должны терять семантику при маппинге в HTTP/GraphQL/CLI.
- Для validation/auth/conflict/not-found сценариев должен сохраняться устойчивый error-class, используемый тестами и адаптерами.
- Для MCP access-layer стабильными считаются коды `tool_disabled`, `tool_not_allowed`, `tool_denied`, `missing_permissions`, `missing_scopes`.
- Runtime tool audit contract через `McpToolCallAuditEvent` считает состояния `allowed`/`denied`, но не переопределяет upstream MCP authorization semantics.
- Для scaffold review/apply слоя стабильными считаются отказы при невалидном slug/name/description, попытке прямой записи во время `alloy_scaffold_module`, отсутствии `confirm=true` на `alloy_apply_module_scaffold` и попытке писать в уже существующий target crate.
