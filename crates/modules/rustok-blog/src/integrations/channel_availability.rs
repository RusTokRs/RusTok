use std::{sync::Arc, time::Duration};

use rustok_api::{
    HostRuntimeContext, PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, RequestContext,
};
use rustok_channel_ports::{
    CHANNEL_PUBLIC_MODULE_READ_CLAIM, ChannelPublicModuleReadPort, ChannelPublicModuleSelector,
    ReadChannelPublicModule,
};
use uuid::Uuid;

/// Blog's client for the Channel owner's public module binding decision.
#[derive(Clone, Default)]
pub struct BlogChannelAvailability(Option<Arc<dyn ChannelPublicModuleReadPort>>);

impl BlogChannelAvailability {
    pub(crate) fn new(port: Option<Arc<dyn ChannelPublicModuleReadPort>>) -> Self {
        Self(port)
    }

    pub fn from_port(port: Arc<dyn ChannelPublicModuleReadPort>) -> Self {
        Self::new(Some(port))
    }

    pub fn from_runtime(runtime: &HostRuntimeContext) -> Self {
        Self::new(runtime.shared_get::<Arc<dyn ChannelPublicModuleReadPort>>())
    }

    /// Check the host-resolved channel ID, if this request carries one.
    pub async fn for_request(
        &self,
        tenant_id: Uuid,
        request: Option<&RequestContext>,
    ) -> Result<Option<bool>, PortError> {
        let Some(request) = request else {
            return Ok(None);
        };
        if request.tenant_id != tenant_id {
            return Err(PortError::forbidden(
                "blog.channel_tenant_mismatch",
                "Blog channel scope does not match the request",
            ));
        }
        let Some(id) = request.channel_id else {
            return Ok(None);
        };
        self.read(
            tenant_id,
            ChannelPublicModuleSelector::Id(id),
            Some(request),
        )
        .await
        .map(Some)
    }

    pub(crate) async fn by_slug(&self, tenant_id: Uuid, slug: String) -> Result<bool, PortError> {
        self.read(tenant_id, ChannelPublicModuleSelector::Slug(slug), None)
            .await
    }

    async fn read(
        &self,
        tenant_id: Uuid,
        channel: ChannelPublicModuleSelector,
        request: Option<&RequestContext>,
    ) -> Result<bool, PortError> {
        let port = self.0.as_ref().ok_or_else(|| {
            PortError::unavailable(
                "blog.channel_provider_unavailable",
                "Blog channel availability is temporarily unavailable",
            )
        })?;
        let correlation_id = request
            .map(|request| request.correlation_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("blog"),
            PLATFORM_FALLBACK_LOCALE,
            correlation_id,
        )
        .with_claim(CHANNEL_PUBLIC_MODULE_READ_CLAIM)
        .with_deadline(Duration::from_secs(2));
        port.is_public_module_available(
            context,
            ReadChannelPublicModule {
                channel,
                module_slug: "blog".into(),
            },
        )
        .await
    }
}
