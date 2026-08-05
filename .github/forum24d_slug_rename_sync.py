from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


route_path = Path("crates/rustok-forum/src/services/topic_route.rs")
route = route_path.read_text()
route = replace_once(
    route,
    "pub const MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN: usize = 500;\n",
    "pub const MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN: usize = 500;\n"
    "pub const FORUM_TOPIC_RENAMED_ROUTE_REASON: &str = \"Topic slug changed\";\n",
    "route rename reason",
)
route = replace_once(
    route,
    '''#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoredRouteDisposition {
''',
    '''#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameForumTopicSlugInput {
    pub locale: String,
    pub slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumTopicSlugRenameResult {
    pub topic_id: Uuid,
    pub locale: String,
    pub previous_slug: String,
    pub slug: String,
    pub previous_path: String,
    pub canonical: ForumTopicRouteDescriptor,
    pub alias_id: Option<Uuid>,
    pub changed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoredRouteDisposition {
''',
    "rename DTOs",
)
route = replace_once(
    route,
    '''    pub async fn resolve(
''',
    '''    async fn canonical_descriptor_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        requested_topic_id: Uuid,
        preferred_locale: &str,
    ) -> ForumResult<ForumTopicRouteDescriptor> {
        let preferred_locale = normalize_route_locale(preferred_locale)?;
        let canonical = ForumTopicCanonicalResolutionService::new(self.db.clone())
            .resolve_unchecked(tenant_id, requested_topic_id)
            .await?;
        let routes = load_current_topic_translation_routes(
            &self.db,
            tenant_id,
            canonical.canonical_topic_id,
        )
        .await?;
        let route = routes
            .iter()
            .find(|route| route.locale == preferred_locale)
            .or_else(|| {
                routes
                    .iter()
                    .find(|route| route.locale == PLATFORM_FALLBACK_LOCALE)
            })
            .or_else(|| routes.first())
            .ok_or(ForumError::TopicRouteNotFound)?;
        let short_id = Self::short_identity(canonical.canonical_topic_id);
        ensure_unambiguous_current_short_id(
            &self.db,
            tenant_id,
            &route.locale,
            &short_id,
            canonical.canonical_topic_id,
        )
        .await?;
        Ok(ForumTopicRouteDescriptor {
            topic_id: canonical.canonical_topic_id,
            locale: route.locale.clone(),
            short_id: short_id.clone(),
            slug: route.slug.clone(),
            path: forum_topic_route_path(&route.locale, &short_id, &route.slug),
        })
    }

    pub async fn resolve(
''',
    "canonical fallback helper",
)
route = replace_once(
    route,
    '''                let canonical = self
                    .canonical_descriptor(tenant_id, route.topic_id, &locale)
                    .await?;
''',
    '''                let canonical = self
                    .canonical_descriptor_with_locale_fallback(
                        tenant_id,
                        route.topic_id,
                        &locale,
                    )
                    .await?;
''',
    "current route fallback resolution",
)
route = replace_once(
    route,
    '''                let canonical = self
                    .canonical_descriptor(tenant_id, target_topic_id, target_locale)
                    .await?;
                Ok(ForumTopicRouteResolution {
                    requested_locale: locale,
                    requested_short_id: short_id,
                    requested_slug: slug,
                    requested_topic_id: Some(alias.topic_id),
                    disposition: ForumTopicRouteDisposition::Redirect,
                    canonical: Some(canonical),
                    alias_id: Some(alias.alias_id),
                })
''',
    '''                let canonical = if target_topic_id == alias.topic_id {
                    match self
                        .canonical_descriptor_with_locale_fallback(
                            tenant_id,
                            target_topic_id,
                            target_locale,
                        )
                        .await
                    {
                        Ok(canonical) => canonical,
                        Err(ForumError::TopicNotFound(_))
                        | Err(ForumError::TopicDeleted)
                        | Err(ForumError::TopicRouteNotFound) => {
                            return Ok(ForumTopicRouteResolution {
                                requested_locale: locale,
                                requested_short_id: short_id,
                                requested_slug: slug,
                                requested_topic_id: Some(alias.topic_id),
                                disposition: ForumTopicRouteDisposition::Gone,
                                canonical: None,
                                alias_id: Some(alias.alias_id),
                            });
                        }
                        Err(error) => return Err(error),
                    }
                } else {
                    self.canonical_descriptor(tenant_id, target_topic_id, target_locale)
                        .await?
                };
                Ok(ForumTopicRouteResolution {
                    requested_locale: locale,
                    requested_short_id: short_id,
                    requested_slug: slug,
                    requested_topic_id: Some(alias.topic_id),
                    disposition: ForumTopicRouteDisposition::Redirect,
                    canonical: Some(canonical),
                    alias_id: Some(alias.alias_id),
                })
''',
    "self redirect lifecycle resolution",
)
route = replace_once(
    route,
    '''    /// Records redirects for all source topic translations with a non-empty slug.
''',
    '''    /// Renames one existing localized route and records its previous path atomically.
    pub(crate) async fn rename_topic_slug_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        input: &RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        let locale = normalize_route_locale(&input.locale)?;
        let slug = normalize_route_slug_for_write(&input.slug)?;
        let current = lock_topic_route_for_rename_in_tx(txn, tenant_id, topic_id, &locale).await?;
        if current.topic_id != topic_id {
            return Err(ForumError::TopicRouteResolutionConflict);
        }
        let previous_slug = normalize_route_slug(&current.slug)?;
        let short_id = Self::short_identity(topic_id);
        ensure_unambiguous_current_short_id(
            txn,
            tenant_id,
            &locale,
            &short_id,
            topic_id,
        )
        .await?;
        let previous_path = forum_topic_route_path(&locale, &short_id, &previous_slug);
        let canonical = ForumTopicRouteDescriptor {
            topic_id,
            locale: locale.clone(),
            short_id: short_id.clone(),
            slug: slug.clone(),
            path: forum_topic_route_path(&locale, &short_id, &slug),
        };
        if previous_slug == slug {
            return Ok(ForumTopicSlugRenameResult {
                topic_id,
                locale,
                previous_slug,
                slug,
                previous_path,
                canonical,
                alias_id: None,
                changed: false,
            });
        }

        let alias_id = Self::record_redirect_alias_in_tx(
            txn,
            tenant_id,
            topic_id,
            &locale,
            &previous_slug,
            topic_id,
            &locale,
            FORUM_TOPIC_RENAMED_ROUTE_REASON,
        )
        .await?;
        update_topic_route_slug_in_tx(txn, tenant_id, topic_id, &locale, &slug).await?;

        Ok(ForumTopicSlugRenameResult {
            topic_id,
            locale,
            previous_slug,
            slug,
            previous_path,
            canonical,
            alias_id: Some(alias_id),
            changed: true,
        })
    }

    /// Records redirects for all source topic translations with a non-empty slug.
''',
    "rename transaction helper",
)
route = replace_once(
    route,
    '''fn normalize_alias_reason(reason: &str) -> ForumResult<String> {
''',
    '''fn normalize_route_slug_for_write(value: &str) -> ForumResult<String> {
    normalize_route_slug(value).map_err(|error| match error {
        ForumError::TopicRouteNotFound => ForumError::Validation(
            "Forum topic route slug must contain a valid route segment".to_string(),
        ),
        other => other,
    })
}

fn normalize_alias_reason(reason: &str) -> ForumResult<String> {
''',
    "write slug validation",
)
route = replace_once(
    route,
    '''async fn load_current_route_for_topic(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
) -> ForumResult<Option<CurrentTopicRoute>> {
''',
    '''async fn load_current_topic_translation_routes<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
) -> ForumResult<Vec<TopicTranslationRoute>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT translation.locale, translation.slug
            FROM forum_topics topic
            JOIN forum_topic_translations translation
              ON translation.tenant_id = topic.tenant_id
             AND translation.topic_id = topic.id
            WHERE topic.tenant_id = $1
              AND topic.id = $2
              AND topic.deleted_at IS NULL
              AND translation.slug IS NOT NULL
              AND length(translation.slug) > 0
            ORDER BY translation.locale, translation.id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT translation.locale, translation.slug
            FROM forum_topics topic
            JOIN forum_topic_translations translation
              ON translation.tenant_id = topic.tenant_id
             AND translation.topic_id = topic.id
            WHERE topic.tenant_id = ?
              AND topic.id = ?
              AND topic.deleted_at IS NULL
              AND translation.slug IS NOT NULL
              AND length(translation.slug) > 0
            ORDER BY translation.locale, translation.id
            "#,
            vec![tenant_id.into(), topic_id.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    db.query_all(statement)
        .await?
        .into_iter()
        .map(topic_translation_route_from_row)
        .collect()
}

async fn lock_topic_route_for_rename_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
) -> ForumResult<CurrentTopicRoute> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let row = txn
                .query_one(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    r#"
                    SELECT topic.id AS topic_id, translation.slug
                    FROM forum_topics topic
                    JOIN forum_topic_translations translation
                      ON translation.tenant_id = topic.tenant_id
                     AND translation.topic_id = topic.id
                    WHERE topic.tenant_id = $1
                      AND topic.id = $2
                      AND topic.deleted_at IS NULL
                      AND translation.locale = $3
                      AND translation.slug IS NOT NULL
                      AND length(translation.slug) > 0
                    FOR UPDATE OF translation
                    "#,
                    vec![tenant_id.into(), topic_id.into(), locale.into()],
                ))
                .await?;
            row.map(current_route_from_row)
                .transpose()?
                .ok_or(ForumError::TopicRouteNotFound)
        }
        DatabaseBackend::Sqlite => {
            let result = txn
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    r#"
                    UPDATE forum_topic_translations
                    SET updated_at = updated_at
                    WHERE tenant_id = ?
                      AND topic_id = ?
                      AND locale = ?
                      AND slug IS NOT NULL
                      AND length(slug) > 0
                      AND EXISTS (
                          SELECT 1 FROM forum_topics topic
                          WHERE topic.tenant_id = ?
                            AND topic.id = ?
                            AND topic.deleted_at IS NULL
                      )
                    "#,
                    vec![
                        tenant_id.into(),
                        topic_id.into(),
                        locale.into(),
                        tenant_id.into(),
                        topic_id.into(),
                    ],
                ))
                .await?;
            if result.rows_affected() != 1 {
                return Err(ForumError::TopicRouteNotFound);
            }
            load_current_route_for_topic(txn, tenant_id, topic_id, locale)
                .await?
                .ok_or(ForumError::TopicRouteNotFound)
        }
        backend => Err(unsupported_backend(backend)),
    }
}

async fn update_topic_route_slug_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
    slug: &str,
) -> ForumResult<()> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            UPDATE forum_topic_translations
            SET slug = $1, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $2 AND topic_id = $3 AND locale = $4
            "#,
            vec![slug.into(), tenant_id.into(), topic_id.into(), locale.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_topic_translations
            SET slug = ?, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND locale = ?
            "#,
            vec![slug.into(), tenant_id.into(), topic_id.into(), locale.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    let result = txn.execute(statement).await?;
    if result.rows_affected() != 1 {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    Ok(())
}

async fn load_current_route_for_topic<C>(
    db: &C,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
) -> ForumResult<Option<CurrentTopicRoute>>
where
    C: ConnectionTrait,
{
''',
    "current route generic and rename helpers",
)
route = replace_once(
    route,
    '''async fn load_current_routes_by_short_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
) -> ForumResult<Vec<CurrentTopicRoute>> {
''',
    '''async fn load_current_routes_by_short_id<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
) -> ForumResult<Vec<CurrentTopicRoute>>
where
    C: ConnectionTrait,
{
''',
    "short-id query generic",
)
route = replace_once(
    route,
    '''async fn ensure_unambiguous_current_short_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
    expected_topic_id: Uuid,
) -> ForumResult<()> {
''',
    '''async fn ensure_unambiguous_current_short_id<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
    expected_topic_id: Uuid,
) -> ForumResult<()>
where
    C: ConnectionTrait,
{
''',
    "short-id guard generic",
)
route_path.write_text(route)

owner_path = Path("crates/rustok-forum/src/services/topic_owner.rs")
owner = owner_path.read_text()
owner = replace_once(owner, "use flex::delete_attached_localized_values;\n", "use chrono::Utc;\nuse flex::delete_attached_localized_values;\n", "owner chrono import")
owner = replace_once(
    owner,
    '''use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, TransactionTrait,
};
''',
    '''use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};
''',
    "owner SeaORM imports",
)
owner = replace_once(
    owner,
    "use crate::entities::{forum_reply, forum_solution};\n",
    "use crate::entities::{forum_reply, forum_solution, forum_topic};\n",
    "owner topic entity import",
)
owner = replace_once(
    owner,
    "use super::topic_route::ForumTopicRouteService;\n",
    '''use super::topic_route::{
    ForumTopicRouteService, ForumTopicSlugRenameResult, RenameForumTopicSlugInput,
};
''',
    "owner route imports",
)
owner = replace_once(
    owner,
    '''    #[instrument(skip(self, security))]
    pub async fn delete(
''',
    '''    #[instrument(skip(self, security, input))]
    pub async fn rename_slug(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        let existing = self.inner.find_topic(tenant_id, topic_id).await?;
        enforce_owned_scope(
            &security,
            Resource::ForumTopics,
            Action::Update,
            existing.author_id,
        )?;

        let txn = self.db.begin().await?;
        let result = ForumTopicRouteService::rename_topic_slug_in_tx(
            &txn,
            tenant_id,
            topic_id,
            &input,
        )
        .await?;
        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
        Ok(result)
    }

    #[instrument(skip(self, security))]
    pub async fn delete(
''',
    "owner rename command",
)
owner_path.write_text(owner)

facade_path = Path("crates/rustok-forum/src/services/topic_facade.rs")
facade = facade_path.read_text()
facade = replace_once(
    facade,
    '''use super::topic_canonical_resolution::{
    ForumTopicCanonicalResolution, ForumTopicCanonicalResolutionService,
};
''',
    '''use super::topic_canonical_resolution::{
    ForumTopicCanonicalResolution, ForumTopicCanonicalResolutionService,
};
use super::topic_route::{ForumTopicSlugRenameResult, RenameForumTopicSlugInput};
''',
    "facade route imports",
)
facade = replace_once(
    facade,
    '''    pub async fn delete(
''',
    '''    pub async fn rename_slug(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: RenameForumTopicSlugInput,
    ) -> ForumResult<ForumTopicSlugRenameResult> {
        self.inner
            .rename_slug(tenant_id, topic_id, security, input)
            .await
    }

    pub async fn delete(
''',
    "facade rename command",
)
facade_path.write_text(facade)

services_path = Path("crates/rustok-forum/src/services/mod.rs")
services = services_path.read_text()
services = replace_once(
    services,
    '''pub use topic_route::{
    FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumTopicRouteDescriptor, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
};
''',
    '''pub use topic_route::{
    FORUM_TOPIC_RENAMED_ROUTE_REASON, FORUM_TOPIC_ROUTE_SHORT_ID_LEN,
    ForumTopicRouteDescriptor, ForumTopicRouteDisposition, ForumTopicRouteResolution,
    ForumTopicRouteService, ForumTopicSlugRenameResult, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
    RenameForumTopicSlugInput,
};
''',
    "services route exports",
)
services_path.write_text(services)

lib_path = Path("crates/rustok-forum/src/lib.rs")
lib = lib_path.read_text()
lib = replace_once(
    lib,
    '''pub use services::{
    FORUM_TOPIC_ROUTE_SHORT_ID_LEN, ForumTopicRouteDescriptor, ForumTopicRouteDisposition,
    ForumTopicRouteResolution, ForumTopicRouteService, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
};
''',
    '''pub use services::{
    FORUM_TOPIC_RENAMED_ROUTE_REASON, FORUM_TOPIC_ROUTE_SHORT_ID_LEN,
    ForumTopicRouteDescriptor, ForumTopicRouteDisposition, ForumTopicRouteResolution,
    ForumTopicRouteService, ForumTopicSlugRenameResult, MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN,
    MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN, MAX_FORUM_TOPIC_ROUTE_SLUG_LEN,
    RenameForumTopicSlugInput,
};
''',
    "crate route exports",
)
lib_path.write_text(lib)

readme_path = Path("crates/rustok-forum/docs/README.md")
readme = readme_path.read_text()
readme = replace_once(
    readme,
    "- FORUM-24C records immutable localized `gone` routes in the topic delete transaction while preserving existing merge redirects.\n",
    "- FORUM-24C records immutable localized `gone` routes in the topic delete transaction while preserving existing merge redirects.\n"
    "- FORUM-24D adds an explicit owner command for localized topic slug changes with atomic old-route aliases and delete/merge lifecycle resolution.\n",
    "README FORUM-24D summary",
)
readme = replace_once(
    readme,
    "- [FORUM-24C topic delete route tombstones](./forum-24c-topic-delete-route-tombstones.md)\n",
    "- [FORUM-24C topic delete route tombstones](./forum-24c-topic-delete-route-tombstones.md)\n"
    "- [FORUM-24D topic slug rename owner](./forum-24d-topic-slug-rename-owner.md)\n",
    "README FORUM-24D link",
)
readme_path.write_text(readme)

plan_path = Path("crates/rustok-forum/docs/implementation-plan.md")
plan = plan_path.read_text()
plan = replace_once(
    plan,
    "| `FORUM-24` | `planned` | FORUM-24A adds deterministic exact-locale topic route identity and an immutable redirect/tombstone ledger; FORUM-24B composes new merge redirects and FORUM-24C composes delete tombstones in their owner transactions. Rename composition, historical backfill, category routes, storefront mounts, hreflang/SEO policy and runtime evidence remain. |",
    "| `FORUM-24` | `planned` | FORUM-24A-D provide deterministic topic route identity, immutable merge/delete history and an explicit localized slug rename owner. Historical backfill, category routes, storefront mounts, transport/UI composition, hreflang/SEO policy and runtime evidence remain. |",
    "FORUM-24 ledger",
)
plan = replace_once(
    plan,
    '''Topic rename aliases, historical backfill, storefront mounting, category routes,
hreflang/SEO policy and retained runtime proof remain.
''',
    '''Historical backfill, storefront mounting, category routes, hreflang/SEO policy and
retained runtime proof remain.
''',
    "FORUM-24C remaining scope",
)
plan = replace_once(
    plan,
    '''No command above was run by the implementation agent, per maintainer request.

## `FORUM-25` — full multilingual and RTL contract
''',
    '''No command above was run by the implementation agent, per maintainer request.

### Delivered in FORUM-24D

- `TopicService::rename_slug` is an explicit owner command with the same
  `forum_topics:update` ownership authorization as the existing topic update;
- the command requires one existing tenant/topic/locale translation with a
  non-empty slug and rejects empty normalized route segments;
- the exact localized route is locked, its old path is stored as an immutable
  self-target redirect, and the translation slug plus topic timestamp commit in
  the same transaction;
- exact normalized replay returns `changed = false` without duplicating the
  alias, while alias ownership or payload drift fails closed;
- old rename paths follow bounded merge canonicalization with exact/fallback
  locale selection and resolve as `gone` after deletion;
- no REST, GraphQL or admin surface changed in this owner-only slice.

Historical backfill, category routes, storefront mounting, transport/UI
composition, hreflang/SEO policy and retained runtime proof remain.

Verification sources:

```bash
node scripts/verify/verify-forum-topic-slug-rename-owner.mjs
cargo test -p rustok-forum --test topic_slug_rename_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.

## `FORUM-25` — full multilingual and RTL contract
''',
    "FORUM-24D delivered section",
)
plan_path.write_text(plan)
