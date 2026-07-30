from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one replacement, found {count}\n{old}")
    target.write_text(text.replace(old, new, 1))


def replace_after(path: str, anchor: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    anchor_index = text.find(anchor)
    if anchor_index < 0:
        raise SystemExit(f"{path}: anchor not found: {anchor}")
    before = text[:anchor_index]
    after = text[anchor_index:]
    count = after.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one replacement after anchor, found {count}\n{old}"
        )
    target.write_text(before + after.replace(old, new, 1))


# Ordinary GraphQL storefront Search.
replace_once(
    "crates/rustok-search/src/graphql/query.rs",
    """    SearchModule, SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n    SearchSuggestionQuery, SearchSuggestionService,\n""",
    """    SearchModule, SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n    SearchSuggestionQuery, SearchSuggestionService, resolve_trusted_storefront_channel,\n""",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search(\n",
    """        let input = normalize_search_preview_input(input)?;\n        enforce_storefront_rate_limit(ctx, policy.surface).await?;\n""",
    """        let input = normalize_search_preview_input(input)?;\n        let request_context = ctx.data::<RequestContext>()?;\n        let trusted_channel = resolve_trusted_storefront_channel(\n            request_context,\n            tenant.id,\n            input.channel_id,\n        )\n        .map_err(|error| FieldError::new(error.to_string()))?;\n        enforce_storefront_rate_limit(ctx, policy.surface).await?;\n""",
)
replace_after(
    "crates/rustok-search/src/graphql/query.rs",
    "    async fn storefront_search(\n",
    "            channel_id: input.channel_id,\n",
    "            channel_id: trusted_channel.channel_id,\n",
)

# Ordinary native storefront Search.
replace_once(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "        use rustok_api::{HostRuntimeContext, TenantContext};\n",
    "        use rustok_api::{HostRuntimeContext, RequestContext, TenantContext};\n",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    """            PgSearchEngine, SearchDictionaryService, SearchEngine, SearchFilterPresetService,\n            SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n""",
    """            PgSearchEngine, SearchDictionaryService, SearchEngine, SearchFilterPresetService,\n            SearchQuery, SearchQueryLogRecord, SearchRankingProfile, SearchSettingsService,\n            resolve_trusted_storefront_channel,\n""",
)
replace_once(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    """        let tenant = leptos_axum::extract::<TenantContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let input = normalize_search_input(query, locale, preset_key, filters)?;\n        let started_at = Instant::now();\n""",
    """        let tenant = leptos_axum::extract::<TenantContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let request_context = leptos_axum::extract::<RequestContext>()\n            .await\n            .map_err(ServerFnError::new)?;\n        let input = normalize_search_input(query, locale, preset_key, filters)?;\n        let trusted_channel = resolve_trusted_storefront_channel(\n            &request_context,\n            tenant.id,\n            input.channel_id,\n        )\n        .map_err(|error| ServerFnError::new(error.to_string()))?;\n        let started_at = Instant::now();\n""",
)
replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_native(\n",
    "            channel_id: input.channel_id,\n",
    "            channel_id: trusted_channel.channel_id,\n",
)

# Shared Forum-only execution owner revalidates every transport.
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    StorefrontSearchResultEligibilityRequest, StorefrontSearchTransport,\n    resolve_storefront_search_category_ids, resolve_storefront_search_result_candidates,\n""",
    """    StorefrontSearchResultEligibilityRequest, StorefrontSearchTransport,\n    resolve_storefront_search_category_ids, resolve_storefront_search_result_candidates,\n    resolve_trusted_storefront_channel,\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    """    let query = normalize_query(&request.query)?;\n    let locale = normalize_locale(request.locale.as_deref())?;\n    let fallback_locale = normalize_required_locale(&request.fallback_locale)?;\n    let source_modules = normalize_filter_values("source_modules", request.source_modules)?;\n""",
    """    let query = normalize_query(&request.query)?;\n    let locale = normalize_locale(request.locale.as_deref())?;\n    let fallback_locale = normalize_required_locale(&request.fallback_locale)?;\n    let requested_channel_id = parse_optional_uuid("channel_id", request.channel_id.as_deref())?;\n    let request_context = request.request_context.ok_or_else(|| {\n        ForumStorefrontSearchExecutionError::Validation(\n            "Forum storefront Search requires trusted request context".to_string(),\n        )\n    })?;\n    let trusted_channel = resolve_trusted_storefront_channel(\n        &request_context,\n        request.tenant_id,\n        requested_channel_id,\n    )\n    .map_err(|error| ForumStorefrontSearchExecutionError::Validation(error.to_string()))?;\n    let source_modules = normalize_filter_values("source_modules", request.source_modules)?;\n""",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "        channel_id: parse_optional_uuid(\"channel_id\", request.channel_id.as_deref())?,\n",
    "        channel_id: trusted_channel.channel_id,\n",
)
replace_once(
    "crates/rustok-search/src/forum_storefront_execution.rs",
    "        request_context: request.request_context,\n",
    "        request_context: Some(request_context),\n",
)

# Search implementation plan.
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    """Periodic release verification found two unresolved runtime boundaries. Public\nGraphQL and native storefront Search accept `channel_id` from caller input rather\nthan deriving it from trusted `RequestContext`, while `PgSearchEngine` applies the\nchannel only to attribute filters, facets, and sorting—not to the ranked product\nresult set. Product search documents also omit the canonical\n`metadata.channel_visibility.allowed_channel_slugs` projection, so an active\nproduct can remain searchable in a channel where the Commerce storefront would\nhide it. The Forum-only eligibility adapter now reuses the trusted route-channel\nslug for Forum decisions, but one cross-result channel predicate is still open.\n""",
    """`FORUM-23B2E1` closes the transport half of storefront channel authority.\nGraphQL and native storefront Search now derive channel ID and slug from trusted\n`RequestContext`; caller-provided `channel_id` is now only a compatibility assertion\nand a mismatched value fails closed. The same owner is used by ordinary and\nForum-only Search, and the shared Forum execution path revalidates tenant and\nchannel context. Product channel visibility remains blocked: `PgSearchEngine`\nstill applies channel only to attribute filters, facets, and sorting, while product\nSearch documents omit the canonical\n`metadata.channel_visibility.allowed_channel_slugs` projection.\n""",
)
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    """- Forum result eligibility contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-result-eligibility.json` and\n  `scripts/verify/verify-forum-search-result-eligibility.mjs`.\n""",
    """- Forum result eligibility contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-result-eligibility.json` and\n  `scripts/verify/verify-forum-search-result-eligibility.mjs`.\n- Trusted storefront channel authority status:\n  `source_complete_execution_pending` under `FORUM-23B2E1`.\n- Trusted channel contract and guardrail:\n  `crates/rustok-forum/contracts/forum-search-trusted-channel-authority.json` and\n  `scripts/verify/verify-forum-search-trusted-channel-authority.mjs`.\n""",
)
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    "- Storefront channel authority and product visibility remain `blocked`.\n",
    """- Trusted storefront channel authority is `source_complete_execution_pending`\n  under `FORUM-23B2E1`.\n- Product channel visibility remains blocked.\n""",
)
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    """17. Added the neutral Forum result-eligibility port, Forum exact topic/reply owner,\n    host adapter, bounded 100-row candidate scan, and post-authorization totals,\n    facets, offset, and limit under `FORUM-23B2D`.\n""",
    """17. Added the neutral Forum result-eligibility port, Forum exact topic/reply owner,\n    host adapter, bounded 100-row candidate scan, and post-authorization totals,\n    facets, offset, and limit under `FORUM-23B2D`.\n18. Added the Search-owned trusted storefront channel authority and bound ordinary\n    plus Forum-only GraphQL/native Search to middleware `RequestContext`; the\n    legacy public `channel_id` is assertion-only under `FORUM-23B2E1`.\n""",
)
replace_once(
    "crates/rustok-search/docs/implementation-plan.md",
    """1. **Close storefront channel authority and visibility.** Derive channel identity\n   from trusted `RequestContext` for GraphQL and native storefront surfaces,\n   denormalize canonical product channel visibility into Search-owned documents,\n   backfill existing documents safely, and make base results, totals, facets,\n   typo fallback, and attribute operations use one fail-closed channel predicate.\n   **Done when:** caller-supplied channel IDs cannot select another channel and a\n   restricted product is absent from every Search response outside its allowed\n   channel.\n""",
    """1. **Close Product channel visibility projection and predicate.** Reuse the\n   trusted `RequestContext` authority delivered by `FORUM-23B2E1`, denormalize\n   canonical product channel visibility into Search-owned documents, backfill\n   existing documents safely, and make base results, totals, facets, typo fallback,\n   suggestions, query rules, and attribute operations use one fail-closed channel\n   predicate. **Done when:** a restricted product is absent from every Search\n   response outside its allowed channel.\n""",
)

# Canonical Forum plan.
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination. Trusted channel authority, remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |\n""",
    """| `FORUM-23` | `in_progress` | FORUM-23A through FORUM-23A11 harden public-author Search projections and durable privacy invalidation; FORUM-23B1 adds exact Forum category filtering; FORUM-23B2A publishes a bounded Forum-owned public/authenticated category-subtree scope; FORUM-23B2B applies the complete delivered richer category audience decision before subtree IDs leave Forum; FORUM-23B2C composes that scope into explicit GraphQL and native Forum-only storefront Search execution; FORUM-23B2D applies exact topic-local and approved-reply result eligibility before visible Search totals, facets and pagination; FORUM-23B2E1 binds storefront channel selection to trusted `RequestContext`. Product channel projection/predicates, remaining filters, owner revision ordering/reconciliation and maintainer runtime evidence remain. |\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """- `forum-search-result-eligibility.json`,\n  `forum-23b2d-search-result-eligibility.md`, and\n  `verify-forum-search-result-eligibility.mjs` lock the owner, bound, transport and\n  post-authorization pagination contract while recording execution as pending.\n\n### Compatibility and degraded mode\n""",
    """- `forum-search-result-eligibility.json`,\n  `forum-23b2d-search-result-eligibility.md`, and\n  `verify-forum-search-result-eligibility.mjs` lock the owner, bound, transport and\n  post-authorization pagination contract while recording execution as pending.\n\n### Delivered in `FORUM-23B2E1`\n\n- `TrustedStorefrontChannel` is a Search-owned neutral authority derived from the\n  middleware `RequestContext`; it requires the exact Search tenant and a complete\n  channel ID/slug pair or an explicitly unscoped request;\n- the public `channel_id` input remains for compatibility but is assertion-only:\n  absent input uses trusted context, an exact match is accepted, and malformed or\n  mismatched input fails closed instead of selecting another channel;\n- ordinary and Forum-only GraphQL/native Search use the same authority, while the\n  shared Forum execution owner revalidates the context for future transports;\n- admin preview and admin global Search keep their existing operator-selected\n  channel behavior;\n- `forum-search-trusted-channel-authority.json`,\n  `forum-23b2e1-trusted-channel-authority.md`, and\n  `verify-forum-search-trusted-channel-authority.mjs` lock the source boundary and\n  record maintainer execution as pending.\n\n### Compatibility and degraded mode\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """No migration, backfill, Search query shape, Forum projection shape, dependency,\npublic DTO or `Cargo.lock` change is required by `FORUM-23B2A/B2B/B2C/B2D`.\n""",
    """No migration, backfill, Search query shape, Forum projection shape, dependency,\npublic DTO or `Cargo.lock` change is required by\n`FORUM-23B2A/B2B/B2C/B2D/B2E1`.\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """Product\ncategory identifiers are never expanded or filtered through Forum policy.\n""",
    """Product\ncategory identifiers are never expanded or filtered through Forum policy. The\nlegacy storefront `channel_id` remains accepted only when it matches the trusted\nrequest channel; missing, incomplete, foreign-tenant or mismatched trusted context\nfails closed. Product channel allowlist projection and base-result filtering remain\nopen and are not claimed by `FORUM-23B2E1`.\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """- derive trusted channel authority consistently for every storefront Search\n  result predicate, especially Product visibility;\n""",
    """- project and backfill canonical Product channel allowlists, then apply the\n  trusted `RequestContext` channel consistently to base results, totals, facets,\n  typo fallback, suggestions, query rules and attribute operations;\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\n""",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\ncargo test -p rustok-search storefront_channel_authority -- --nocapture\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\ncargo check -p rustok-search --features graphql --all-targets\n""",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\nnode scripts/verify/verify-forum-search-trusted-channel-authority.mjs\ncargo check -p rustok-search --features graphql --all-targets\n""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """The `FORUM-23B2A/B2B/B2C/B2D` source and contract records do not claim successful\nruntime verification until the maintainer runs the commands above.\n""",
    """The `FORUM-23B2A/B2B/B2C/B2D/B2E1` source and contract records do not claim\nsuccessful runtime verification until the maintainer runs the commands above.\n""",
)

print("FORUM-23B2E1 trusted channel patch applied")
