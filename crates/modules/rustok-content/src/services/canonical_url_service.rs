use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rustok_core::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set,
};
use uuid::Uuid;

use super::content_orchestration_service::CanonicalUrlMutation;
use crate::entities::{canonical_url, url_alias};
use crate::{ContentError, ContentResult, normalize_locale_code, resolve_by_locale};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContentRoute {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub matched_url: String,
    pub canonical_url: String,
    pub redirect_required: bool,
}

#[derive(Clone)]
pub struct CanonicalUrlService {
    db: DatabaseConnection,
}

impl CanonicalUrlService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn resolve_route(
        &self,
        tenant_id: Uuid,
        locale: &str,
        route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>> {
        let locale = normalize_locale_code(locale)
            .ok_or_else(|| ContentError::validation("locale must not be empty"))?;
        let route = normalize_route(route)?;

        let aliases = url_alias::Entity::find()
            .filter(url_alias::Column::TenantId.eq(tenant_id))
            .filter(url_alias::Column::AliasUrl.eq(route.clone()))
            .all(&self.db)
            .await?;
        let resolved_alias = resolve_by_locale(&aliases, &locale, |alias| alias.locale.as_str());
        if let Some(alias) = resolved_alias.item {
            return Ok(Some(ResolvedContentRoute {
                target_kind: alias.target_kind.clone(),
                target_id: alias.target_id,
                locale: resolved_alias.effective_locale,
                matched_url: alias.alias_url.clone(),
                canonical_url: alias.canonical_url.clone(),
                redirect_required: true,
            }));
        }

        let canonicals = canonical_url::Entity::find()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::CanonicalUrl.eq(route.clone()))
            .all(&self.db)
            .await?;
        let resolved_canonical =
            resolve_by_locale(&canonicals, &locale, |canonical| canonical.locale.as_str());

        Ok(resolved_canonical
            .item
            .map(|canonical| ResolvedContentRoute {
                target_kind: canonical.target_kind.clone(),
                target_id: canonical.target_id,
                locale: resolved_canonical.effective_locale,
                matched_url: route,
                canonical_url: canonical.canonical_url.clone(),
                redirect_required: false,
            }))
    }
}

/// Single writer for `canonical_url` and `url_alias` state.
///
/// Every module that changes a public route (content orchestration, Blog slug
/// changes, target deletion) goes through this type, so alias protection,
/// purge events, and SEO redirect events stay on one code path.
#[derive(Clone)]
pub struct CanonicalUrlWriter {
    event_bus: TransactionalEventBus,
}

impl CanonicalUrlWriter {
    pub fn new(event_bus: TransactionalEventBus) -> Self {
        Self { event_bus }
    }

    /// Removes every canonical and alias row owned by one target and publishes
    /// `UrlAliasPurged` for the removed routes. Used when the target record is
    /// deleted, so public routes never resolve to a missing record.
    pub async fn remove_target_routes_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        target_kind: &str,
        target_id: Uuid,
    ) -> ContentResult<()> {
        let target_kind = self.normalize_target_kind(target_kind)?;
        let canonicals = canonical_url::Entity::find()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::TargetKind.eq(target_kind.clone()))
            .filter(canonical_url::Column::TargetId.eq(target_id))
            .all(txn)
            .await?;
        let aliases = url_alias::Entity::find()
            .filter(url_alias::Column::TenantId.eq(tenant_id))
            .filter(url_alias::Column::TargetKind.eq(target_kind.clone()))
            .filter(url_alias::Column::TargetId.eq(target_id))
            .all(txn)
            .await?;

        let mut purged: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for canonical in &canonicals {
            purged
                .entry(canonical.locale.clone())
                .or_default()
                .insert(canonical.canonical_url.clone());
        }
        for alias in &aliases {
            purged
                .entry(alias.locale.clone())
                .or_default()
                .insert(alias.alias_url.clone());
        }

        canonical_url::Entity::delete_many()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::TargetKind.eq(target_kind.clone()))
            .filter(canonical_url::Column::TargetId.eq(target_id))
            .exec(txn)
            .await?;
        url_alias::Entity::delete_many()
            .filter(url_alias::Column::TenantId.eq(tenant_id))
            .filter(url_alias::Column::TargetKind.eq(target_kind.clone()))
            .filter(url_alias::Column::TargetId.eq(target_id))
            .exec(txn)
            .await?;

        for (locale, urls) in purged {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    actor_id,
                    DomainEvent::UrlAliasPurged {
                        target_id,
                        target_kind: target_kind.clone(),
                        locale,
                        urls: urls.into_iter().collect(),
                    },
                )
                .await?;
        }
        Ok(())
    }

    /// Releases every alias row that still points at `route`, in every locale,
    /// so a module that owns a global route can claim it for a new target (for
    /// example a new Blog post that takes a retired slug). Each released alias
    /// is reported through `UrlAliasPurged`. Fails when a live canonical URL
    /// already owns the route, because a live canonical is never released.
    pub async fn release_alias_route_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        route: &str,
    ) -> ContentResult<()> {
        let route = self.normalize_route_url("route", route)?;
        let live_canonical = canonical_url::Entity::find()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::CanonicalUrl.eq(route.clone()))
            .one(txn)
            .await?;
        if live_canonical.is_some() {
            return Err(ContentError::validation(format!(
                "route `{route}` is a live canonical URL and cannot be released"
            )));
        }

        let aliases = url_alias::Entity::find()
            .filter(url_alias::Column::TenantId.eq(tenant_id))
            .filter(url_alias::Column::AliasUrl.eq(route.clone()))
            .all(txn)
            .await?;

        let mut purged: BTreeSet<(String, Uuid, String)> = BTreeSet::new();
        for alias in aliases {
            purged.insert((
                alias.target_kind.clone(),
                alias.target_id,
                alias.locale.clone(),
            ));
            url_alias::Entity::delete_by_id(alias.id).exec(txn).await?;
        }

        for (target_kind, target_id, locale) in purged {
            self.event_bus
                .publish_in_tx(
                    txn,
                    tenant_id,
                    actor_id,
                    DomainEvent::UrlAliasPurged {
                        target_id,
                        target_kind,
                        locale,
                        urls: vec![route.clone()],
                    },
                )
                .await?;
        }
        Ok(())
    }

    fn normalize_target_kind(&self, target_kind: &str) -> ContentResult<String> {
        let normalized = target_kind.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(ContentError::validation("target_kind must not be empty"));
        }
        if normalized.len() > 64 {
            return Err(ContentError::validation("target_kind must be <= 64 chars"));
        }
        if !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
        {
            return Err(ContentError::validation(
                "target_kind may contain only lowercase ascii letters, digits, `_`, or `.`",
            ));
        }
        Ok(normalized)
    }

    fn normalize_route_url(&self, field: &str, value: &str) -> ContentResult<String> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(ContentError::validation(format!(
                "{field} must not be empty"
            )));
        }
        if normalized.len() > 512 {
            return Err(ContentError::validation(format!(
                "{field} must be <= 512 chars"
            )));
        }
        if !normalized.starts_with('/') {
            return Err(ContentError::validation(format!(
                "{field} must start with `/`"
            )));
        }
        if normalized.chars().any(char::is_whitespace) || normalized.contains("://") {
            return Err(ContentError::validation(format!(
                "{field} must be a relative route without whitespace or scheme"
            )));
        }
        Ok(normalized.to_string())
    }

    pub async fn apply_canonical_url_mutations(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        updates: &[CanonicalUrlMutation],
    ) -> ContentResult<()> {
        for update in updates {
            let target_kind = self.normalize_target_kind(&update.target_kind)?;
            let locale = normalize_locale_code(&update.locale)
                .ok_or_else(|| ContentError::validation("canonical locale must not be empty"))?;
            let canonical_route =
                self.normalize_route_url("canonical_url", update.canonical_url.as_str())?;

            self.ensure_canonical_route_available(
                txn,
                tenant_id,
                &locale,
                &canonical_route,
                &target_kind,
                update.target_id,
            )
            .await?;

            let mut alias_urls = BTreeSet::new();
            for alias in &update.alias_urls {
                let alias = self.normalize_route_url("alias_url", alias)?;
                if alias != canonical_route {
                    alias_urls.insert(alias);
                }
            }

            for retired in &update.retired_targets {
                let retired_kind = self.normalize_target_kind(&retired.target_kind)?;
                let retired_locale = normalize_locale_code(&retired.locale).ok_or_else(|| {
                    ContentError::validation("retired canonical locale must not be empty")
                })?;

                let retired_canonical = canonical_url::Entity::find()
                    .filter(canonical_url::Column::TenantId.eq(tenant_id))
                    .filter(canonical_url::Column::TargetKind.eq(retired_kind.clone()))
                    .filter(canonical_url::Column::TargetId.eq(retired.target_id))
                    .filter(canonical_url::Column::Locale.eq(retired_locale.clone()))
                    .one(txn)
                    .await?;

                if let Some(retired_canonical) = retired_canonical {
                    if retired_canonical.canonical_url != canonical_route {
                        alias_urls.insert(retired_canonical.canonical_url.clone());
                    }
                    canonical_url::Entity::delete_by_id(retired_canonical.id)
                        .exec(txn)
                        .await?;
                }

                let retired_aliases = url_alias::Entity::find()
                    .filter(url_alias::Column::TenantId.eq(tenant_id))
                    .filter(url_alias::Column::TargetKind.eq(retired_kind))
                    .filter(url_alias::Column::TargetId.eq(retired.target_id))
                    .filter(url_alias::Column::Locale.eq(retired_locale))
                    .all(txn)
                    .await?;
                for retired_alias in retired_aliases {
                    if retired_alias.alias_url != canonical_route {
                        alias_urls.insert(retired_alias.alias_url.clone());
                    }
                    url_alias::Entity::delete_by_id(retired_alias.id)
                        .exec(txn)
                        .await?;
                }
            }

            let existing_canonical = canonical_url::Entity::find()
                .filter(canonical_url::Column::TenantId.eq(tenant_id))
                .filter(canonical_url::Column::TargetKind.eq(target_kind.clone()))
                .filter(canonical_url::Column::TargetId.eq(update.target_id))
                .filter(canonical_url::Column::Locale.eq(locale.clone()))
                .one(txn)
                .await?;

            let now = Utc::now();
            let mut mapping_changed = false;
            if let Some(existing_canonical) = existing_canonical {
                if existing_canonical.canonical_url != canonical_route {
                    alias_urls.insert(existing_canonical.canonical_url.clone());
                    let mut active: canonical_url::ActiveModel = existing_canonical.into();
                    active.canonical_url = Set(canonical_route.clone());
                    active.updated_at = Set(now.into());
                    active.update(txn).await?;
                    mapping_changed = true;
                }
            } else {
                canonical_url::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    tenant_id: Set(tenant_id),
                    target_kind: Set(target_kind.clone()),
                    target_id: Set(update.target_id),
                    locale: Set(locale.clone()),
                    canonical_url: Set(canonical_route.clone()),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(txn)
                .await?;
                mapping_changed = true;
            }

            let alias_urls = alias_urls.into_iter().collect::<Vec<_>>();

            for alias in &alias_urls {
                self.ensure_alias_route_available(
                    txn,
                    tenant_id,
                    &locale,
                    alias,
                    &target_kind,
                    update.target_id,
                )
                .await?;
            }

            url_alias::Entity::delete_many()
                .filter(url_alias::Column::TenantId.eq(tenant_id))
                .filter(url_alias::Column::Locale.eq(locale.clone()))
                .filter(url_alias::Column::AliasUrl.eq(canonical_route.clone()))
                .exec(txn)
                .await?;

            for alias in &alias_urls {
                let existing_alias = url_alias::Entity::find()
                    .filter(url_alias::Column::TenantId.eq(tenant_id))
                    .filter(url_alias::Column::Locale.eq(locale.clone()))
                    .filter(url_alias::Column::AliasUrl.eq(alias.clone()))
                    .one(txn)
                    .await?;

                if let Some(existing_alias) = existing_alias {
                    let mut active: url_alias::ActiveModel = existing_alias.into();
                    active.target_kind = Set(target_kind.clone());
                    active.target_id = Set(update.target_id);
                    active.canonical_url = Set(canonical_route.clone());
                    active.updated_at = Set(now.into());
                    active.update(txn).await?;
                } else {
                    url_alias::ActiveModel {
                        id: Set(rustok_core::generate_id()),
                        tenant_id: Set(tenant_id),
                        target_kind: Set(target_kind.clone()),
                        target_id: Set(update.target_id),
                        locale: Set(locale.clone()),
                        alias_url: Set(alias.clone()),
                        canonical_url: Set(canonical_route.clone()),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(txn)
                    .await?;
                }
            }

            if mapping_changed || !alias_urls.is_empty() {
                self.event_bus
                    .publish_in_tx(
                        txn,
                        tenant_id,
                        actor_id,
                        DomainEvent::CanonicalUrlChanged {
                            target_id: update.target_id,
                            target_kind: target_kind.clone(),
                            locale: locale.clone(),
                            new_canonical_url: canonical_route.clone(),
                            old_urls: alias_urls.clone(),
                        },
                    )
                    .await?;

                if !alias_urls.is_empty() {
                    self.event_bus
                        .publish_in_tx(
                            txn,
                            tenant_id,
                            actor_id,
                            DomainEvent::UrlAliasPurged {
                                target_id: update.target_id,
                                target_kind: target_kind.clone(),
                                locale,
                                urls: alias_urls,
                            },
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn ensure_canonical_route_available(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        canonical_route: &str,
        target_kind: &str,
        target_id: Uuid,
    ) -> ContentResult<()> {
        let existing_canonical = canonical_url::Entity::find()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::Locale.eq(locale.to_string()))
            .filter(canonical_url::Column::CanonicalUrl.eq(canonical_route.to_string()))
            .one(txn)
            .await?;

        if let Some(existing) = existing_canonical
            && (existing.target_kind != target_kind || existing.target_id != target_id)
        {
            return Err(ContentError::validation(format!(
                "canonical_url `{canonical_route}` already belongs to another content target"
            )));
        }

        let existing_alias = url_alias::Entity::find()
            .filter(url_alias::Column::TenantId.eq(tenant_id))
            .filter(url_alias::Column::Locale.eq(locale.to_string()))
            .filter(url_alias::Column::AliasUrl.eq(canonical_route.to_string()))
            .one(txn)
            .await?;

        if let Some(existing) = existing_alias
            && (existing.target_kind != target_kind || existing.target_id != target_id)
        {
            return Err(ContentError::validation(format!(
                "canonical_url `{canonical_route}` collides with an alias owned by another content target"
            )));
        }

        Ok(())
    }

    async fn ensure_alias_route_available(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        alias_route: &str,
        target_kind: &str,
        target_id: Uuid,
    ) -> ContentResult<()> {
        let existing_canonical = canonical_url::Entity::find()
            .filter(canonical_url::Column::TenantId.eq(tenant_id))
            .filter(canonical_url::Column::Locale.eq(locale.to_string()))
            .filter(canonical_url::Column::CanonicalUrl.eq(alias_route.to_string()))
            .one(txn)
            .await?;

        if let Some(existing) = existing_canonical
            && (existing.target_kind != target_kind || existing.target_id != target_id)
        {
            return Err(ContentError::validation(format!(
                "alias_url `{alias_route}` would shadow another target canonical URL"
            )));
        }

        Ok(())
    }
}

fn normalize_route(route: &str) -> ContentResult<String> {
    let route = route.trim();
    if route.is_empty() {
        return Err(ContentError::validation("route must not be empty"));
    }
    if route.len() > 512 {
        return Err(ContentError::validation("route must be <= 512 chars"));
    }
    if !route.starts_with('/') {
        return Err(ContentError::validation("route must start with `/`"));
    }
    if route.chars().any(char::is_whitespace) || route.contains("://") {
        return Err(ContentError::validation(
            "route must be a relative path without whitespace or scheme",
        ));
    }
    Ok(route.to_string())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectOptions, Database};

    use super::*;

    async fn test_db() -> DatabaseConnection {
        let db_url = format!(
            "sqlite:file:canonical_url_service_{}?mode=memory&cache=shared",
            Uuid::new_v4()
        );
        let mut opts = ConnectOptions::new(db_url);
        opts.max_connections(5)
            .min_connections(1)
            .sqlx_logging(false);
        Database::connect(opts)
            .await
            .expect("failed to connect sqlite db")
    }

    async fn seed_tables(db: &DatabaseConnection) {
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE content_canonical_urls (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                target_kind TEXT NOT NULL,
                target_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                canonical_url TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create canonical table");

        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE content_url_aliases (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                target_kind TEXT NOT NULL,
                target_id TEXT NOT NULL,
                locale TEXT NOT NULL,
                alias_url TEXT NOT NULL,
                canonical_url TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create alias table");
    }

    #[tokio::test]
    async fn resolves_alias_to_redirect_target() {
        let db = test_db().await;
        seed_tables(&db).await;
        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();

        canonical_url::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_kind: Set("blog_post".to_string()),
            target_id: Set(target_id),
            locale: Set("en-US".to_string()),
            canonical_url: Set("/modules/blog?slug=release-notes".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert canonical");

        url_alias::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_kind: Set("blog_post".to_string()),
            target_id: Set(target_id),
            locale: Set("en-US".to_string()),
            alias_url: Set("/modules/forum?topic=old".to_string()),
            canonical_url: Set("/modules/blog?slug=release-notes".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert alias");

        let service = CanonicalUrlService::new(db);
        let resolved = service
            .resolve_route(tenant_id, "EN_us", "/modules/forum?topic=old")
            .await
            .expect("resolve alias")
            .expect("alias should resolve");

        assert_eq!(resolved.target_kind, "blog_post");
        assert_eq!(resolved.target_id, target_id);
        assert_eq!(resolved.locale, "en-US");
        assert_eq!(resolved.matched_url, "/modules/forum?topic=old");
        assert_eq!(resolved.canonical_url, "/modules/blog?slug=release-notes");
        assert!(resolved.redirect_required);
    }

    #[tokio::test]
    async fn resolves_alias_via_platform_locale_fallback() {
        let db = test_db().await;
        seed_tables(&db).await;
        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();

        canonical_url::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_kind: Set("blog_post".to_string()),
            target_id: Set(target_id),
            locale: Set("en".to_string()),
            canonical_url: Set("/modules/blog?slug=release-notes".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert canonical");

        url_alias::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_kind: Set("blog_post".to_string()),
            target_id: Set(target_id),
            locale: Set("en".to_string()),
            alias_url: Set("/modules/forum?topic=old".to_string()),
            canonical_url: Set("/modules/blog?slug=release-notes".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert alias");

        let service = CanonicalUrlService::new(db);
        let resolved = service
            .resolve_route(tenant_id, "EN_us", "/modules/forum?topic=old")
            .await
            .expect("resolve alias with fallback")
            .expect("alias should resolve through platform fallback");

        assert_eq!(resolved.target_kind, "blog_post");
        assert_eq!(resolved.target_id, target_id);
        assert_eq!(resolved.locale, "en");
        assert_eq!(resolved.canonical_url, "/modules/blog?slug=release-notes");
        assert!(resolved.redirect_required);
    }

    #[tokio::test]
    async fn resolves_canonical_without_redirect() {
        let db = test_db().await;
        seed_tables(&db).await;
        let tenant_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        let now = chrono::Utc::now().fixed_offset();

        canonical_url::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            target_kind: Set("forum_topic".to_string()),
            target_id: Set(target_id),
            locale: Set("ru".to_string()),
            canonical_url: Set("/modules/forum?topic=canonical".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await
        .expect("insert canonical");

        let service = CanonicalUrlService::new(db);
        let resolved = service
            .resolve_route(tenant_id, "ru", "/modules/forum?topic=canonical")
            .await
            .expect("resolve canonical")
            .expect("canonical should resolve");

        assert_eq!(resolved.target_kind, "forum_topic");
        assert_eq!(resolved.target_id, target_id);
        assert_eq!(resolved.canonical_url, "/modules/forum?topic=canonical");
        assert!(!resolved.redirect_required);
    }
}
