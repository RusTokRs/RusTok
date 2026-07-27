from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(relative: str, old: str, new: str) -> None:
    path = ROOT / relative
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{relative}: expected one anchor, found {count}: {old[:100]!r}")
    path.write_text(text.replace(old, new, 1))


replace_once(
    "crates/rustok-forum/src/entities/mod.rs",
    "pub mod forum_category_audience_user;\npub mod forum_category_lifecycle;",
    "pub mod forum_category_audience_user;\n"
    "pub mod forum_category_topic_create_audience_channel;\n"
    "pub mod forum_category_topic_create_audience_group;\n"
    "pub mod forum_category_topic_create_audience_policy;\n"
    "pub mod forum_category_topic_create_audience_role;\n"
    "pub mod forum_category_topic_create_audience_user;\n"
    "pub mod forum_category_lifecycle;",
)
replace_once(
    "crates/rustok-forum/src/entities/mod.rs",
    "pub use forum_category_audience_policy::Entity as ForumCategoryAudiencePolicyEntity;\n",
    "pub use forum_category_audience_policy::Entity as ForumCategoryAudiencePolicyEntity;\n"
    "pub use forum_category_topic_create_audience_policy::Entity as ForumCategoryTopicCreateAudiencePolicyEntity;\n",
)

replace_once(
    "crates/rustok-forum/src/migrations/mod.rs",
    "mod m20260725_000002_add_forum_topic_audience_policy;\n",
    "mod m20260725_000002_add_forum_topic_audience_policy;\n"
    "mod m20260727_000001_add_forum_category_topic_create_audience;\n",
)
replace_once(
    "crates/rustok-forum/src/migrations/mod.rs",
    "        Box::new(m20260725_000002_add_forum_topic_audience_policy::Migration),\n",
    "        Box::new(m20260725_000002_add_forum_topic_audience_policy::Migration),\n"
    "        Box::new(m20260727_000001_add_forum_category_topic_create_audience::Migration),\n",
)

replace_once(
    "crates/rustok-forum/src/services/mod.rs",
    "mod category_audience;\n",
    "mod category_audience;\nmod category_topic_create_audience;\n",
)
replace_once(
    "crates/rustok-forum/src/services/mod.rs",
    "pub use category_audience::{\n"
    "    ForumCategoryAudiencePolicy, ForumCategoryAudiencePolicyLayer,\n"
    "    ForumCategoryAudiencePolicyService, SetForumCategoryAudiencePolicyInput,\n"
    "};\n",
    "pub use category_audience::{\n"
    "    ForumCategoryAudiencePolicy, ForumCategoryAudiencePolicyLayer,\n"
    "    ForumCategoryAudiencePolicyService, SetForumCategoryAudiencePolicyInput,\n"
    "};\n"
    "pub use category_topic_create_audience::{\n"
    "    ForumCategoryTopicCreateAudiencePolicy, ForumCategoryTopicCreateAudiencePolicyLayer,\n"
    "    ForumCategoryTopicCreateAudiencePolicyService,\n"
    "    SetForumCategoryTopicCreateAudiencePolicyInput,\n"
    "};\n",
)

replace_once(
    "crates/rustok-forum/src/lib.rs",
    "    ForumCategoryAudiencePolicyService, ForumCategoryVisibilityPolicy,\n",
    "    ForumCategoryAudiencePolicyService, ForumCategoryTopicCreateAudiencePolicy,\n"
    "    ForumCategoryTopicCreateAudiencePolicyLayer,\n"
    "    ForumCategoryTopicCreateAudiencePolicyService, ForumCategoryVisibilityPolicy,\n",
)
replace_once(
    "crates/rustok-forum/src/lib.rs",
    "    SetForumCategoryAudiencePolicyInput, SetForumCategoryVisibilityPolicyInput,\n",
    "    SetForumCategoryAudiencePolicyInput,\n"
    "    SetForumCategoryTopicCreateAudiencePolicyInput, SetForumCategoryVisibilityPolicyInput,\n",
)

replace_once(
    "crates/rustok-forum/src/services/category_audience.rs",
    "async fn load_category_ancestor_ids<C>(\n",
    "pub(crate) async fn load_category_ancestor_ids<C>(\n",
)

replace_once(
    "crates/rustok-forum/CRATE_API.md",
    "- `CategoryService::set_topic_policy(tenant_id, category_id, security, UpdateCategoryTopicPolicyInput) -> CategoryTopicPolicyResponse`\n",
    "- `CategoryService::set_topic_policy(tenant_id, category_id, security, UpdateCategoryTopicPolicyInput) -> CategoryTopicPolicyResponse`\n"
    "- `pub struct ForumCategoryTopicCreateAudiencePolicyService`\n"
    "- `ForumCategoryTopicCreateAudiencePolicyService::get(tenant_id, category_id, security) -> ForumCategoryTopicCreateAudiencePolicy`\n"
    "- `ForumCategoryTopicCreateAudiencePolicyService::set(tenant_id, category_id, security, SetForumCategoryTopicCreateAudiencePolicyInput) -> ForumCategoryTopicCreateAudiencePolicy`\n",
)
replace_once(
    "crates/rustok-forum/CRATE_API.md",
    "- Existing topics remain unchanged when a category policy is disabled; the policy controls new topic placement only.\n"
    "### Category presentation contract\n",
    "- Existing topics remain unchanged when a category policy is disabled; the policy controls new topic placement only.\n"
    "### Category topic-create audience policy\n"
    "- `ForumCategoryTopicCreateAudiencePolicyService` stores a separate normalized topic-create audience rule; it does not mutate content visibility.\n"
    "- Effective topic-create audience is the root-to-category conjunction of every non-empty local layer.\n"
    "- Managed `get` and atomic replacement `set` require `forum_categories:manage`; empty constraints restore inheritance.\n"
    "- PostgreSQL and SQLite enforce tenant/category ownership, typed relations, immutable rows, and bounded direct channel/group/user inserts.\n"
    "- `FORUM-20AQ` publishes persistence only; `TopicService::create`, REST, GraphQL, and facts-provider enforcement remain unchanged.\n"
    "- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs` after changing this boundary.\n"
    "### Category presentation contract\n",
)

old_row = "| `FORUM-20` | `in_progress` | FORUM-20A-AP provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors behind exact recipient capability. Write audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |"
new_row = "| `FORUM-20` | `in_progress` | FORUM-20A-AQ provide inherited and richer category/topic visibility, recipient-aware Forum notification authorization, the Notifications inbox/group owner plane, authenticated storefront ports, native and GraphQL read/open/write transport parity, grouped UI and navigation. FORUM-20AM synchronizes the ledgers; FORUM-20AN adds GraphQL group-state commands; FORUM-20AO adds auth-reactive grouped bootstrap refresh; FORUM-20AP materializes initially non-public topic-created descriptors; FORUM-20AQ adds normalized inherited category topic-create audience persistence. Topic-create enforcement, reply/moderate audiences, remaining trust/channel facts, search/index/SEO/deep-link migration, scheduled reconciliation/redaction, delivery transports and PostgreSQL cross-consumer evidence remain. |"
replace_once("crates/rustok-forum/docs/implementation-plan.md", old_row, new_row)

ap_block = "### Delivered in `FORUM-20AP`\n\n- materialize `forum.topic.created` descriptors for active topics that are already non-public\n  when the host publishes the exact notification recipient-context capability;\n- keep public-only descriptor creation when that capability is absent, preserving the existing\n  fail-closed optional-module profile;\n- expose only topic/category identifiers in the descriptor and defer all recipient authority to\n  the bounded subscription audience recheck;\n- preserve author exclusion, raw subscription cursor progress, current category/topic audience\n  evaluation, non-oracular inactive targets, and later target-open authorization.\n\n### Compatibility and degraded mode"
aq_block = "### Delivered in `FORUM-20AP`\n\n- materialize `forum.topic.created` descriptors for active topics that are already non-public\n  when the host publishes the exact notification recipient-context capability;\n- keep public-only descriptor creation when that capability is absent, preserving the existing\n  fail-closed optional-module profile;\n- expose only topic/category identifiers in the descriptor and defer all recipient authority to\n  the bounded subscription audience recheck;\n- preserve author exclusion, raw subscription cursor progress, current category/topic audience\n  evaluation, non-oracular inactive targets, and later target-open authorization.\n\n### Delivered in `FORUM-20AQ`\n\n- add five normalized Forum-owned tables for a category-local topic-create audience layer plus\n  typed role, channel, group, and explicit allow/deny relations;\n- keep topic-create policy separate from category/topic visibility while inheriting every\n  non-empty root-to-category layer as a conjunction;\n- expose managed inspection and atomic replacement under `forum_categories:manage`, with an\n  empty constraint set clearing only the local layer and restoring inheritance;\n- enforce tenant/category composite ownership, raw relation bounds, immutable stored rows, and\n  PostgreSQL/SQLite parity without changing `TopicService::create` or any transport.\n\n### Compatibility and degraded mode"
replace_once("crates/rustok-forum/docs/implementation-plan.md", ap_block, aq_block)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    "- add create/reply/moderate audience policies and owner write commands;",
    "- compose topic-create command-time audience enforcement and transports, then add reply and\n  moderation audience policies plus owner write commands;",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    "node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs\n",
    "node scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs\n"
    "node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs\n",
)

replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '  "FORUM-20A-AP provide",\n',
    '  "FORUM-20A-AQ provide",\n',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    '  "### Delivered in `FORUM-20AP`",\n',
    '  "### Delivered in `FORUM-20AP`",\n  "### Delivered in `FORUM-20AQ`",\n',
)
replace_once(
    "scripts/verify/verify-forum-notification-plan-sync.mjs",
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AP.");',
    'console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AQ.");',
)
replace_once(
    "scripts/verify/verify-forum-notification-topic-descriptor-materialization.mjs",
    '  "FORUM-20A-AP provide",\n',
    '  "FORUM-20A-AQ provide",\n',
)

for relative in [
    "scripts/agent/apply_forum_20aq.py",
    ".github/workflows/agent-forum-category-topic-create-audience-20aq.yml",
]:
    path = ROOT / relative
    if path.exists():
        path.unlink()
