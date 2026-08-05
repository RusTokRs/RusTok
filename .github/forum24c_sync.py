from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"expected one anchor in {path}: {old[:80]!r}")
    file.write_text(text.replace(old, new, 1))


route_path = "crates/rustok-forum/src/services/topic_route.rs"
route_anchor = '''        Ok(alias_count)
    }

    pub(crate) async fn record_redirect_alias_in_tx(
'''
route_replacement = '''        Ok(alias_count)
    }

    /// Records immutable gone routes for every localized slug owned by a topic.
    ///
    /// Existing redirects are preserved so lifecycle cleanup of an archived merge source cannot
    /// downgrade its canonical history. Exact gone rows are idempotent; any ownership or payload
    /// drift fails closed.
    pub(crate) async fn record_delete_tombstones_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        reason: &str,
    ) -> ForumResult<u32> {
        let reason = normalize_alias_reason(reason)?;
        let routes = load_topic_translation_routes_in_tx(txn, tenant_id, topic_id).await?;
        let short_id = Self::short_identity(topic_id);
        let mut inserted = 0_u32;

        for route in routes {
            let aliases =
                load_route_aliases(txn, tenant_id, &route.locale, &short_id, &route.slug).await?;
            match aliases.as_slice() {
                [] => {
                    Self::record_gone_alias_in_tx(
                        txn,
                        tenant_id,
                        topic_id,
                        &route.locale,
                        &route.slug,
                        &reason,
                    )
                    .await?;
                    inserted = inserted.checked_add(1).ok_or_else(|| {
                        ForumError::Validation(
                            "Forum topic delete route tombstone count overflow".to_string(),
                        )
                    })?;
                }
                [alias] if alias.topic_id == topic_id => {
                    if alias.disposition == StoredRouteDisposition::Redirect {
                        let target_topic_id = alias
                            .target_topic_id
                            .filter(|target_topic_id| !target_topic_id.is_nil())
                            .ok_or(ForumError::TopicRouteResolutionConflict)?;
                        let target_locale = alias
                            .target_locale
                            .as_deref()
                            .ok_or(ForumError::TopicRouteResolutionConflict)?;
                        let _ = target_topic_id;
                        normalize_route_locale(target_locale)?;
                    }
                    match alias.disposition {
                        StoredRouteDisposition::Redirect => {}
                        StoredRouteDisposition::Gone
                            if alias.target_topic_id.is_none()
                                && alias.target_locale.is_none()
                                && alias.reason == reason => {}
                        StoredRouteDisposition::Gone => {
                            return Err(ForumError::TopicRouteResolutionConflict);
                        }
                    }
                }
                _ => return Err(ForumError::TopicRouteResolutionConflict),
            }
        }

        Ok(inserted)
    }

    pub(crate) async fn record_redirect_alias_in_tx(
'''
replace_once(route_path, route_anchor, route_replacement)

owner_path = "crates/rustok-forum/src/services/topic_owner.rs"
replace_once(
    owner_path,
    '''use super::topic;
use super::user_stats::UserStatsService;

/// Public owner service for topic commands.
''',
    '''use super::topic;
use super::topic_route::ForumTopicRouteService;
use super::user_stats::UserStatsService;

const FORUM_TOPIC_DELETED_ROUTE_REASON: &str = "Topic deleted";

/// Public owner service for topic commands.
''',
)
replace_once(
    owner_path,
    '''        delete_attached_localized_values(&txn, tenant_id, "topic", topic_id)
            .await
            .map_err(map_flex_cleanup_error)?;
''',
    '''        ForumTopicRouteService::record_delete_tombstones_in_tx(
            &txn,
            tenant_id,
            topic_id,
            FORUM_TOPIC_DELETED_ROUTE_REASON,
        )
        .await?;

        delete_attached_localized_values(&txn, tenant_id, "topic", topic_id)
            .await
            .map_err(map_flex_cleanup_error)?;
''',
)

readme_path = "crates/rustok-forum/docs/README.md"
replace_once(
    readme_path,
    '''- FORUM-24B composes immutable localized source-route redirects into new topic merges in the same owner transaction without changing merge receipts or events.
''',
    '''- FORUM-24B composes immutable localized source-route redirects into new topic merges in the same owner transaction without changing merge receipts or events.
- FORUM-24C records immutable localized `gone` routes in the topic delete transaction while preserving existing merge redirects.
''',
)
replace_once(
    readme_path,
    '''- [FORUM-24B topic merge route aliases](./forum-24b-topic-merge-route-aliases.md)
''',
    '''- [FORUM-24B topic merge route aliases](./forum-24b-topic-merge-route-aliases.md)
- [FORUM-24C topic delete route tombstones](./forum-24c-topic-delete-route-tombstones.md)
''',
)

plan_path = "crates/rustok-forum/docs/implementation-plan.md"
replace_once(
    plan_path,
    '''| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; FORUM-24B composes new merge redirects in the owner transaction. Rename/delete composition, historical backfill, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |
''',
    '''| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; FORUM-24B composes new merge redirects and FORUM-24C composes delete tombstones in their owner transactions. Rename composition, historical backfill, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |
''',
)
replace_once(
    plan_path,
    '''Topic rename aliases and deletion tombstones remain follow-up owner composition.
Historical merge receipt backfill, storefront mounting and retained runtime
proof also remain.
''',
    '''Topic rename aliases remain follow-up owner composition. Historical merge receipt
backfill, storefront mounting and retained runtime proof also remain.
''',
)
replace_once(
    plan_path,
    '''No command above was run by the implementation agent, per maintainer request.

## `FORUM-25` — full multilingual and RTL contract
''',
    '''No command above was run by the implementation agent, per maintainer request.

### Delivered in FORUM-24C

- `TopicService::delete` delegates to
  `ForumTopicRouteService::record_delete_tombstones_in_tx` before localized
  cleanup and soft-delete mutation;
- every topic translation with a non-empty slug receives one immutable `gone`
  route with no target topic or locale and the stable reason `Topic deleted`;
- the tombstones commit with the existing delete lifecycle, counters, events and
  projection invalidation without changing public command or event schemas;
- an existing redirect for the same topic and route is preserved, so deleting an
  archived merge source cannot downgrade FORUM-24B canonical history;
- exact existing `gone` rows are idempotent and ownership, target-field or reason
  drift fails closed.

Topic rename aliases, historical backfill, storefront mounting, category routes,
hreflang/SEO policy and retained runtime proof remain.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-delete-route-tombstone-owner.mjs
cargo test -p rustok-forum --test topic_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

## `FORUM-25` — full multilingual and RTL contract
''',
)
