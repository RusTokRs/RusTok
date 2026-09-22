use async_trait::async_trait;
use rustok_iggy_connector::{
    IggyConnectorConfigurationSnapshot, IggyConnectorControl, IggyConnectorSettingsInput,
    IggyConnectorUpdateOutcome,
};
use uuid::Uuid;

use crate::services::iggy_connector_settings_service::IggyConnectorSettingsService;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
pub struct ServerIggyConnectorControl {
    runtime: ServerRuntimeContext,
}

impl ServerIggyConnectorControl {
    pub fn new(runtime: ServerRuntimeContext) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl IggyConnectorControl for ServerIggyConnectorControl {
    async fn configuration(&self) -> Result<IggyConnectorConfigurationSnapshot, String> {
        IggyConnectorSettingsService::configuration(&self.runtime)
            .await
            .map_err(|error| error.to_string())
    }

    async fn update_configuration(
        &self,
        input: IggyConnectorSettingsInput,
        actor_id: Uuid,
        actor_tenant_id: Uuid,
    ) -> Result<IggyConnectorUpdateOutcome, String> {
        IggyConnectorSettingsService::save(&self.runtime, input, actor_id, actor_tenant_id)
            .await
            .map_err(|error| error.to_string())?;
        let snapshot = IggyConnectorSettingsService::configuration(&self.runtime)
            .await
            .map_err(|error| error.to_string())?;
        Ok(IggyConnectorUpdateOutcome {
            desired_mode: snapshot.desired_mode,
            configured: snapshot.configured,
            restart_required: snapshot.restart_required,
        })
    }
}
