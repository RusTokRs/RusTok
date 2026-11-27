use sea_orm::{ConnectionTrait, QuerySelect, QueryTrait};
use sea_orm::prelude::*;

use super::_entities::oauth_consents::{self};
pub use super::_entities::oauth_consents::{ActiveModel, Column, Entity, Model};

impl Model {
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }

    /// Decode the complete stored grant; never silently discard invalid entries.
    pub fn scopes_list(&self) -> Result<Vec<String>, serde_json::Error> {
        serde_json::from_value(self.scopes.clone())
    }
}

impl Entity {
    /// Resolve a token family's consumed original code to the exact grant.
    /// Missing or unbound legacy evidence is never inferred.
    pub async fn matches_consumed_code_generation<C: ConnectionTrait>(
        db: &C,
        code_id: Option<Uuid>,
        consent_id: Uuid,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<bool, DbErr> {
        use super::_entities::oauth_authorization_codes as codes;
        let Some(code_id) = code_id else {
            return Ok(false);
        };
        // Re-observe active consent in the same SQL snapshot as the code.
        // A prior scope read must not hide a replacement between reads.
        let live = Self::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.eq(consent_id))
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::AppId.eq(app_id))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::RevokedAt.is_null())
            .into_query();
        Ok(codes::Entity::find_by_id(code_id)
            .filter(codes::Column::TenantId.eq(tenant_id))
            .filter(codes::Column::AppId.eq(app_id))
            .filter(codes::Column::UserId.eq(user_id))
            .filter(codes::Column::ConsentId.in_subquery(live))
            .filter(codes::Column::UsedAt.is_not_null())
            .one(db)
            .await?
            .is_some())
    }

    pub async fn find_active_consent<C>(
        db: &C,
        app_id: Uuid,
        user_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Self::find()
            .filter(
                sea_orm::Condition::all()
                    .add(oauth_consents::Column::AppId.eq(app_id))
                    .add(oauth_consents::Column::UserId.eq(user_id))
                    .add(oauth_consents::Column::TenantId.eq(tenant_id))
                    .add(oauth_consents::Column::RevokedAt.is_null()),
            )
            .one(db)
            .await
    }
}
