use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    CollectionOwnerService, CollectionOwnerSnapshot, CommerceError, CreateCollectionOwnerInput,
    UpdateCollectionOwnerInput,
};

#[async_trait]
pub trait CollectionOwnerPort: Send + Sync {
    async fn create_collection(
        &self,
        context: PortContext,
        input: CreateCollectionOwnerInput,
    ) -> Result<CollectionOwnerSnapshot, PortError>;

    async fn update_collection(
        &self,
        context: PortContext,
        collection_id: Uuid,
        input: UpdateCollectionOwnerInput,
    ) -> Result<CollectionOwnerSnapshot, PortError>;

    async fn delete_collection(
        &self,
        context: PortContext,
        collection_id: Uuid,
    ) -> Result<(), PortError>;
}

pub fn in_process_collection_owner_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CollectionOwnerPort> {
    Arc::new(CollectionOwnerService::new(db, event_bus))
}

#[async_trait]
impl CollectionOwnerPort for CollectionOwnerService {
    async fn create_collection(
        &self,
        context: PortContext,
        input: CreateCollectionOwnerInput,
    ) -> Result<CollectionOwnerSnapshot, PortError> {
        let tenant_id = authorize_write(&context)?;
        CollectionOwnerService::create_collection(self, tenant_id, actor_id(&context), input)
            .await
            .map_err(owner_error_to_port_error)
    }

    async fn update_collection(
        &self,
        context: PortContext,
        collection_id: Uuid,
        input: UpdateCollectionOwnerInput,
    ) -> Result<CollectionOwnerSnapshot, PortError> {
        let tenant_id = authorize_write(&context)?;
        CollectionOwnerService::update_collection(
            self,
            tenant_id,
            actor_id(&context),
            collection_id,
            input,
        )
        .await
        .map_err(owner_error_to_port_error)
    }

    async fn delete_collection(
        &self,
        context: PortContext,
        collection_id: Uuid,
    ) -> Result<(), PortError> {
        let tenant_id = authorize_write(&context)?;
        CollectionOwnerService::delete_collection(
            self,
            tenant_id,
            actor_id(&context),
            collection_id,
        )
        .await
        .map_err(owner_error_to_port_error)
    }
}

fn authorize_write(context: &PortContext) -> Result<Uuid, PortError> {
    context.require_policy(PortCallPolicy::write())?;
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "commerce.collection_owner_tenant_id_invalid",
            "Collection owner context must carry a UUID tenant_id",
        )
    })
}

fn actor_id(context: &PortContext) -> Option<Uuid> {
    Uuid::parse_str(&context.actor.id).ok()
}

fn owner_error_to_port_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Database(_) => PortError::unavailable(
            "commerce.collection_owner_database_unavailable",
            "Collection owner storage is temporarily unavailable",
        ),
        CommerceError::DuplicateHandle { .. } => PortError::conflict(
            "commerce.collection_owner_handle_conflict",
            "Collection handle conflicts with another active Collection",
        ),
        CommerceError::Validation(message) if message == "collection_id was not found" => {
            PortError::not_found(
                "commerce.collection_owner_not_found",
                "Collection was not found",
            )
        }
        CommerceError::Validation(_) => PortError::validation(
            "commerce.collection_owner_validation",
            "Collection owner request is invalid",
        ),
        _ => PortError::invariant_violation(
            "commerce.collection_owner_invariant",
            "Collection owner operation failed",
        ),
    }
}
