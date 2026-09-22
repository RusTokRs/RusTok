use rustok_mcp::{
    ModuleScaffoldDraftStatus, ScaffoldModulePreview, ScaffoldModuleRequest, StagedModuleScaffold,
};
use sea_orm::{QueryFilter, QueryOrder, QuerySelect, entity::prelude::*};

pub use super::_entities::mcp_scaffold_drafts::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::TenantId.eq(tenant_id))
            .order_by_desc(Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
    }
}

impl Model {
    pub fn status_value(&self) -> ModuleScaffoldDraftStatus {
        match self.status.as_str() {
            "applied" => ModuleScaffoldDraftStatus::Applied,
            _ => ModuleScaffoldDraftStatus::Staged,
        }
    }

    pub fn request(&self) -> Result<ScaffoldModuleRequest, serde_json::Error> {
        serde_json::from_value(self.request_payload.clone())
    }

    pub fn preview(&self) -> Result<ScaffoldModulePreview, serde_json::Error> {
        serde_json::from_value(self.preview_payload.clone())
    }

    pub fn to_staged_draft(&self) -> Result<StagedModuleScaffold, serde_json::Error> {
        Ok(StagedModuleScaffold {
            draft_id: self.id.to_string(),
            request: self.request()?,
            preview: self.preview()?,
            status: self.status_value(),
        })
    }
}
