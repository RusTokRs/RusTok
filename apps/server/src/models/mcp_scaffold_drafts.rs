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

fn map_persisted_status(status: &str) -> Result<ModuleScaffoldDraftStatus, String> {
    match status {
        "staged" | "applying" => Ok(ModuleScaffoldDraftStatus::Staged),
        "applied" => Ok(ModuleScaffoldDraftStatus::Applied),
        "recovery_required" => Err(
            "Scaffold draft requires manual workspace reconciliation".to_string(),
        ),
        other => Err(format!(
            "Scaffold draft has unsupported persisted status '{}'",
            other
        )),
    }
}

impl Model {
    pub fn status_value(&self) -> Result<ModuleScaffoldDraftStatus, String> {
        map_persisted_status(&self.status)
    }

    pub fn request(&self) -> Result<ScaffoldModuleRequest, serde_json::Error> {
        serde_json::from_value(self.request_payload.clone())
    }

    pub fn preview(&self) -> Result<ScaffoldModulePreview, serde_json::Error> {
        serde_json::from_value(self.preview_payload.clone())
    }

    /// Convert a claimed draft for the owner crate's apply API. The database
    /// remains authoritative for the applying transition; the owner DTO has
    /// only staged/applied states.
    pub fn to_staged_draft(&self) -> Result<StagedModuleScaffold, String> {
        let status = self.status_value()?;
        let request = self
            .request()
            .map_err(|error| format!("Invalid persisted scaffold request: {error}"))?;
        let preview = self
            .preview()
            .map_err(|error| format!("Invalid persisted scaffold preview: {error}"))?;

        Ok(StagedModuleScaffold {
            draft_id: self.id.to_string(),
            request,
            preview,
            status,
        })
    }

    /// Review responses expose only the owner crate's staged/applied states.
    /// Internal in-flight and recovery states must never be presented as staged.
    pub fn to_reviewable_draft(&self) -> Result<StagedModuleScaffold, String> {
        match self.status.as_str() {
            "staged" | "applied" => self.to_staged_draft(),
            "applying" => Err(
                "Scaffold draft is currently applying and cannot be reviewed until the apply attempt is reconciled"
                    .to_string(),
            ),
            "recovery_required" => Err(
                "Scaffold draft requires manual workspace reconciliation before it can be reviewed"
                    .to_string(),
            ),
            other => Err(format!(
                "Scaffold draft has unsupported persisted status '{}'",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::map_persisted_status;
    use rustok_mcp::ModuleScaffoldDraftStatus;

    #[test]
    fn persisted_status_mapping_fails_closed_for_internal_or_unknown_states() {
        assert_eq!(
            map_persisted_status("staged"),
            Ok(ModuleScaffoldDraftStatus::Staged)
        );
        assert_eq!(
            map_persisted_status("applying"),
            Ok(ModuleScaffoldDraftStatus::Staged)
        );
        assert_eq!(
            map_persisted_status("applied"),
            Ok(ModuleScaffoldDraftStatus::Applied)
        );
        assert!(map_persisted_status("recovery_required").is_err());
        assert!(map_persisted_status("unknown").is_err());
    }
}
