from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text)


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old[:120]!r}")
    write(path, text.replace(old, new, 1))


def regex_once(path: str, pattern: str, replacement: str) -> None:
    text = read(path)
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE | re.DOTALL)
    if count != 1:
        raise RuntimeError(f"{path}: expected one regex occurrence, found {count}: {pattern!r}")
    write(path, updated)


replace_once(
    "crates/rustok-forum/src/lib.rs",
    "pub mod subscription;\npub mod visibility;",
    "pub mod subscription;\nmod topic_create_transport;\npub mod visibility;",
)

replace_once(
    "crates/rustok-forum/src/graphql/mod.rs",
    "mod read_state;\nmod storefront_read_state;",
    "mod read_state;\nmod runtime_data;\nmod storefront_read_state;",
)
replace_once(
    "crates/rustok-forum/src/graphql/mod.rs",
    "pub use read_state::*;\npub use storefront_read_state::*;",
    "pub use read_state::*;\npub use runtime_data::{ForumGraphqlRuntimeData, attach_schema_data};\npub use storefront_read_state::*;",
)

replace_once(
    "crates/rustok-forum/rustok-module.toml",
    "query = \"graphql::ForumQuery\"\nmutation = \"graphql::ForumMutation\"",
    "query = \"graphql::ForumQuery\"\nmutation = \"graphql::ForumMutation\"\nruntime_data_factory = \"graphql::attach_schema_data\"",
)

replace_once(
    "crates/rustok-forum/src/graphql/mutation.rs",
    "use crate::{\n    CategoryResponse, CategoryService, ReplyService, SubscriptionService, TopicService, VoteService,\n};",
    "use crate::{\n    CategoryResponse, CategoryService, ReplyService, SubscriptionService, TopicService, VoteService,\n};\nuse crate::topic_create_transport::{\n    ForumTopicCreateTransport, topic_create_audience_port_context,\n};\n\nuse super::ForumGraphqlRuntimeData;",
)
replace_once(
    "crates/rustok-forum/src/graphql/mutation.rs",
    """        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let service = TopicService::new(db.clone(), event_bus.clone());
        let topic = service
            .create(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                crate::CreateTopicInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                },
            )
            .await?;""",
    """        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let topic = runtime
            .topic_service(db.clone(), event_bus.clone())
            .create_with_audience_context(
                tenant_id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                audience_context,
                crate::CreateTopicInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                },
            )
            .await?;""",
)

replace_once(
    "crates/rustok-forum/src/graphql/content_commands.rs",
    """use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ForumQuoteReferenceInput,
    ForumQuoteTargetKindInput, ReplyResponse, ReplyService, TopicResponse, TopicService,
    UpdateReplyCommandInput, UpdateTopicCommandInput,
};""",
    """use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ForumQuoteReferenceInput,
    ForumQuoteTargetKindInput, ReplyResponse, ReplyService, TopicResponse, TopicService,
    UpdateReplyCommandInput, UpdateTopicCommandInput,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};""",
)
replace_once(
    "crates/rustok-forum/src/graphql/content_commands.rs",
    "use super::{GqlForumQuoteReferenceInput, GqlForumQuoteTargetKind, GqlForumReply, GqlForumTopic};",
    "use super::{\n    ForumGraphqlRuntimeData, GqlForumQuoteReferenceInput, GqlForumQuoteTargetKind, GqlForumReply,\n    GqlForumTopic,\n};",
)
replace_once(
    "crates/rustok-forum/src/graphql/content_commands.rs",
    """        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let topic = TopicService::new(db.clone(), event_bus.clone())
            .create_command(
                tenant_id,
                security(&auth),
                CreateTopicCommandInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;""",
    """        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = resolve_tenant_scope(tenant, tenant_id)?;
        let audience_context = topic_create_audience_port_context(
            ForumTopicCreateTransport::Graphql,
            tenant_id,
            &auth,
            ctx.data_opt::<rustok_api::RequestContext>(),
            tenant.default_locale.as_str(),
        )?;
        let runtime = ctx
            .data_opt::<ForumGraphqlRuntimeData>()
            .cloned()
            .unwrap_or_default();
        let topic = runtime
            .topic_service(db.clone(), event_bus.clone())
            .create_command_with_audience_context(
                tenant_id,
                security(&auth),
                audience_context,
                CreateTopicCommandInput {
                    locale: input.locale,
                    category_id: input.category_id,
                    title: input.title,
                    slug: input.slug,
                    body: input.body,
                    body_format: input
                        .body_format
                        .unwrap_or_else(|| CONTENT_FORMAT_MARKDOWN.to_string()),
                    content_json: input.content_json,
                    metadata: input.metadata.unwrap_or_else(|| serde_json::json!({})),
                    tags: input.tags,
                    channel_slugs: input.channel_slugs,
                    quotes: map_quotes(input.quotes),
                },
            )
            .await?;""",
)

replace_once(
    "crates/rustok-forum/src/controllers/mod.rs",
    """pub struct ForumHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}""",
    """pub struct ForumHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    audience_facts: Option<crate::SharedForumAudienceFactsPort>,
}""",
)
replace_once(
    "crates/rustok-forum/src/controllers/mod.rs",
    """    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }
}""",
    """    fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn topic_service(&self) -> crate::TopicService {
        match self.audience_facts.clone() {
            Some(facts) => crate::TopicService::with_audience_facts(
                self.db_clone(),
                self.event_bus(),
                facts,
            ),
            None => crate::TopicService::new(self.db_clone(), self.event_bus()),
        }
    }
}""",
)
replace_once(
    "crates/rustok-forum/src/controllers/mod.rs",
    """        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })""",
    """        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
            audience_facts: runtime.shared_get::<crate::SharedForumAudienceFactsPort>(),
        })""",
)

replace_once(
    "crates/rustok-forum/src/controllers/topics.rs",
    """use crate::{
    CreateTopicInput, ListTopicsFilter, ModerationService, SubscriptionService, TopicListItem,
    TopicResponse, TopicService, UpdateTopicInput, VoteService,
};""",
    """use crate::{
    CreateTopicInput, ListTopicsFilter, ModerationService, SubscriptionService, TopicListItem,
    TopicResponse, TopicService, UpdateTopicInput, VoteService,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};""",
)
replace_once(
    "crates/rustok-forum/src/controllers/topics.rs",
    """pub async fn create_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateTopicInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_CREATE],
        "Permission denied: forum_topics:create required",
    )?;

    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .create(tenant.id, forum_security(&auth), input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}""",
    """pub async fn create_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Json(input): Json<CreateTopicInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_CREATE],
        "Permission denied: forum_topics:create required",
    )?;

    let audience_context = topic_create_audience_port_context(
        ForumTopicCreateTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(crate::controllers::map_forum_error)?;
    let topic = runtime
        .topic_service()
        .create_with_audience_context(tenant.id, forum_security(&auth), audience_context, input)
        .await
        .map_err(crate::controllers::map_forum_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}""",
)

replace_once(
    "crates/rustok-forum/src/controllers/content_commands.rs",
    "use rustok_api::{AuthContext, Permission, TenantContext, has_any_effective_permission};",
    "use rustok_api::{\n    AuthContext, Permission, RequestContext, TenantContext, has_any_effective_permission,\n};",
)
replace_once(
    "crates/rustok-forum/src/controllers/content_commands.rs",
    """use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ReplyResponse, ReplyService, TopicResponse,
    TopicService, UpdateReplyCommandInput, UpdateTopicCommandInput,
};""",
    """use crate::{
    CreateReplyCommandInput, CreateTopicCommandInput, ReplyResponse, ReplyService, TopicResponse,
    TopicService, UpdateReplyCommandInput, UpdateTopicCommandInput,
};
use crate::topic_create_transport::{
    ForumTopicCreateTransport, topic_create_audience_port_context,
};""",
)
replace_once(
    "crates/rustok-forum/src/controllers/content_commands.rs",
    """pub async fn create_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateTopicCommandInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_CREATE,
        "Permission denied: forum_topics:create required",
    )?;
    let topic = TopicService::new(runtime.db_clone(), runtime.event_bus())
        .create_command(tenant.id, forum_security(&auth), input)
        .await
        .map_err(command_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}""",
    """pub async fn create_topic(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Json(input): Json<CreateTopicCommandInput>,
) -> HttpResult<(StatusCode, Json<TopicResponse>)> {
    ensure_permission(
        &auth,
        Permission::FORUM_TOPICS_CREATE,
        "Permission denied: forum_topics:create required",
    )?;
    let audience_context = topic_create_audience_port_context(
        ForumTopicCreateTransport::Rest,
        tenant.id,
        &auth,
        Some(&request_context),
        tenant.default_locale.as_str(),
    )
    .map_err(command_error)?;
    let topic = runtime
        .topic_service()
        .create_command_with_audience_context(
            tenant.id,
            forum_security(&auth),
            audience_context,
            input,
        )
        .await
        .map_err(command_error)?;
    Ok((StatusCode::CREATED, Json(topic)))
}""",
)

replace_once(
    "crates/rustok-forum/CRATE_API.md",
    "- `pub struct ForumTopicCreateAudienceAuthorizationService`, `ForumTopicCreateAudienceAuthorization`",
    "- `pub struct ForumTopicCreateAudienceAuthorizationService`, `ForumTopicCreateAudienceAuthorization`\n- `pub struct graphql::ForumGraphqlRuntimeData`; `graphql::attach_schema_data(GraphqlRuntimeInputs)`",
)
replace_once(
    "crates/rustok-forum/CRATE_API.md",
    """- GraphQL, REST, and server runtime context/facts composition remain a follow-up after `FORUM-20AR`.
- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs` and `node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs` after changing this boundary.""",
    """- `FORUM-20AS` composes GraphQL and REST topic-create calls through exact authenticated `PortContext` values and consumes the host-published optional facts port without adding identity to DTOs.
- Both legacy and inline-quote topic-create transports preserve the owner gate before writes; categories with local decisions remain compatible when the provider is absent.
- The Forum manifest publishes `graphql::attach_schema_data`, while the HTTP router consumes the same `SharedForumAudienceFactsPort` from `HostRuntimeContext`.
- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs`, `node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs` after changing this boundary.""",
)

plan_path = "crates/rustok-forum/docs/implementation-plan.md"
plan = read(plan_path)
plan, count = re.subn(
    r"\| `FORUM-20` \| `in_progress` \| FORUM-20A-AR provide.*? \|",
    "| `FORUM-20` | `in_progress` | FORUM-20A-AS provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors; FORUM-20AQ adds normalized inherited category topic-create audience persistence; FORUM-20AR enforces that policy in every topic-create owner path; FORUM-20AS composes exact GraphQL/REST caller context and the host-published Groups facts provider into both create transports. Reply/moderate audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |",
    plan,
    count=1,
)
if count != 1:
    raise RuntimeError(f"{plan_path}: FORUM-20 ledger row replacement count={count}")
anchor = """### Compatibility and degraded mode
"""
section = """### Delivered in `FORUM-20AS`

- compose both legacy and inline-quote GraphQL topic-create mutations through one manifest-backed
  runtime wrapper and the existing context-aware owner methods;
- compose both REST topic-create handlers through `HostRuntimeContext`, using only authenticated
  tenant/user identity plus the middleware-resolved locale and route channel;
- attach read deadline, permission claims, and a bounded correlation id before any optional owner
  facts call, rejecting mismatched request tenant or actor before provider access;
- consume the existing feature-guarded Groups facts publication for both transports while keeping
  provider absence fail closed and adding no topic-create DTO, migration, or Forum-to-Groups dependency.

"""
if anchor not in plan:
    raise RuntimeError(f"{plan_path}: compatibility anchor missing")
plan = plan.replace(anchor, section + anchor, 1)
plan = plan.replace(
    "- compose GraphQL/REST/runtime topic-create audience composition, then add reply and\n  moderation audience policies plus owner write commands;",
    "- provide Forum trust and Channel membership facts adapters, then add reply and moderation\n  audience policies plus owner write commands;",
    1,
)
plan = plan.replace(
    "node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs\n",
    "node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs\n"
    "cargo test -p rustok-forum topic_create_transport -- --nocapture\n"
    "cargo test -p rustok-forum graphql::runtime_data -- --nocapture\n"
    "node scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs\n",
    1,
)
write(plan_path, plan)

replace_once(
    "scripts/verify/verify-forum-topic-create-audience-enforcement.mjs",
    '"FORUM-20A-AR provide",\n  "### Delivered in `FORUM-20AR`",\n  "GraphQL/REST/runtime topic-create audience composition",',
    '"FORUM-20A-AS provide",\n  "### Delivered in `FORUM-20AR`",\n  "### Delivered in `FORUM-20AS`",\n  "Forum trust and Channel membership facts adapters",',
)
replace_once(
    "scripts/verify/verify-forum-category-topic-create-audience-policy.mjs",
    '"FORUM-20A-AR provide",\n  "### Delivered in `FORUM-20AQ`",\n  "### Delivered in `FORUM-20AR`",\n  "GraphQL/REST/runtime topic-create audience composition",',
    '"FORUM-20A-AS provide",\n  "### Delivered in `FORUM-20AQ`",\n  "### Delivered in `FORUM-20AR`",\n  "### Delivered in `FORUM-20AS`",\n  "Forum trust and Channel membership facts adapters",',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '"FORUM-20A-AR provide",',
    '"FORUM-20A-AS provide",',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '"### Delivered in `FORUM-20AR`",\n  "PostgreSQL concurrency",',
    '"### Delivered in `FORUM-20AR`",\n  "### Delivered in `FORUM-20AS`",\n  "PostgreSQL concurrency",',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AR.");',
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AS.");',
)
replace_once(
    "scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs",
    '"FORUM-20A-AR provide",',
    '"FORUM-20A-AS provide",',
)

# Temporary implementation machinery must never remain in the final diff.
(ROOT / "scripts/agent/apply_forum_20as.py").unlink()
(ROOT / ".github/workflows/agent-forum-topic-create-transport-20as.yml").unlink()
