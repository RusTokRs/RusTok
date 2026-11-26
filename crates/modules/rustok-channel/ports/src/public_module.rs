use async_trait::async_trait;
use rustok_api::{PortContext, PortError};

pub const CHANNEL_PUBLIC_MODULE_READ_CLAIM: &str = "channel:public_module:read";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadChannelPublicModule {
    pub channel_slug: String,
    pub module_slug: String,
}

/// Channel-owned availability for a tenant-scoped public request. The
/// caller supplies an authenticated service context with the read claim and
/// deadline. Unknown channels, inactive channels and explicit denials all
/// produce `false`. Storage and transport failures are typed errors.
#[async_trait]
pub trait ChannelPublicModuleReadPort: Send + Sync {
    async fn is_public_module_available(
        &self,
        context: PortContext,
        request: ReadChannelPublicModule,
    ) -> Result<bool, PortError>;
}
