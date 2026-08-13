use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_content::normalize_locale_code;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    dto::{TaxonomyScopeType, TaxonomyTermKind},
    entities::taxonomy_term_route_key,
    error::{TaxonomyError, TaxonomyResult},
    services::TaxonomyService,
};

impl TaxonomyService {
    /// Resolves a localized route key for a module consumer without applying
    /// Taxonomy RBAC a second time. Consumer modules are responsible for
    /// authorizing their own read operation before calling this method.
    ///
    /// Resolution follows the public Taxonomy route contract: module scope
    /// before global scope, then requested locale, explicit fallback locale and
    /// the platform fallback locale. Route ownership is read from the
    /// storage-atomic route-key registry rather than reconstructed from
    /// translation and alias tables.
    #[instrument(skip(self))]
    pub async fn resolve_term_id_for_module(
        &self,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        module_slug: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        slug_or_alias: &str,
    ) -> TaxonomyResult<Option<Uuid>> {
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let module_scope = normalize_module_scope(module_slug)?;
        let route_key = slug::slugify(slug_or_alias);
        if route_key.is_empty() {
            return Ok(None);
        }

        let locales = resolve_locale_candidates(&locale, fallback_locale.as_deref());
        for (scope_type, scope_value) in [
            (TaxonomyScopeType::Module, module_scope.as_str()),
            (TaxonomyScopeType::Global, ""),
        ] {
            for candidate_locale in &locales {
                if let Some(route) = taxonomy_term_route_key::Entity::find()
                    .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
                    .filter(taxonomy_term_route_key::Column::Kind.eq(kind))
                    .filter(taxonomy_term_route_key::Column::ScopeType.eq(scope_type))
                    .filter(taxonomy_term_route_key::Column::ScopeValue.eq(scope_value))
                    .filter(taxonomy_term_route_key::Column::Locale.eq(candidate_locale))
                    .filter(taxonomy_term_route_key::Column::RouteKey.eq(&route_key))
                    .one(self.database())
                    .await?
                {
                    return Ok(Some(route.term_id));
                }
            }
        }

        Ok(None)
    }
}

fn normalize_locale(locale: &str) -> TaxonomyResult<String> {
    normalize_locale_code(locale).ok_or_else(|| TaxonomyError::validation("Invalid locale"))
}

fn normalize_module_scope(module_slug: &str) -> TaxonomyResult<String> {
    let value = module_slug
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect::<String>();
    if value.is_empty() {
        return Err(TaxonomyError::validation(
            "Module scope requires a non-empty scope_value",
        ));
    }
    Ok(value)
}

fn resolve_locale_candidates(locale: &str, fallback_locale: Option<&str>) -> Vec<String> {
    let mut candidates = vec![locale.to_string()];
    if let Some(fallback_locale) = fallback_locale
        && fallback_locale != locale
    {
        candidates.push(fallback_locale.to_string());
    }
    if locale != PLATFORM_FALLBACK_LOCALE && fallback_locale != Some(PLATFORM_FALLBACK_LOCALE) {
        candidates.push(PLATFORM_FALLBACK_LOCALE.to_string());
    }
    candidates
}

#[cfg(test)]
mod tests {
    use rustok_core::{MigrationSource, SecurityContext, UserRole};
    use rustok_test_utils::db::setup_test_db;
    use sea_orm::DatabaseConnection;
    use sea_orm_migration::prelude::SchemaManager;

    use super::*;
    use crate::{CreateTaxonomyTermInput, TaxonomyModule};

    async fn setup() -> (DatabaseConnection, TaxonomyService) {
        let db = setup_test_db().await;
        let schema_manager = SchemaManager::new(&db);
        for migration in TaxonomyModule.migrations() {
            migration
                .up(&schema_manager)
                .await
                .expect("failed to run taxonomy migration");
        }
        let service = TaxonomyService::new(db.clone());
        (db, service)
    }

    fn admin() -> SecurityContext {
        SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
    }

    async fn create_tag(
        service: &TaxonomyService,
        tenant_id: Uuid,
        scope_type: TaxonomyScopeType,
        scope_value: Option<&str>,
        locale: &str,
        name: &str,
        slug: &str,
    ) -> Uuid {
        service
            .create_term(
                tenant_id,
                admin(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type,
                    scope_value: scope_value.map(str::to_string),
                    locale: locale.to_string(),
                    name: name.to_string(),
                    slug: Some(slug.to_string()),
                    canonical_key: Some(format!("{}-{locale}", slug::slugify(name))),
                    description: None,
                    aliases: Vec::new(),
                },
            )
            .await
            .expect("tag should be created")
    }

    #[tokio::test]
    async fn consumer_lookup_prefers_module_scope_over_global_scope() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let global_id = create_tag(
            &service,
            tenant_id,
            TaxonomyScopeType::Global,
            None,
            "en",
            "Global Rust",
            "rust",
        )
        .await;
        let module_id = create_tag(
            &service,
            tenant_id,
            TaxonomyScopeType::Module,
            Some("blog"),
            "en",
            "Blog Rust",
            "rust",
        )
        .await;

        let resolved = service
            .resolve_term_id_for_module(
                tenant_id,
                TaxonomyTermKind::Tag,
                "blog",
                "en",
                None,
                "rust",
            )
            .await
            .expect("route lookup should succeed");

        assert_eq!(resolved, Some(module_id));
        assert_ne!(resolved, Some(global_id));
    }

    #[tokio::test]
    async fn consumer_lookup_keeps_identical_route_keys_locale_scoped() {
        let (_db, service) = setup().await;
        let tenant_id = Uuid::new_v4();
        let en_id = create_tag(
            &service,
            tenant_id,
            TaxonomyScopeType::Module,
            Some("blog"),
            "en",
            "English Rust",
            "rust",
        )
        .await;
        let fr_id = create_tag(
            &service,
            tenant_id,
            TaxonomyScopeType::Module,
            Some("blog"),
            "fr",
            "French Rust",
            "rust",
        )
        .await;

        let fr = service
            .resolve_term_id_for_module(
                tenant_id,
                TaxonomyTermKind::Tag,
                "blog",
                "fr",
                Some("en"),
                "rust",
            )
            .await
            .expect("French route lookup should succeed");
        let de_with_en_fallback = service
            .resolve_term_id_for_module(
                tenant_id,
                TaxonomyTermKind::Tag,
                "blog",
                "de",
                Some("en"),
                "rust",
            )
            .await
            .expect("fallback route lookup should succeed");

        assert_eq!(fr, Some(fr_id));
        assert_eq!(de_with_en_fallback, Some(en_id));
    }
}
