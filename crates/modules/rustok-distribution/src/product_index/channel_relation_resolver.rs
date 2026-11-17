use std::sync::Arc;

use async_trait::async_trait;
use rustok_channel::PostgresChannelIndexIdentity;
use rustok_product::ProductSalesChannelIndexRelationObservation;
use rustok_product::services::{
    ProductSalesChannelObservationSource, ProductSalesChannelRelationResolver,
    ProductSalesChannelRelationResolverError, bind_channel_observation, channel_identity_selector,
    load_product_visibility,
};
use sea_orm::{AccessMode, DatabaseConnection, IsolationLevel, TransactionTrait};
use uuid::Uuid;

pub(super) fn embedded_relation_resolver(
    db: DatabaseConnection,
) -> ProductSalesChannelRelationResolver {
    ProductSalesChannelRelationResolver::new(
        db.clone(),
        Arc::new(EmbeddedProductChannelObservation { db }),
    )
}

struct EmbeddedProductChannelObservation {
    db: DatabaseConnection,
}

#[async_trait]
impl ProductSalesChannelObservationSource for EmbeddedProductChannelObservation {
    async fn observe(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
    ) -> Result<ProductSalesChannelIndexRelationObservation, ProductSalesChannelRelationResolverError>
    {
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
            .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;

        let result = async {
            let (visibility, product_source_version) =
                load_product_visibility(&transaction, tenant_id, product_id).await?;
            let snapshot = PostgresChannelIndexIdentity::read_in(
                &transaction,
                tenant_id,
                channel_identity_selector(&visibility),
            )
            .await;
            bind_channel_observation(tenant_id, product_source_version, visibility, snapshot)
        }
        .await;

        match result {
            Ok(observation) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| ProductSalesChannelRelationResolverError::Unavailable)?;
                Ok(observation)
            }
            Err(error) => {
                if let Err(rb_err) = transaction.rollback().await {
                    tracing::warn!(%rb_err, "failed to rollback transaction during channel relation resolution");
                }
                Err(error)
            }
        }
    }
}
