use tracing::instrument;
use uuid::Uuid;

use rustok_tenant::TenantService;

use crate::error::{PagesError, PagesResult};

use super::PageService;

pub const FEATURE_BUILDER_INLINE_EDIT_ENABLED: &str = "pages.builder.inline_edit.enabled";

impl PageService {
    #[instrument(skip(self))]
    pub async fn ensure_builder_inline_edit_enabled_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> PagesResult<()> {
        let settings = TenantService::new(self.db.clone())
            .find_tenant_module(tenant_id, "pages")
            .await?
            .map(|module| module.settings);
        let enabled = settings
            .as_ref()
            .and_then(|settings| settings.get("builder"))
            .and_then(|builder| builder.get("inline_edit"))
            .and_then(|inline_edit| inline_edit.get("enabled"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !enabled {
            return Err(PagesError::feature_disabled(
                FEATURE_BUILDER_INLINE_EDIT_ENABLED,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_edit_feature_name_is_stable() {
        assert_eq!(
            FEATURE_BUILDER_INLINE_EDIT_ENABLED,
            "pages.builder.inline_edit.enabled"
        );
    }
}
