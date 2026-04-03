use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;

use crate::{
    dto::{
        CreateShippingProfileInput, ListShippingProfilesInput, ShippingProfileResponse,
        UpdateShippingProfileInput,
    },
    entities::shipping_profile,
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

        let now = Utc::now();
        let id = generate_id();
        shipping_profile::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            slug: Set(slug),
            name: Set(input.name.trim().to_string()),
            description: Set(normalize_optional_text(input.description)),
            active: Set(true),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        self.get_shipping_profile(tenant_id, id).await
    }

    pub async fn list_shipping_profiles(
        &self,
        tenant_id: Uuid,
        input: ListShippingProfilesInput,
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
            query = query.filter(
                shipping_profile::Column::Slug
                    .contains(search)
                    .or(shipping_profile::Column::Name.contains(search)),
            );
        }

        let total = query.clone().count(&self.db).await?;
        let rows = query
            .order_by_asc(shipping_profile::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        Ok((rows.into_iter().map(map_shipping_profile).collect(), total))
    }

    pub async fn get_shipping_profile(
        &self,
        tenant_id: Uuid,
        shipping_profile_id: Uuid,
    ) -> CommerceResult<ShippingProfileResponse> {
        let row = self
            .load_shipping_profile(tenant_id, shipping_profile_id)
            .await?;
        Ok(map_shipping_profile(row))
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
        if let Some(name) = input.name {
            let name = name.trim();
            if name.is_empty() {
                return Err(CommerceError::Validation(
                    "shipping profile name cannot be empty".into(),
                ));
            }
            active.name = Set(name.to_string());
        }
        if input.description.is_some() {
            active.description = Set(normalize_optional_text(input.description));
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }

        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_shipping_profile(tenant_id, shipping_profile_id)
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

        self.get_shipping_profile(tenant_id, shipping_profile_id)
            .await
    }
}

fn map_shipping_profile(value: shipping_profile::Model) -> ShippingProfileResponse {
    ShippingProfileResponse {
        id: value.id,
        tenant_id: value.tenant_id,
        slug: value.slug,
        name: value.name,
        description: value.description,
        active: value.active,
        metadata: value.metadata,
        created_at: value.created_at.into(),
        updated_at: value.updated_at.into(),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
