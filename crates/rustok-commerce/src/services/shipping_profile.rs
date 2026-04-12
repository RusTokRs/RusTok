use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Value,
};
use sea_orm::sea_query::Expr;
use sea_orm::Condition;
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::{generate_id, normalize_locale_tag};

use crate::{
    dto::{
        CreateShippingProfileInput, ListShippingProfilesInput, ShippingProfileResponse,
        ShippingProfileTranslationInput, ShippingProfileTranslationResponse,
        UpdateShippingProfileInput,
    },
    entities::{shipping_profile, shipping_profile_translation},
    storefront_shipping::normalize_shipping_profile_slug,
    CommerceError, CommerceResult,
};

pub struct ShippingProfileService {
    db: DatabaseConnection,
}

impl ShippingProfileService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_shipping_profile(
        &self,
        tenant_id: Uuid,
        input: CreateShippingProfileInput,
    ) -> CommerceResult<ShippingProfileResponse> {
        input
            .validate()
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let slug = normalize_shipping_profile_slug(&input.slug)
            .ok_or_else(|| CommerceError::Validation("shipping profile slug is required".into()))?;
        self.ensure_slug_available(tenant_id, &slug, None).await?;
        let normalized_translations = normalize_translation_inputs(input.translations)?;

        let now = Utc::now();
        let id = generate_id();
        let active_profile = shipping_profile::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            slug: Set(slug),
            active: Set(true),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        active_profile.insert(&self.db).await?;

        insert_translations(&self.db, id, &normalized_translations).await?;

        self.get_shipping_profile(tenant_id, id, None, None).await
    }

    pub async fn list_shipping_profiles(
        &self,
        tenant_id: Uuid,
        input: ListShippingProfilesInput,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CommerceResult<(Vec<ShippingProfileResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = shipping_profile::Entity::find()
            .filter(shipping_profile::Column::TenantId.eq(tenant_id));

        if let Some(active) = input.active {
            query = query.filter(shipping_profile::Column::Active.eq(active));
        }
        if let Some(search) = input
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let backend = self.db.get_database_backend();
            let condition =
                shipping_profile_translation_search_condition(backend, search.trim());
            query = query.filter(
                Condition::any()
                    .add(shipping_profile::Column::Slug.contains(search))
                    .add(condition),
            );
        }

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .order_by_asc(shipping_profile::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        let items = load_profiles_with_translations(
            &self.db,
            rows,
            requested_locale.or_else(|| input.locale.as_deref()),
            tenant_default_locale,
        )
        .await?;

        Ok((items, total))
    }

    pub async fn get_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CommerceResult<ShippingProfileResponse> {
        let row = self
            .load_shipping_profile(tenant_id, shipping_profile_id)
            .await?;
        let items = load_profiles_with_translations(
            &self.db,
            vec![row],
            requested_locale,
            tenant_default_locale,
        )
        .await?;
        items
            .into_iter()
            .next()
            .ok_or_else(|| CommerceError::ShippingProfileNotFound(shipping_profile_id))
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, shipping_profile_id = %shipping_profile_id))]
    pub async fn update_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
        input: UpdateShippingProfileInput,
    ) -> CommerceResult<ShippingProfileResponse> {
        input
            .validate()
            .map_err(|error| CommerceError::Validation(error.to_string()))?;

        let row = self
            .load_shipping_profile(tenant_id, shipping_profile_id)
            .await?;
        let mut active: shipping_profile::ActiveModel = row.into();

        if let Some(slug) = input.slug {
            let slug = normalize_shipping_profile_slug(&slug).ok_or_else(|| {
                CommerceError::Validation("shipping profile slug cannot be empty".into())
            })?;
            self.ensure_slug_available(tenant_id, &slug, Some(shipping_profile_id))
                .await?;
            active.slug = Set(slug);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }

        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        if let Some(translations) = input.translations {
            let normalized = normalize_translation_inputs(translations)?;
            replace_translations(&self.db, shipping_profile_id, &normalized).await?;
        }

        self.get_shipping_profile(tenant_id, shipping_profile_id, None, None)
            .await
    }

    pub async fn deactivate_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
    ) -> CommerceResult<ShippingProfileResponse> {
        self.set_shipping_profile_active(tenant_id, shipping_profile_id, false)
            .await
    }

    pub async fn reactivate_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
    ) -> CommerceResult<ShippingProfileResponse> {
        self.set_shipping_profile_active(tenant_id, shipping_profile_id, true)
            .await
    }

    pub async fn ensure_shipping_profile_slug_exists(
        &self,
        tenant_id: Uuid,
        slug: &str,
    ) -> CommerceResult<()> {
        let slug = normalize_shipping_profile_slug(slug)
            .ok_or_else(|| CommerceError::Validation("shipping profile slug is required".into()))?;
        let exists = shipping_profile::Entity::find()
            .filter(shipping_profile::Column::TenantId.eq(tenant_id))
            .filter(shipping_profile::Column::Slug.eq(slug.clone()))
            .filter(shipping_profile::Column::Active.eq(true))
            .one(&self.db)
            .await?
            .is_some();

        if exists {
            Ok(())
        } else {
            Err(CommerceError::Validation(format!(
                "Unknown shipping profile slug: {slug}"
            )))
        }
    }

    pub async fn ensure_shipping_profile_slugs_exist<I, S>(
        &self,
        tenant_id: Uuid,
        slugs: I,
    ) -> CommerceResult<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for slug in slugs {
            if let Some(normalized) = normalize_shipping_profile_slug(slug.as_ref()) {
                self.ensure_shipping_profile_slug_exists(tenant_id, &normalized)
                    .await?;
            }
        }
        Ok(())
    }

    async fn ensure_slug_available(
        &self,
        tenant_id: Uuid,
        slug: &str,
        current_id: Option<Uuid>,
    ) -> CommerceResult<()> {
        let existing = shipping_profile::Entity::find()
            .filter(shipping_profile::Column::TenantId.eq(tenant_id))
            .filter(shipping_profile::Column::Slug.eq(slug))
            .one(&self.db)
            .await?;

        if existing.is_some_and(|row| Some(row.id) != current_id) {
            return Err(CommerceError::DuplicateShippingProfileSlug(
                slug.to_string(),
            ));
        }

        Ok(())
    }

    async fn load_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
    ) -> CommerceResult<shipping_profile::Model> {
        shipping_profile::Entity::find_by_id(shipping_profile_id)
            .filter(shipping_profile::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CommerceError::ShippingProfileNotFound(shipping_profile_id))
    }

    async fn set_shipping_profile_active(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
        active: bool,
    ) -> CommerceResult<ShippingProfileResponse> {
        let row = self
            .load_shipping_profile(tenant_id, shipping_profile_id)
            .await?;
        let mut model: shipping_profile::ActiveModel = row.into();
        model.active = Set(active);
        model.updated_at = Set(Utc::now().into());
        model.update(&self.db).await?;

        self.get_shipping_profile(tenant_id, shipping_profile_id, None, None)
            .await
    }
}

async fn load_profiles_with_translations(
    db: &DatabaseConnection,
    rows: Vec<shipping_profile::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> CommerceResult<Vec<ShippingProfileResponse>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let translations = shipping_profile_translation::Entity::find()
        .filter(shipping_profile_translation::Column::ShippingProfileId.is_in(ids.clone()))
        .all(db)
        .await?;

    let mut translations_by_profile: HashMap<Uuid, Vec<shipping_profile_translation::Model>> =
        HashMap::new();
    for translation in translations {
        translations_by_profile
            .entry(translation.shipping_profile_id)
            .or_default()
            .push(translation);
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let translations = translations_by_profile
                .remove(&row.id)
                .unwrap_or_default();
            map_shipping_profile(
                row,
                translations,
                requested_locale,
                tenant_default_locale,
            )
        })
        .collect())
}

fn map_shipping_profile(
    value: shipping_profile::Model,
    translations: Vec<shipping_profile_translation::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ShippingProfileResponse {
    let available_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    let requested_locale = requested_locale
        .and_then(normalize_locale_tag)
        .filter(|value| !value.is_empty());
    let (resolved, effective_locale) =
        resolve_translation(&translations, requested_locale.as_deref(), tenant_default_locale);

    let (name, description) = resolved
        .map(|translation| (translation.name.clone(), translation.description.clone()))
        .unwrap_or_else(|| ("".to_string(), None));

    ShippingProfileResponse {
        id: value.id,
        tenant_id: value.tenant_id,
        slug: value.slug,
        name,
        description,
        active: value.active,
        metadata: value.metadata,
        created_at: value.created_at.into(),
        updated_at: value.updated_at.into(),
        requested_locale,
        effective_locale,
        available_locales,
        translations: translations
            .into_iter()
            .map(|translation| ShippingProfileTranslationResponse {
                locale: translation.locale,
                name: translation.name,
                description: translation.description,
            })
            .collect(),
    }
}

fn normalize_translation_inputs(
    translations: Vec<ShippingProfileTranslationInput>,
) -> CommerceResult<Vec<ShippingProfileTranslationInput>> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "At least one translation is required".into(),
        ));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_locale_tag(&translation.locale).ok_or_else(|| {
            CommerceError::Validation("Invalid locale for shipping profile translation".into())
        })?;
        if !seen.insert(locale.clone()) {
            return Err(CommerceError::Validation(
                "Duplicate locale in shipping profile translations".into(),
            ));
        }
        let name = translation.name.trim();
        if name.is_empty() {
            return Err(CommerceError::Validation(
                "Shipping profile name cannot be empty".into(),
            ));
        }
        let description = translation
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        normalized.push(ShippingProfileTranslationInput {
            locale,
            name: name.to_string(),
            description,
        });
    }
    Ok(normalized)
}

async fn insert_translations(
    db: &DatabaseConnection,
    shipping_profile_id: Uuid,
    translations: &[ShippingProfileTranslationInput],
) -> CommerceResult<()> {
    for translation in translations {
        shipping_profile_translation::ActiveModel {
            id: Set(generate_id()),
            shipping_profile_id: Set(shipping_profile_id),
            locale: Set(translation.locale.clone()),
            name: Set(translation.name.clone()),
            description: Set(translation.description.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn replace_translations(
    db: &DatabaseConnection,
    shipping_profile_id: Uuid,
    translations: &[ShippingProfileTranslationInput],
) -> CommerceResult<()> {
    shipping_profile_translation::Entity::delete_many()
        .filter(
            shipping_profile_translation::Column::ShippingProfileId.eq(shipping_profile_id),
        )
        .exec(db)
        .await?;
    insert_translations(db, shipping_profile_id, translations).await
}

fn resolve_translation<'a>(
    translations: &'a [shipping_profile_translation::Model],
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> (Option<&'a shipping_profile_translation::Model>, Option<String>) {
    let mut lookup = HashMap::new();
    for translation in translations {
        if let Some(normalized) = normalize_locale_tag(&translation.locale) {
            lookup.insert(normalized, translation);
        }
    }

    if let Some(locale) = requested_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some((*found).locale.clone()));
        }
    }
    if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some((*found).locale.clone()));
        }
    }
    translations
        .first()
        .map(|item| (Some(item), Some(item.locale.clone())))
        .unwrap_or((None, None))
}

fn shipping_profile_translation_search_condition(
    backend: sea_orm::DbBackend,
    search: &str,
) -> Condition {
    let pattern = format!("%{search}%");
    let exists_sql = match backend {
        sea_orm::DbBackend::Sqlite => {
            "EXISTS (
                SELECT 1
                FROM shipping_profile_translations spt
                WHERE spt.shipping_profile_id = shipping_profiles.id
                  AND spt.name LIKE ?1
            )"
        }
        _ => {
            "EXISTS (
                SELECT 1
                FROM shipping_profile_translations spt
                WHERE spt.shipping_profile_id = shipping_profiles.id
                  AND spt.name LIKE $1
            )"
        }
    };

    Condition::all().add(Expr::cust_with_values(exists_sql, vec![Value::from(pattern)]))
}
