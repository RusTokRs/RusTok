use sea_orm::prelude::*;

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
    pub async fn find_by_hash(
        db: &DatabaseConnection,
        code_hash: &str,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(oauth_authorization_codes::Column::CodeHash.eq(code_hash))
            .one(db)
            .await
    }
}
