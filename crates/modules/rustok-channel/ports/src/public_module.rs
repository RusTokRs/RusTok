use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

pub const CHANNEL_PUBLIC_MODULE_READ_CLAIM: &str = "channel:public_module:read";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelPublicModuleSelector {
    Id(Uuid),
    Slug(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadChannelPublicModule {
    pub channel: ChannelPublicModuleSelector,
    pub module_slug: String,
}

/// Channel-owned availability for a tenant-scoped public request. The
/// caller supplies an authenticated service context with the read claim and
/// deadline. Inactive channels and explicit denials produce `false`, as does
/// an unknown slug. An ID absent from the caller's tenant produces a typed
/// not-found error. Storage and transport failures are typed errors.
#[async_trait]
pub trait ChannelPublicModuleReadPort: Send + Sync {
    async fn is_public_module_available(
        &self,
        context: PortContext,
        request: ReadChannelPublicModule,
    ) -> Result<bool, PortError>;
}
