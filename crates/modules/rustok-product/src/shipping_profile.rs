//! Product-owned consumer boundary for active Shipping Profile admission.
//! No Commerce tables/services are imported. This read runs only on first execution
//! after receipt admission, never in a transport decorator in front of replay.
use crate::{CatalogService, CommerceError, CommerceResult};
use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

#[async_trait]
pub trait ProductShippingProfilePort: Send + Sync {
    /// All normalized explicit slugs must be active in context.tenant_id.
    /// Authoritative unknown/inactive is NotFound; outages/configuration are retryable.
    async fn validate(&self, context: PortContext, slugs: Vec<String>) -> Result<(), PortError>;
}

impl CatalogService {
    pub fn with_shipping_profile_port(mut self, port: Arc<dyn ProductShippingProfilePort>) -> Self {
        self.shipping_profile_port = Some(port);
        self
    }
    pub(crate) async fn validate_shipping_profiles<'a>(
        &self,
        tenant_id: uuid::Uuid,
        actor_id: uuid::Uuid,
        slugs: impl IntoIterator<Item = &'a str>,
    ) -> CommerceResult<()> {
        let slugs: BTreeSet<String> = slugs
            .into_iter()
            .filter_map(crate::services::catalog::helpers::normalize_shipping_profile_slug)
            .collect();
        if slugs.is_empty() {
            return Ok(());
        }
        if slugs.len() > 100 || slugs.iter().any(|slug| slug.len() > 100) {
            return Err(CommerceError::Validation(
                "Invalid shipping profile selection".into(),
            ));
        }
        let unavailable = || {
            CommerceError::Database(sea_orm::DbErr::Custom(
                "Product shipping profile capability is unavailable".into(),
            ))
        };
        // Reuse the admitted Product operation's authenticated context. Trusted
        // internal service calls receive the same scoped fallback as image writes.
        let context =
            crate::media_reference::operation_context(tenant_id, actor_id, uuid::Uuid::new_v4())?;
        context
            .require_policy(PortCallPolicy::read())
            .map_err(|_| unavailable())?;
        let port = self
            .shipping_profile_port
            .as_ref()
            .ok_or_else(unavailable)?;
        let deadline = Duration::from_millis(context.deadline_ms.unwrap_or(1).min(2000));
        match tokio::time::timeout(
            deadline,
            port.validate(context.clone(), slugs.into_iter().collect()),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) if error.kind == PortErrorKind::NotFound && !error.retryable => Err(
                CommerceError::Validation("Shipping profile is unknown or inactive".into()),
            ),
            // Do not turn an authorization/configuration failure or uncertain read
            // into a terminal receipt rejecting a valid caller request.
            _ => Err(unavailable()),
        }
    }
}
