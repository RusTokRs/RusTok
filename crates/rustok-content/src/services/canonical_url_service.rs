use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::{canonical_url, url_alias};
use crate::{normalize_locale_code, resolve_by_locale, ContentError, ContentResult};

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

        db.execute(Statement::from_string(
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

        db.execute(Statement::from_string(
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
            locale: Set("en-us".to_string()),
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
            locale: Set("en-us".to_string()),
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
        assert_eq!(resolved.locale, "en-us");
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
