from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(relative: str, old: str, new: str) -> None:
    path = ROOT / relative
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{relative}: expected one anchor, found {count}: {old[:120]!r}")
    path.write_text(text.replace(old, new, 1))


# Topic owner facade: enforce every legacy create path and publish exact-context seams.
replace_once(
    "crates/rustok-forum/src/services/topic_facade.rs",
    "use rustok_api::{Action, Resource};",
    "use rustok_api::{Action, PortContext, Resource};",
)
replace_once(
    "crates/rustok-forum/src/services/topic_facade.rs",
    "use rustok_outbox::TransactionalEventBus;\n\nuse crate::dto::{",
    "use rustok_outbox::TransactionalEventBus;\n\n"
    "use crate::audience::SharedForumAudienceFactsPort;\n"
    "use crate::dto::{",
)
replace_once(
    "crates/rustok-forum/src/services/topic_facade.rs",
    "use super::topic_owner;\n",
    "use super::topic_create_audience_authorization::ForumTopicCreateAudienceAuthorizationService;\n"
    "use super::topic_owner;\n",
)
replace_once(
    "crates/rustok-forum/src/services/topic_facade.rs",
    "pub struct TopicService {\n    db: DatabaseConnection,\n    inner: topic_owner::TopicService,\n}\n\nimpl TopicService {\n    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        Self {\n            inner: topic_owner::TopicService::new(db.clone(), event_bus),\n            db,\n        }\n    }\n\n    pub async fn create(\n        &self,\n        tenant_id: Uuid,\n        security: SecurityContext,\n        input: CreateTopicInput,\n    ) -> ForumResult<TopicResponse> {\n        self.create_command(tenant_id, security, input.into()).await\n    }\n\n    pub async fn create_command(\n        &self,\n        tenant_id: Uuid,\n        security: SecurityContext,\n        input: CreateTopicCommandInput,\n    ) -> ForumResult<TopicResponse> {\n        let response = self\n            .inner\n            .create_command(tenant_id, security, input)\n            .await?;\n        require_localized_topic_response(response)\n    }\n",
    "pub struct TopicService {\n"
    "    db: DatabaseConnection,\n"
    "    inner: topic_owner::TopicService,\n"
    "    create_audience: ForumTopicCreateAudienceAuthorizationService,\n"
    "}\n\n"
    "impl TopicService {\n"
    "    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n"
    "        Self::with_optional_audience_facts(db, event_bus, None)\n"
    "    }\n\n"
    "    pub fn with_audience_facts(\n"
    "        db: DatabaseConnection,\n"
    "        event_bus: TransactionalEventBus,\n"
    "        facts_port: SharedForumAudienceFactsPort,\n"
    "    ) -> Self {\n"
    "        Self::with_optional_audience_facts(db, event_bus, Some(facts_port))\n"
    "    }\n\n"
    "    fn with_optional_audience_facts(\n"
    "        db: DatabaseConnection,\n"
    "        event_bus: TransactionalEventBus,\n"
    "        facts_port: Option<SharedForumAudienceFactsPort>,\n"
    "    ) -> Self {\n"
    "        Self {\n"
    "            inner: topic_owner::TopicService::new(db.clone(), event_bus),\n"
    "            create_audience: ForumTopicCreateAudienceAuthorizationService::new(\n"
    "                db.clone(),\n"
    "                facts_port,\n"
    "            ),\n"
    "            db,\n"
    "        }\n"
    "    }\n\n"
    "    pub async fn create(\n"
    "        &self,\n"
    "        tenant_id: Uuid,\n"
    "        security: SecurityContext,\n"
    "        input: CreateTopicInput,\n"
    "    ) -> ForumResult<TopicResponse> {\n"
    "        self.create_command_with_optional_audience_context(\n"
    "            tenant_id,\n"
    "            security,\n"
    "            None,\n"
    "            input.into(),\n"
    "        )\n"
    "        .await\n"
    "    }\n\n"
    "    pub async fn create_with_audience_context(\n"
    "        &self,\n"
    "        tenant_id: Uuid,\n"
    "        security: SecurityContext,\n"
    "        context: PortContext,\n"
    "        input: CreateTopicInput,\n"
    "    ) -> ForumResult<TopicResponse> {\n"
    "        self.create_command_with_optional_audience_context(\n"
    "            tenant_id,\n"
    "            security,\n"
    "            Some(context),\n"
    "            input.into(),\n"
    "        )\n"
    "        .await\n"
    "    }\n\n"
    "    pub async fn create_command(\n"
    "        &self,\n"
    "        tenant_id: Uuid,\n"
    "        security: SecurityContext,\n"
    "        input: CreateTopicCommandInput,\n"
    "    ) -> ForumResult<TopicResponse> {\n"
    "        self.create_command_with_optional_audience_context(tenant_id, security, None, input)\n"
    "            .await\n"
    "    }\n\n"
    "    pub async fn create_command_with_audience_context(\n"
    "        &self,\n"
    "        tenant_id: Uuid,\n"
    "        security: SecurityContext,\n"
    "        context: PortContext,\n"
    "        input: CreateTopicCommandInput,\n"
    "    ) -> ForumResult<TopicResponse> {\n"
    "        self.create_command_with_optional_audience_context(\n"
    "            tenant_id,\n"
    "            security,\n"
    "            Some(context),\n"
    "            input,\n"
    "        )\n"
    "        .await\n"
    "    }\n\n"
    "    async fn create_command_with_optional_audience_context(\n"
    "        &self,\n"
    "        tenant_id: Uuid,\n"
    "        security: SecurityContext,\n"
    "        context: Option<PortContext>,\n"
    "        input: CreateTopicCommandInput,\n"
    "    ) -> ForumResult<TopicResponse> {\n"
    "        self.create_audience\n"
    "            .require(tenant_id, input.category_id, &security, context)\n"
    "            .await?;\n"
    "        let response = self\n"
    "            .inner\n"
    "            .create_command(tenant_id, security, input)\n"
    "            .await?;\n"
    "        require_localized_topic_response(response)\n"
    "    }\n",
)

# Module registration and public API exports.
replace_once(
    "crates/rustok-forum/src/services/mod.rs",
    "mod topic_facade;\n",
    "mod topic_create_audience_authorization;\nmod topic_facade;\n",
)
replace_once(
    "crates/rustok-forum/src/services/mod.rs",
    "pub use topic_facade::TopicService;\n",
    "pub use topic_create_audience_authorization::{\n"
    "    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,\n"
    "};\n"
    "pub use topic_facade::TopicService;\n",
)
replace_once(
    "crates/rustok-forum/src/lib.rs",
    "    ForumTopicReadState, ForumTopicReadStateService, ForumTopicUnreadSummary,\n",
    "    ForumTopicCreateAudienceAuthorization, ForumTopicCreateAudienceAuthorizationService,\n"
    "    ForumTopicReadState, ForumTopicReadStateService, ForumTopicUnreadSummary,\n",
)

# Public crate API documentation.
replace_once(
    "crates/rustok-forum/CRATE_API.md",
    "- `TopicService::create_command(tenant_id, security, CreateTopicCommandInput) -> TopicResponse`\n",
    "- `TopicService::create_command(tenant_id, security, CreateTopicCommandInput) -> TopicResponse`\n"
    "- `TopicService::create_with_audience_context(tenant_id, security, PortContext, CreateTopicInput) -> TopicResponse`\n"
    "- `TopicService::create_command_with_audience_context(tenant_id, security, PortContext, CreateTopicCommandInput) -> TopicResponse`\n"
    "- `TopicService::with_audience_facts(db, event_bus, SharedForumAudienceFactsPort) -> TopicService`\n"
    "- `pub struct ForumTopicCreateAudienceAuthorizationService`, `ForumTopicCreateAudienceAuthorization`\n",
)
replace_once(
    "crates/rustok-forum/CRATE_API.md",
    "- `FORUM-20AQ` publishes persistence only; `TopicService::create`, REST, GraphQL, and facts-provider enforcement remain unchanged.\n"
    "- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs` after changing this boundary.\n"
    "### Category presentation contract\n",
    "- `FORUM-20AQ` publishes normalized persistence; `FORUM-20AR` composes it into every public topic-create owner method.\n"
    "- Categories without a topic-create layer retain historical behavior; local role and explicit-user decisions require no owner facts.\n"
    "- Unresolved trust, Channel, or Groups selectors require an exact caller `PortContext` and an injected `SharedForumAudienceFactsPort`; missing composition fails closed.\n"
    "- Authorization runs before topic, translation, relation, counter, user-stat, and domain-event writes and returns one generic public denial.\n"
    "- GraphQL, REST, and server runtime context/facts composition remain a follow-up after `FORUM-20AR`.\n"
    "- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs` and `node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs` after changing this boundary.\n"
    "### Category presentation contract\n",
)

# Canonical ledger and residual scope.
old_row = "| `FORUM-20` | `in_progress` | FORUM-20A-AQ provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors; FORUM-20AQ adds normalized inherited category topic-create audience persistence. Topic-create enforcement, reply/moderate audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |"
new_row = "| `FORUM-20` | `in_progress` | FORUM-20A-AR provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors; FORUM-20AQ adds normalized inherited category topic-create audience persistence; FORUM-20AR enforces that policy in every topic-create owner path. GraphQL/REST/runtime facts composition, reply/moderate audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |"
replace_once("crates/rustok-forum/docs/implementation-plan.md", old_row, new_row)
aq_block = "### Delivered in `FORUM-20AQ`\n\n- add five normalized Forum-owned tables for a category-local topic-create audience layer plus\n  typed role, channel, group, and explicit allow/deny relations;\n- keep topic-create policy separate from category/topic visibility while inheriting every\n  non-empty root-to-category layer as a conjunction;\n- expose managed inspection and atomic replacement under `forum_categories:manage`, with an\n  empty constraint set clearing only the local layer and restoring inheritance;\n- enforce tenant/category composite ownership, raw relation bounds, immutable stored rows, and\n  PostgreSQL/SQLite parity without changing `TopicService::create` or any transport.\n\n### Compatibility and degraded mode"
ar_block = "### Delivered in `FORUM-20AQ`\n\n- add five normalized Forum-owned tables for a category-local topic-create audience layer plus\n  typed role, channel, group, and explicit allow/deny relations;\n- keep topic-create policy separate from category/topic visibility while inheriting every\n  non-empty root-to-category layer as a conjunction;\n- expose managed inspection and atomic replacement under `forum_categories:manage`, with an\n  empty constraint set clearing only the local layer and restoring inheritance;\n- enforce tenant/category composite ownership, raw relation bounds, immutable stored rows, and\n  PostgreSQL/SQLite parity without changing `TopicService::create` or any transport.\n\n### Delivered in `FORUM-20AR`\n\n- enforce `forum_topics:create` before loading the bounded inherited category topic-create\n  policy and require every root-to-category layer to allow the caller;\n- keep unrestricted categories and locally decidable role/explicit-user layers independent of\n  optional owner facts while preserving explicit-deny precedence;\n- require exact tenant/user `PortContext` plus the optional facts capability only when trust,\n  Channel, or Groups selectors remain unresolved, and fail closed when either is absent;\n- gate every public `TopicService` create path before topic, relation, counter, user-stat, or\n  event writes and publish context-aware owner seams without changing GraphQL or REST DTOs.\n\n### Compatibility and degraded mode"
replace_once("crates/rustok-forum/docs/implementation-plan.md", aq_block, ar_block)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    "- compose topic-create command-time audience enforcement and transports, then add reply and\n  moderation audience policies plus owner write commands;",
    "- compose GraphQL/REST/runtime topic-create audience composition, then add reply and\n  moderation audience policies plus owner write commands;",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    "node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs\n",
    "node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs\n"
    "cargo test -p rustok-forum --test topic_create_audience_enforcement_sqlite -- --nocapture\n"
    "node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs\n",
)

# Cumulative downstream ledger guards.
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '  "FORUM-20A-AQ provide",\n',
    '  "FORUM-20A-AR provide",\n',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '  "### Delivered in `FORUM-20AQ`",\n',
    '  "### Delivered in `FORUM-20AQ`",\n  "### Delivered in `FORUM-20AR`",\n',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AQ.");',
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AR.");',
)
replace_once(
    "scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs",
    '  "FORUM-20A-AQ provide",\n',
    '  "FORUM-20A-AR provide",\n',
)
replace_once(
    "scripts/verify/verify-forum-category-topic-create-audience-policy.mjs",
    '  "FORUM-20A-AQ provide",\n',
    '  "FORUM-20A-AR provide",\n',
)
replace_once(
    "scripts/verify/verify-forum-category-topic-create-audience-policy.mjs",
    '  "### Delivered in `FORUM-20AQ`",\n  "topic-create command-time audience enforcement",\n',
    '  "### Delivered in `FORUM-20AQ`",\n  "### Delivered in `FORUM-20AR`",\n  "GraphQL/REST/runtime topic-create audience composition",\n',
)

for relative in [
    "scripts/agent/apply_forum_20ar.py",
    ".github/workflows/agent-forum-topic-create-audience-enforcement-20ar.yml",
]:
    path = ROOT / relative
    if path.exists():
        path.unlink()
