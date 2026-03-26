use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::engine::SearchEngineKind;
use crate::models::SearchSettingsRecord;
use crate::{SearchFilterPresetService, SearchRankingProfile};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "search_settings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub active_engine: String,
    pub fallback_engine: String,
    pub config: Json,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub struct SearchSettingsService;

impl SearchSettingsService {
    pub async fn load_effective(
        db: &DatabaseConnection,
        tenant_id: Option<Uuid>,
    ) -> Result<SearchSettingsRecord, DbErr> {
        if let Some(tenant_id) = tenant_id {
            if let Some(model) = Entity::find()
                .filter(Column::TenantId.eq(tenant_id))
                .order_by_desc(Column::UpdatedAt)
                .one(db)
                .await?
            {
                return map_model(model);
            }
        }

        if let Some(model) = Entity::find()
            .filter(Column::TenantId.is_null())
            .order_by_desc(Column::UpdatedAt)
            .one(db)
            .await?
        {
            return map_model(model);
        }

        Ok(SearchSettingsRecord::default_for_tenant(tenant_id))
    }

    pub async fn save(
        db: &DatabaseConnection,
        tenant_id: Option<Uuid>,
        active_engine: SearchEngineKind,
        fallback_engine: SearchEngineKind,
        config: serde_json::Value,
    ) -> Result<SearchSettingsRecord, DbErr> {
        SearchRankingProfile::validate_config(&config)
            .map_err(|err| DbErr::Custom(err.to_string()))?;
        SearchFilterPresetService::validate_config(&config)
            .map_err(|err| DbErr::Custom(err.to_string()))?;

        let existing = if let Some(tenant_id) = tenant_id {
            Entity::find()
                .filter(Column::TenantId.eq(tenant_id))
                .order_by_desc(Column::UpdatedAt)
                .one(db)
                .await?
        } else {
            Entity::find()
                .filter(Column::TenantId.is_null())
                .order_by_desc(Column::UpdatedAt)
                .one(db)
                .await?
        };

        let now: DateTimeWithTimeZone = chrono::Utc::now().into();

        let model = match existing {
            Some(existing) => {
                let mut active: ActiveModel = existing.into();
                active.active_engine = Set(active_engine.as_str().to_string());
                active.fallback_engine = Set(fallback_engine.as_str().to_string());
                active.config = Set(config.into());
                active.updated_at = Set(now);
                active.update(db).await?
            }
            None => {
                ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    active_engine: Set(active_engine.as_str().to_string()),
                    fallback_engine: Set(fallback_engine.as_str().to_string()),
                    config: Set(config.into()),
                    updated_at: Set(now),
                }
                .insert(db)
                .await?
            }
        };

        map_model(model)
    }
}

fn map_model(model: Model) -> Result<SearchSettingsRecord, DbErr> {
    Ok(SearchSettingsRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        active_engine: parse_engine_value("active_engine", &model.active_engine)?,
        fallback_engine: parse_engine_value("fallback_engine", &model.fallback_engine)?,
        config: model.config,
        updated_at: model.updated_at.into(),
    })
}

fn parse_engine_value(field_name: &str, value: &str) -> Result<SearchEngineKind, DbErr> {
    SearchEngineKind::try_from_str(value).ok_or_else(|| {
        DbErr::Custom(format!(
            "search_settings.{field_name} contains unsupported engine value '{}'",
            value
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{map_model, Model};
    use sea_orm::prelude::Json;
    use uuid::Uuid;

    #[test]
    fn map_model_rejects_invalid_engine_values() {
        let model = Model {
            id: Uuid::new_v4(),
            tenant_id: None,
            active_engine: "bogus".to_string(),
            fallback_engine: "postgres".to_string(),
            config: Json::from(serde_json::json!({})),
            updated_at: chrono::Utc::now().into(),
        };

        let error = map_model(model).expect_err("invalid engine should fail");
        let message = error.to_string();
        assert!(message.contains("active_engine"));
        assert!(message.contains("bogus"));
    }

    #[tokio::test]
    async fn save_rejects_invalid_filter_preset_config() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");

        let error = super::SearchSettingsService::save(
            &db,
            None,
            crate::SearchEngineKind::Postgres,
            crate::SearchEngineKind::Postgres,
            serde_json::json!({
                "filter_presets": {
                    "storefront_search": [
                        { "key": "bad key!", "label": "Broken" }
                    ]
                }
            }),
        )
        .await
        .expect_err("invalid config should fail");

        assert!(error.to_string().contains("invalid characters"));
    }
}
