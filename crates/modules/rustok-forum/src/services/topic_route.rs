use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_content::normalize_locale_code;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, QueryResult,
    Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

use super::topic_canonical_resolution::ForumTopicCanonicalResolutionService;

pub const FORUM_TOPIC_ROUTE_SHORT_ID_LEN: usize = 12;
pub const MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN: usize = 64;
pub const MAX_FORUM_TOPIC_ROUTE_SLUG_LEN: usize = 255;
pub const MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN: usize = 500;
pub const FORUM_TOPIC_RENAMED_ROUTE_REASON: &str = "Topic slug changed";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumTopicRouteDisposition {
    Canonical,
    Redirect,
    Gone,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumTopicRouteDescriptor {
    pub topic_id: Uuid,
    pub locale: String,
    pub short_id: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumTopicRouteResolution {
    pub requested_locale: String,
    pub requested_short_id: String,
    pub requested_slug: String,
    pub requested_topic_id: Option<Uuid>,
    pub disposition: ForumTopicRouteDisposition,
    pub canonical: Option<ForumTopicRouteDescriptor>,
    pub alias_id: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    Redirect,
    Gone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
    alias_id: Uuid,
    topic_id: Uuid,
    disposition: StoredRouteDisposition,
    target_topic_id: Option<Uuid>,
    target_locale: Option<String>,
    reason: String,
}

/// Forum-owned topic route identity and immutable alias resolver.
///
/// The short identity is the first 48 bits of the topic UUID rendered as twelve
/// lowercase hexadecimal characters. A route is canonical only when exactly one
/// topic in the tenant/locale owns that short identity. Collisions fail closed.
///
/// This service is intentionally transport-neutral. Callers must perform the
/// same visibility/read authorization required for the canonical topic before
/// disclosing a descriptor or redirect target.
pub struct ForumTopicRouteService {
    db: DatabaseConnection,
}

impl ForumTopicRouteService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn short_identity(topic_id: Uuid) -> String {
        let compact = topic_id.simple().to_string();
        compact[..FORUM_TOPIC_ROUTE_SHORT_ID_LEN].to_string()
    }

    pub async fn canonical_descriptor(
        &self,
        tenant_id: Uuid,
        requested_topic_id: Uuid,
        locale: &str,
    ) -> ForumResult<ForumTopicRouteDescriptor> {
        let locale = normalize_route_locale(locale)?;
        let canonical = ForumTopicCanonicalResolutionService::new(self.db.clone())
            .resolve_unchecked(tenant_id, requested_topic_id)
            .await?;
        let route = load_current_route_for_topic(
            &self.db,
            tenant_id,
            canonical.canonical_topic_id,
            &locale,
        )
        .await?
        .ok_or(ForumError::TopicRouteNotFound)?;
        let short_id = Self::short_identity(route.topic_id);
        ensure_unambiguous_current_short_id(
            &self.db,
            tenant_id,
            &locale,
            &short_id,
            route.topic_id,
        )
        .await?;

        let slug = normalize_route_slug(&route.slug)?;
        Ok(ForumTopicRouteDescriptor {
            topic_id: route.topic_id,
            path: forum_topic_route_path(&locale, &short_id, &slug),
            locale,
            short_id,
            slug,
        })
    }

    async fn canonical_descriptor_with_locale_fallback(
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
        &self,
        tenant_id: Uuid,
        locale: &str,
        short_id: &str,
        slug: &str,
    ) -> ForumResult<ForumTopicRouteResolution> {
        let locale = normalize_route_locale(locale)?;
        let short_id = normalize_short_identity(short_id)?;
        let slug = normalize_route_slug(slug)?;

        let current =
            load_current_routes_by_short_id(&self.db, tenant_id, &locale, &short_id).await?;
        match current.as_slice() {
            [route] => {
                let canonical = self
                    .canonical_descriptor_with_locale_fallback(tenant_id, route.topic_id, &locale)
                    .await?;
                let disposition = if route.topic_id == canonical.topic_id && slug == canonical.slug
                {
                    ForumTopicRouteDisposition::Canonical
                } else {
                    ForumTopicRouteDisposition::Redirect
                };
                return Ok(ForumTopicRouteResolution {
                    requested_locale: locale,
                    requested_short_id: short_id,
                    requested_slug: slug,
                    requested_topic_id: Some(route.topic_id),
                    disposition,
                    canonical: Some(canonical),
                    alias_id: None,
                });
            }
            [] => {}
            _ => return Err(ForumError::TopicRouteResolutionConflict),
        }

        let aliases = load_route_aliases(&self.db, tenant_id, &locale, &short_id, &slug).await?;
        let alias = match aliases.as_slice() {
            [alias] => alias,
            [] => return Err(ForumError::TopicRouteNotFound),
            _ => return Err(ForumError::TopicRouteResolutionConflict),
        };

        match alias.disposition {
            StoredRouteDisposition::Gone => Ok(ForumTopicRouteResolution {
                requested_locale: locale,
                requested_short_id: short_id,
                requested_slug: slug,
                requested_topic_id: Some(alias.topic_id),
                disposition: ForumTopicRouteDisposition::Gone,
                canonical: None,
                alias_id: Some(alias.alias_id),
            }),
            StoredRouteDisposition::Redirect => {
                let target_topic_id = alias
                    .target_topic_id
                    .ok_or(ForumError::TopicRouteResolutionConflict)?;
                let target_locale = alias
                    .target_locale
                    .as_deref()
                    .ok_or(ForumError::TopicRouteResolutionConflict)?;
                let canonical = if target_topic_id == alias.topic_id {
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
            }
        }
    }

    /// Renames one existing localized route and records its previous path atomically.
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
        ensure_unambiguous_current_short_id(txn, tenant_id, &locale, &short_id, topic_id).await?;
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
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        slug: &str,
        target_topic_id: Uuid,
        target_locale: &str,
        reason: &str,
    ) -> ForumResult<Uuid> {
        record_alias_in_tx(
            txn,
            tenant_id,
            topic_id,
            locale,
            slug,
            StoredRouteDisposition::Redirect,
            Some(target_topic_id),
            Some(target_locale),
            reason,
        )
        .await
    }

    pub(crate) async fn record_gone_alias_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_id: Uuid,
        locale: &str,
        slug: &str,
        reason: &str,
    ) -> ForumResult<Uuid> {
        record_alias_in_tx(
            txn,
            tenant_id,
            topic_id,
            locale,
            slug,
            StoredRouteDisposition::Gone,
            None,
            None,
            reason,
        )
        .await
    }
}

fn forum_topic_route_path(locale: &str, short_id: &str, slug: &str) -> String {
    format!("/{locale}/forum/t/{short_id}/{slug}")
}

fn normalize_route_locale(locale: &str) -> ForumResult<String> {
    let locale = normalize_locale_code(locale)
        .ok_or_else(|| ForumError::Validation("Invalid forum topic route locale".to_string()))?;
    if locale.chars().count() > MAX_FORUM_TOPIC_ROUTE_LOCALE_LEN {
        return Err(ForumError::Validation(
            "Forum topic route locale is too long".to_string(),
        ));
    }
    Ok(locale)
}

fn normalize_short_identity(value: &str) -> ForumResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != FORUM_TOPIC_ROUTE_SHORT_ID_LEN
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ForumError::TopicRouteNotFound);
    }
    Ok(value)
}

fn normalize_route_slug(value: &str) -> ForumResult<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() || normalized.len() > MAX_FORUM_TOPIC_ROUTE_SLUG_LEN {
        return Err(ForumError::TopicRouteNotFound);
    }
    Ok(normalized)
}

fn normalize_route_slug_for_write(value: &str) -> ForumResult<String> {
    normalize_route_slug(value).map_err(|error| match error {
        ForumError::TopicRouteNotFound => ForumError::Validation(
            "Forum topic route slug must contain a valid route segment".to_string(),
        ),
        other => other,
    })
}

fn normalize_alias_reason(reason: &str) -> ForumResult<String> {
    let reason = reason.trim();
    if reason.is_empty()
        || reason.chars().count() > MAX_FORUM_TOPIC_ROUTE_ALIAS_REASON_LEN
        || reason.chars().any(char::is_control)
    {
        return Err(ForumError::Validation(
            "Forum topic route alias reason is invalid".to_string(),
        ));
    }
    Ok(reason.to_string())
}

async fn load_topic_translation_routes_in_tx(
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
    txn.query_all_raw(statement)
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

async fn load_current_topic_translation_routes<C>(
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
    db.query_all_raw(statement)
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
                .query_one_raw(Statement::from_sql_and_values(
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
                .execute_raw(Statement::from_sql_and_values(
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
            vec![
                slug.into(),
                tenant_id.into(),
                topic_id.into(),
                locale.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE forum_topic_translations
            SET slug = ?, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = ? AND topic_id = ? AND locale = ?
            "#,
            vec![
                slug.into(),
                tenant_id.into(),
                topic_id.into(),
                locale.into(),
            ],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    let result = txn.execute_raw(statement).await?;
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
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
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
            LIMIT 1
            "#,
            vec![tenant_id.into(), topic_id.into(), locale.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT topic.id AS topic_id, translation.slug
            FROM forum_topics topic
            JOIN forum_topic_translations translation
              ON translation.tenant_id = topic.tenant_id
             AND translation.topic_id = topic.id
            WHERE topic.tenant_id = ?
              AND topic.id = ?
              AND topic.deleted_at IS NULL
              AND translation.locale = ?
              AND translation.slug IS NOT NULL
              AND length(translation.slug) > 0
            LIMIT 1
            "#,
            vec![tenant_id.into(), topic_id.into(), locale.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    let row = db.query_one_raw(statement).await?;
    row.map(current_route_from_row).transpose()
}

async fn load_current_routes_by_short_id<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
) -> ForumResult<Vec<CurrentTopicRoute>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT topic.id AS topic_id, translation.slug
            FROM forum_topics topic
            JOIN forum_topic_translations translation
              ON translation.tenant_id = topic.tenant_id
             AND translation.topic_id = topic.id
            WHERE topic.tenant_id = $1
              AND topic.deleted_at IS NULL
              AND translation.locale = $2
              AND translation.slug IS NOT NULL
              AND length(translation.slug) > 0
              AND left(replace(lower(topic.id::text), '-', ''), 12) = $3
            ORDER BY topic.id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), short_id.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT topic.id AS topic_id, translation.slug
            FROM forum_topics topic
            JOIN forum_topic_translations translation
              ON translation.tenant_id = topic.tenant_id
             AND translation.topic_id = topic.id
            WHERE topic.tenant_id = ?
              AND topic.deleted_at IS NULL
              AND translation.locale = ?
              AND translation.slug IS NOT NULL
              AND length(translation.slug) > 0
              AND substr(lower(hex(topic.id)), 1, 12) = ?
            ORDER BY topic.id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), short_id.into()],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    db.query_all_raw(statement)
        .await?
        .into_iter()
        .map(current_route_from_row)
        .collect()
}

async fn ensure_unambiguous_current_short_id<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
    expected_topic_id: Uuid,
) -> ForumResult<()>
where
    C: ConnectionTrait,
{
    let routes = load_current_routes_by_short_id(db, tenant_id, locale, short_id).await?;
    match routes.as_slice() {
        [route] if route.topic_id == expected_topic_id => Ok(()),
        _ => Err(ForumError::TopicRouteResolutionConflict),
    }
}

fn current_route_from_row(row: QueryResult) -> ForumResult<CurrentTopicRoute> {
    Ok(CurrentTopicRoute {
        topic_id: row.try_get("", "topic_id")?,
        slug: row.try_get("", "slug")?,
    })
}

async fn load_route_aliases<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    short_id: &str,
    slug: &str,
) -> ForumResult<Vec<StoredTopicRouteAlias>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT alias_id, topic_id, disposition, target_topic_id, target_locale, reason
            FROM forum_topic_route_aliases
            WHERE tenant_id = $1
              AND locale = $2
              AND short_id = $3
              AND slug = $4
            ORDER BY alias_id
            LIMIT 2
            "#,
            vec![
                tenant_id.into(),
                locale.into(),
                short_id.into(),
                slug.into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT alias_id, topic_id, disposition, target_topic_id, target_locale, reason
            FROM forum_topic_route_aliases
            WHERE tenant_id = ?
              AND locale = ?
              AND short_id = ?
              AND slug = ?
            ORDER BY alias_id
            LIMIT 2
            "#,
            vec![
                tenant_id.into(),
                locale.into(),
                short_id.into(),
                slug.into(),
            ],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    db.query_all_raw(statement)
        .await?
        .into_iter()
        .map(stored_alias_from_row)
        .collect()
}

fn stored_alias_from_row(row: QueryResult) -> ForumResult<StoredTopicRouteAlias> {
    let disposition: String = row.try_get("", "disposition")?;
    let disposition = match disposition.as_str() {
        "redirect" => StoredRouteDisposition::Redirect,
        "gone" => StoredRouteDisposition::Gone,
        _ => return Err(ForumError::TopicRouteResolutionConflict),
    };
    Ok(StoredTopicRouteAlias {
        alias_id: row.try_get("", "alias_id")?,
        topic_id: row.try_get("", "topic_id")?,
        disposition,
        target_topic_id: row.try_get("", "target_topic_id")?,
        target_locale: row.try_get("", "target_locale")?,
        reason: row.try_get("", "reason")?,
    })
}

#[allow(clippy::too_many_arguments)]
async fn record_alias_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    topic_id: Uuid,
    locale: &str,
    slug: &str,
    disposition: StoredRouteDisposition,
    target_topic_id: Option<Uuid>,
    target_locale: Option<&str>,
    reason: &str,
) -> ForumResult<Uuid> {
    let locale = normalize_route_locale(locale)?;
    let slug = normalize_route_slug(slug)?;
    let short_id = ForumTopicRouteService::short_identity(topic_id);
    let reason = normalize_alias_reason(reason)?;
    let target_locale = target_locale.map(normalize_route_locale).transpose()?;
    let disposition_value = match disposition {
        StoredRouteDisposition::Redirect => "redirect",
        StoredRouteDisposition::Gone => "gone",
    };
    match disposition {
        StoredRouteDisposition::Redirect
            if target_topic_id.is_none() || target_locale.is_none() =>
        {
            return Err(ForumError::TopicRouteResolutionConflict);
        }
        StoredRouteDisposition::Gone if target_topic_id.is_some() || target_locale.is_some() => {
            return Err(ForumError::TopicRouteResolutionConflict);
        }
        _ => {}
    }

    let alias_id = Uuid::new_v4();
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO forum_topic_route_aliases (
                tenant_id, alias_id, topic_id, locale, short_id, slug,
                disposition, target_topic_id, target_locale, reason, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, locale, short_id, slug) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                alias_id.into(),
                topic_id.into(),
                locale.clone().into(),
                short_id.clone().into(),
                slug.clone().into(),
                disposition_value.into(),
                target_topic_id.into(),
                target_locale.clone().into(),
                reason.clone().into(),
            ],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            INSERT INTO forum_topic_route_aliases (
                tenant_id, alias_id, topic_id, locale, short_id, slug,
                disposition, target_topic_id, target_locale, reason, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, locale, short_id, slug) DO NOTHING
            "#,
            vec![
                tenant_id.into(),
                alias_id.into(),
                topic_id.into(),
                locale.clone().into(),
                short_id.clone().into(),
                slug.clone().into(),
                disposition_value.into(),
                target_topic_id.into(),
                target_locale.clone().into(),
                reason.clone().into(),
            ],
        ),
        backend => return Err(unsupported_backend(backend)),
    };
    txn.execute_raw(statement).await?;

    let aliases = load_route_aliases(txn, tenant_id, &locale, &short_id, &slug).await?;
    let existing = match aliases.as_slice() {
        [alias] => alias,
        _ => return Err(ForumError::TopicRouteResolutionConflict),
    };
    if existing.topic_id != topic_id
        || existing.disposition != disposition
        || existing.target_topic_id != target_topic_id
        || existing.target_locale != target_locale
        || existing.reason != reason
    {
        return Err(ForumError::TopicRouteResolutionConflict);
    }
    Ok(existing.alias_id)
}

fn unsupported_backend(backend: DatabaseBackend) -> ForumError {
    ForumError::Validation(format!(
        "Forum topic route identity does not support database backend {backend:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_identity_is_stable_and_lowercase() {
        let topic_id = Uuid::parse_str("ABCD1234-5678-4ABC-8DEF-0123456789AB").expect("uuid");
        assert_eq!(
            ForumTopicRouteService::short_identity(topic_id),
            "abcd12345678"
        );
    }

    #[test]
    fn canonical_path_uses_normalized_locale_and_slug() {
        let locale = normalize_route_locale(" EN_us ").expect("locale");
        let slug = normalize_route_slug(" A focused topic! ").expect("slug");
        assert_eq!(
            forum_topic_route_path(&locale, "abcd12345678", &slug),
            "/en-US/forum/t/abcd12345678/a-focused-topic"
        );
    }

    #[test]
    fn malformed_short_identity_fails_as_not_found() {
        assert!(matches!(
            normalize_short_identity("not-a-route"),
            Err(ForumError::TopicRouteNotFound)
        ));
    }
}
