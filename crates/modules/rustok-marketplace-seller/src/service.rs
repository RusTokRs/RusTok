use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellersInput, MarketplaceSellerEventResponse, MarketplaceSellerMemberResponse,
    MarketplaceSellerMemberRole, MarketplaceSellerMemberStatus, MarketplaceSellerResponse,
    UpdateMarketplaceSellerMemberInput,
};
use crate::entities::{seller, seller_member};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::localized_sellers::{load_seller_responses, localized_seller_ids_for_search};
use crate::seller_events::list_seller_events;

pub(crate) use crate::localized_sellers::{
    MISSING_TRANSLATION_PREFIX, load_seller_response, normalize_seller_locale, upsert_translation,
};

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
        let locale = normalize_seller_locale(locale)?;
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
            let translation_ids =
                localized_seller_ids_for_search(&self.db, tenant_id, locale.as_str(), search)
                    .await?;
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
        let items = load_seller_responses(&self.db, models, locale.as_str()).await?;
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

    pub async fn list_events(
        &self,
        tenant_id: Uuid,
        seller_id: Uuid,
        limit: u64,
    ) -> MarketplaceSellerResult<Vec<MarketplaceSellerEventResponse>> {
        find_seller(&self.db, tenant_id, seller_id).await?;
        list_seller_events(&self.db, tenant_id, seller_id, limit).await
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

pub(crate) fn required_text(value: String, field: &str) -> MarketplaceSellerResult<String> {
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
