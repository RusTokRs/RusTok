use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, prelude::*, sea_query::Expr};

use super::_entities::oauth_authorization_codes::{self};
pub use super::_entities::oauth_authorization_codes::{ActiveModel, Column, Entity, Model};

impl Model {
    pub fn is_active(&self) -> bool {
        let now_utc = chrono::Utc::now();
        let expires_at_utc = self.expires_at.with_timezone(&chrono::Utc);

        self.used_at.is_none() && expires_at_utc > now_utc
    }

    pub fn scopes_list(&self) -> Vec<String> {
        self.scopes
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Entity {
    /// Atomically reserve an unused, unexpired authorization code for exchange.
    ///
    /// Only one concurrent caller can move `used_at` from NULL. The returned
    /// model is a one-shot exchange lease: `used_at` is cleared in-memory so the
    /// existing validation path can continue, while the persisted row remains
    /// consumed and all competing exchanges fail closed.
    pub async fn find_by_hash(
        db: &DatabaseConnection,
        code_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        let now = chrono::Utc::now();
        let reserved = Self::update_many()
            .col_expr(oauth_authorization_codes::Column::UsedAt, Expr::value(now))
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .filter(oauth_authorization_codes::Column::UsedAt.is_null())
            .filter(oauth_authorization_codes::Column::ExpiresAt.gt(now))
            .exec(db)
            .await?;

        if reserved.rows_affected != 1 {
            return Ok(None);
        }

        let mut code = Self::find()
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .one(db)
            .await?;
        if let Some(code) = code.as_mut() {
            code.used_at = None;
        }
        Ok(code)
    }
}
