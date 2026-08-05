from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one patch anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


route_path = "crates/rustok-forum/src/services/topic_route.rs"
replace_once(
    route_path,
    "use rustok_content::normalize_locale_code;\n",
    "use rustok_api::PLATFORM_FALLBACK_LOCALE;\nuse rustok_content::normalize_locale_code;\n",
)
replace_once(
    route_path,
    """#[derive(Clone, Debug, Eq, PartialEq)]
struct CurrentTopicRoute {
    topic_id: Uuid,
    slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredTopicRouteAlias {
""",
    """#[derive(Clone, Debug, Eq, PartialEq)]
struct CurrentTopicRoute {
    topic_id: Uuid,
    slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopicTranslationRoute {
    locale: String,
    slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredTopicRouteAlias {
""",
)
replace_once(
    route_path,
    """    pub(crate) async fn record_redirect_alias_in_tx(
""",
    """    /// Records redirects for all source topic translations with a non-empty slug.
    ///
    /// Target locale precedence is exact source locale, platform fallback locale, then the
    /// lexicographically first available target locale. The target slug is intentionally not
    /// stored so resolution always recomputes the current canonical target route.
    pub(crate) async fn record_merge_redirect_aliases_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        source_topic_id: Uuid,
        target_topic_id: Uuid,
        reason: &str,
    ) -> ForumResult<u32> {
        if source_topic_id == target_topic_id {
            return Err(ForumError::TopicRouteResolutionConflict);
        }

        let source_routes =
            load_topic_translation_routes_in_tx(txn, tenant_id, source_topic_id).await?;
        if source_routes.is_empty() {
            return Ok(0);
        }
        let target_routes =
            load_topic_translation_routes_in_tx(txn, tenant_id, target_topic_id).await?;
        let first_target_locale = target_routes
            .first()
            .map(|route| route.locale.as_str())
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum topic merge target must provide at least one localized route when the source owns routes"
                        .to_string(),
                )
            })?;
        let fallback_target_locale = target_routes
            .iter()
            .find(|route| route.locale == PLATFORM_FALLBACK_LOCALE)
            .map(|route| route.locale.as_str());
        let alias_count = u32::try_from(source_routes.len()).map_err(|_| {
            ForumError::Validation(
                "Forum topic merge route alias count exceeds supported range".to_string(),
            )
        })?;

        for source_route in source_routes {
            let target_locale = target_routes
                .iter()
                .find(|target_route| target_route.locale == source_route.locale)
                .map(|target_route| target_route.locale.as_str())
                .or(fallback_target_locale)
                .unwrap_or(first_target_locale);
            Self::record_redirect_alias_in_tx(
                txn,
                tenant_id,
                source_topic_id,
                &source_route.locale,
                &source_route.slug,
                target_topic_id,
                target_locale,
                reason,
            )
            .await?;
        }

        Ok(alias_count)
    }

    pub(crate) async fn record_redirect_alias_in_tx(
""",
)
replace_once(
    route_path,
    """async fn load_current_route_for_topic(
""",
    """async fn load_topic_translation_routes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<TopicTranslationRoute>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT locale, slug
            FROM forum_topic_translations
            WHERE tenant_id = $1
              AND topic_id = $2
              AND slug IS NOT NULL
              AND length(slug) > 0
            ORDER BY locale, id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT locale, slug
            FROM forum_topic_translations
            WHERE tenant_id = ?
              AND topic_id = ?
              AND slug IS NOT NULL
              AND length(slug) > 0
            ORDER BY locale, id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    txn.query_all(statement)
        .await?
        .into_iter()
        .map(topic_translation_route_from_row)
        .collect()
}

fn topic_translation_route_from_row(row: QueryResult) -> ForumResult<TopicTranslationRoute> {
    let locale: String = row.try_get("", "locale")?;
    let slug: String = row.try_get("", "slug")?;
    Ok(TopicTranslationRoute {
        locale: normalize_route_locale(&locale)?,
        slug: normalize_route_slug(&slug)?,
    })
}

async fn load_current_route_for_topic(
""",
)

merge_path = "crates/rustok-forum/src/services/topic_merge.rs"
replace_once(
    merge_path,
    """use super::topic_audience::load_policy_for_topic;
use super::user_stats::UserStatsService;
""",
    """use super::topic_audience::load_policy_for_topic;
use super::topic_route::ForumTopicRouteService;
use super::user_stats::UserStatsService;
""",
)
replace_once(
    merge_path,
    """        source_active.update(&txn).await?;

        let payload = topic_merged_payload(
""",
    """        source_active.update(&txn).await?;

        ForumTopicRouteService::record_merge_redirect_aliases_in_tx(
            &txn,
            tenant_id,
            input.source_topic_id,
            target_topic_id,
            &reason,
        )
        .await?;

        let payload = topic_merged_payload(
""",
)

readme_path = "crates/rustok-forum/docs/README.md"
replace_once(
    readme_path,
    """- FORUM-24A adds `ForumTopicRouteService`, a twelve-hex topic identity, exact-locale canonical descriptors and an append-only redirect/tombstone ledger; host mounting and owner write composition remain follow-up scope.
""",
    """- FORUM-24A adds `ForumTopicRouteService`, a twelve-hex topic identity, exact-locale canonical descriptors and an append-only redirect/tombstone ledger; host mounting and owner write composition remain follow-up scope.
- FORUM-24B composes immutable localized source-route redirects into new topic merges in the same owner transaction without changing merge receipts or events.
""",
)
replace_once(
    readme_path,
    """- [FORUM-24A topic route identity owner](./forum-24a-topic-route-identity-owner.md)
""",
    """- [FORUM-24A topic route identity owner](./forum-24a-topic-route-identity-owner.md)
- [FORUM-24B topic merge route aliases](./forum-24b-topic-merge-route-aliases.md)
""",
)

plan_path = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    plan_path,
    """| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; owner write composition, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |
""",
    """| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; FORUM-24B composes new merge redirects in the owner transaction. Rename/delete composition, historical backfill, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |
""",
)
replace_once(
    plan_path,
    """No command above was run by the implementation agent, per maintainer request.


## `FORUM-25` — full multilingual and RTL contract
""",
    """No command above was run by the implementation agent, per maintainer request.

### Delivered in FORUM-24B

- `ForumTopicMergeService` delegates localized route history to
  `ForumTopicRouteService::record_merge_redirect_aliases_in_tx` before commit;
- every source translation with a non-empty slug receives one immutable redirect
  keyed by its original locale, short identity and slug;
- target locale selection is deterministic: exact source locale, platform
  fallback locale, then the lexicographically first available target locale;
- redirects store target topic plus locale and continue to recompute the latest
  target slug without changing the merge receipt or `forum.topic.merged` event;
- source topics without routes keep existing merge behavior, while a routed
  source fails closed when the target has no canonical localized route;
- exact merge replay returns the existing receipt and does not duplicate aliases.

Topic rename aliases and deletion tombstones remain follow-up owner composition.
Historical merge receipt backfill, storefront mounting and retained runtime
proof also remain.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-merge-route-alias-owner.mjs
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

## `FORUM-25` — full multilingual and RTL contract
""",
)
