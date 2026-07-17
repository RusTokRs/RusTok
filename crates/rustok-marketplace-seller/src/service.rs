use std::collections::{HashMap, HashSet};

use chrono::Utc;
use rustok_api::{build_locale_candidates, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellersInput, MarketplaceSellerMemberResponse, MarketplaceSellerMemberRole,
    MarketplaceSellerMemberStatus, MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse,
    MarketplaceSellerStatus, UpdateMarketplaceSellerMemberInput,
};
use crate::entities::{seller, seller_member, seller_translation};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};

pub struct MarketplaceSellerService {
    db: DatabaseConnection,
}

impl MarketplaceSellerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn get_seller(
        &self,
        tenant_id: Uuid,
        seller_id: Uuid,
        locale: &str,
    ) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
        load_seller_response(&self.db, tenant_id, seller_id, locale).await
    }

    pub async fn list_sellers(
        &self,
        tenant_id: Uuid,
        locale: &str,
        input: ListMarketplaceSellersInput,
    ) -> MarketplaceSellerResult<(Vec<MarketplaceSellerResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let candidates = seller_locale_candidates(locale)?;
        let mut query = seller::Entity::find().filter(seller::Column::TenantId.eq(tenant_id));

        if let Some(status) = input.status {
            query = query.filter(seller::Column::Status.eq(status.as_str()));
        }
        if let Some(status) = input.onboarding_status {
            query = query.filter(seller::Column::OnboardingStatus.eq(status.as_str()));
        }
        if let Some(search) = input
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let translation_ids = seller_translation::Entity::find()
                .filter(seller_translation::Column::TenantId.eq(tenant_id))
                .filter(seller_translation::Column::Locale.is_in(candidates.clone()))
                .filter(seller_translation::Column::DisplayName.contains(search))
                .all(&self.db)
                .await?
                .into_iter()
                .map(|translation| translation.seller_id)
                .collect::<HashSet<_>>();
            let mut condition = Condition::any()
                .add(seller::Column::Handle.contains(search))
                .add(seller::Column::LegalName.contains(search));
            if !translation_ids.is_empty() {
                condition = condition.add(seller::Column::Id.is_in(translation_ids));
            }
            query = query.filter(condition);
        }

        let paginator = query
            .order_by_desc(seller::Column::UpdatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let items = load_seller_responses(&self.db, models, locale).await?;
        Ok((items, total))
    }

    pub async fn get_membership(
        &self,
        tenant_id: Uuid,
        seller_id: Uuid,
        user_id: Uuid,
    ) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
        let model = seller_member::Entity::find()
            .filter(seller_member::Column::TenantId.eq(tenant_id))
            .filter(seller_member::Column::SellerId.eq(seller_id))
            .filter(seller_member::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplaceSellerError::MembershipNotFound { seller_id, user_id })?;
        map_member(model)
    }

    pub async fn list_members(
        &self,
        tenant_id: Uuid,
        seller_id: Uuid,
    ) -> MarketplaceSellerResult<Vec<MarketplaceSellerMemberResponse>> {
        find_seller(&self.db, tenant_id, seller_id).await?;
        seller_member::Entity::find()
            .filter(seller_member::Column::TenantId.eq(tenant_id))
            .filter(seller_member::Column::SellerId.eq(seller_id))
            .order_by_asc(seller_member::Column::CreatedAt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_member)
            .collect()
    }
}

pub(crate) async fn find_seller<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
) -> MarketplaceSellerResult<seller::Model> {
    seller::Entity::find_by_id(seller_id)
        .filter(seller::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(MarketplaceSellerError::SellerNotFound(seller_id))
}

pub(crate) async fn load_seller_response<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let model = find_seller(connection, tenant_id, seller_id).await?;
    let translation = resolve_translation(connection, tenant_id, seller_id, locale).await?;
    map_seller(model, translation)
}

async fn load_seller_responses<C: ConnectionTrait>(
    connection: &C,
    models: Vec<seller::Model>,
    locale: &str,
) -> MarketplaceSellerResult<Vec<MarketplaceSellerResponse>> {
    if models.is_empty() {
        return Ok(Vec::new());
    }
    let normalized = normalize_seller_locale(locale)?;
    let candidates = seller_locale_candidates(normalized.as_str())?;
    let tenant_id = models[0].tenant_id;
    let seller_ids = models.iter().map(|model| model.id).collect::<Vec<_>>();
    let translations = seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.is_in(seller_ids))
        .filter(seller_translation::Column::Locale.is_in(candidates.clone()))
        .all(connection)
        .await?
        .into_iter()
        .map(|translation| ((translation.seller_id, translation.locale.clone()), translation))
        .collect::<HashMap<_, _>>();

    models
        .into_iter()
        .map(|model| {
            let translation = candidates
                .iter()
                .find_map(|candidate| translations.get(&(model.id, candidate.clone())).cloned())
                .ok_or_else(|| MarketplaceSellerError::TranslationNotFound {
                    seller_id: model.id,
                    locale: normalized.clone(),
                })?;
            map_seller(model, translation)
        })
        .collect()
}

async fn resolve_translation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
) -> MarketplaceSellerResult<seller_translation::Model> {
    let normalized = normalize_seller_locale(locale)?;
    let candidates = seller_locale_candidates(normalized.as_str())?;
    let translations = seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .filter(seller_translation::Column::Locale.is_in(candidates.clone()))
        .all(connection)
        .await?
        .into_iter()
        .map(|translation| (translation.locale.clone(), translation))
        .collect::<HashMap<_, _>>();
    candidates
        .iter()
        .find_map(|candidate| translations.get(candidate).cloned())
        .ok_or(MarketplaceSellerError::TranslationNotFound {
            seller_id,
            locale: normalized,
        })
}

pub(crate) async fn upsert_translation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
    display_name: String,
) -> MarketplaceSellerResult<seller_translation::Model> {
    let locale = normalize_seller_locale(locale)?;
    let now = Utc::now();
    if let Some(existing) = seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .filter(seller_translation::Column::Locale.eq(locale.as_str()))
        .one(connection)
        .await?
    {
        let mut active: seller_translation::ActiveModel = existing.into();
        active.display_name = Set(display_name);
        active.updated_at = Set(now.into());
        return active.update(connection).await.map_err(Into::into);
    }
    seller_translation::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        seller_id: Set(seller_id),
        locale: Set(locale),
        display_name: Set(display_name),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(connection)
    .await
    .map_err(Into::into)
}

pub(crate) fn map_seller(
    model: seller::Model,
    translation: seller_translation::Model,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let status = MarketplaceSellerStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller status `{}`",
            model.status
        ))
    })?;
    let onboarding_status =
        MarketplaceSellerOnboardingStatus::parse(model.onboarding_status.as_str()).ok_or_else(
            || {
                MarketplaceSellerError::Validation(format!(
                    "unknown marketplace seller onboarding status `{}`",
                    model.onboarding_status
                ))
            },
        )?;
    Ok(MarketplaceSellerResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        handle: model.handle,
        resolved_locale: translation.locale,
        display_name: translation.display_name,
        legal_name: model.legal_name,
        status,
        onboarding_status,
        onboarding_note: model.onboarding_note,
        suspension_reason: model.suspension_reason,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
        activated_at: model.activated_at,
        suspended_at: model.suspended_at,
    })
}

pub(crate) fn map_member(
    model: seller_member::Model,
) -> MarketplaceSellerResult<MarketplaceSellerMemberResponse> {
    let role = MarketplaceSellerMemberRole::parse(model.role.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller member role `{}`",
            model.role
        ))
    })?;
    let status = MarketplaceSellerMemberStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller member status `{}`",
            model.status
        ))
    })?;
    Ok(MarketplaceSellerMemberResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        user_id: model.user_id,
        role,
        status,
        invited_by_actor_id: model.invited_by_actor_id,
        accepted_at: model.accepted_at,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

pub(crate) fn validate_owner_membership_update(
    current: &seller_member::Model,
    input: &UpdateMarketplaceSellerMemberInput,
) -> MarketplaceSellerResult<()> {
    if current.role == MarketplaceSellerMemberRole::Owner.as_str()
        && matches!(
            input.role,
            Some(role) if role != MarketplaceSellerMemberRole::Owner
        )
    {
        return Err(MarketplaceSellerError::Validation(
            "owner membership role cannot be changed".to_string(),
        ));
    }
    if current.role == MarketplaceSellerMemberRole::Owner.as_str()
        && input.status == Some(MarketplaceSellerMemberStatus::Disabled)
    {
        return Err(MarketplaceSellerError::Validation(
            "owner membership cannot be disabled".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn normalize_seller_locale(value: &str) -> MarketplaceSellerResult<String> {
    normalize_locale_tag(value).ok_or_else(|| {
        MarketplaceSellerError::Validation(
            "locale must be a normalized BCP47-like tag with at most 32 bytes".to_string(),
        )
    })
}

fn seller_locale_candidates(value: &str) -> MarketplaceSellerResult<Vec<String>> {
    let normalized = normalize_seller_locale(value)?;
    Ok(build_locale_candidates(
        [Some(normalized.as_str()), Some(PLATFORM_FALLBACK_LOCALE)],
        true,
    ))
}

pub(crate) fn normalize_handle(value: &str) -> MarketplaceSellerResult<String> {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.len() < 2
        || normalized.len() > 80
        || normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
        })
    {
        return Err(MarketplaceSellerError::Validation(
            "handle must contain 2 to 80 lowercase ASCII letters, digits, or internal hyphens"
                .to_string(),
        ));
    }
    Ok(normalized)
}

pub(crate) fn required_text(
    value: String,
    field: &str,
) -> MarketplaceSellerResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MarketplaceSellerError::Validation(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.to_string())
}

pub(crate) fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

pub(crate) fn object_or_empty(
    value: serde_json::Value,
    field: &str,
) -> MarketplaceSellerResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceSellerError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

pub(crate) fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
