//! Server-side registration of field-definition services.
//! This file wires concrete module implementations into generic registry.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;
use rustok_events::EventEnvelope;

use crate::models::order_field_definitions::{
    CreateFieldDefinitionInput as OrderCreateInput, Model as OrderModel,
    UpdateFieldDefinitionInput as OrderUpdateInput,
};
use crate::models::product_field_definitions::{
    CreateFieldDefinitionInput as ProductCreateInput, Model as ProductModel,
    UpdateFieldDefinitionInput as ProductUpdateInput,
};
use crate::models::topic_field_definitions::{
    CreateFieldDefinitionInput as TopicCreateInput, Model as TopicModel,
    UpdateFieldDefinitionInput as TopicUpdateInput,
};
use crate::models::user_field_definitions::{
    CreateFieldDefinitionInput as UserCreateInput, Model as UserModel,
    UpdateFieldDefinitionInput as UserUpdateInput,
};
use crate::services::order_field_service::OrderFieldService;
use crate::services::product_field_service::ProductFieldService;
use crate::services::topic_field_service::TopicFieldService;
use crate::services::user_field_service::UserFieldService;
use flex::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    FieldDefinitionViewSource, UpdateFieldDefinitionCommand,
};

struct UserFieldDefinitionService;
struct OrderFieldDefinitionService;
struct ProductFieldDefinitionService;
struct TopicFieldDefinitionService;

macro_rules! impl_field_definition_view_source {
    ($model:ty) => {
        impl FieldDefinitionViewSource for $model {
            fn id(&self) -> Uuid {
                self.id
            }

            fn field_key(&self) -> &str {
                &self.field_key
            }

            fn field_type(&self) -> &str {
                &self.field_type
            }

            fn label(&self) -> &serde_json::Value {
                &self.label
            }

            fn description(&self) -> Option<&serde_json::Value> {
                self.description.as_ref()
            }

            fn is_localized(&self) -> bool {
                self.is_localized
            }

            fn is_required(&self) -> bool {
                self.is_required
            }

            fn default_value(&self) -> Option<&serde_json::Value> {
                self.default_value.as_ref()
            }

            fn validation(&self) -> Option<&serde_json::Value> {
                self.validation.as_ref()
            }

            fn position(&self) -> i32 {
                self.position
            }

            fn is_active(&self) -> bool {
                self.is_active
            }

            fn created_at(&self) -> String {
                self.created_at.to_rfc3339()
            }

            fn updated_at(&self) -> String {
                self.updated_at.to_rfc3339()
            }
        }
    };
}

impl_field_definition_view_source!(UserModel);
impl_field_definition_view_source!(OrderModel);
impl_field_definition_view_source!(ProductModel);
impl_field_definition_view_source!(TopicModel);

flex::impl_field_definition_command_conversions!(UserCreateInput, UserUpdateInput);
flex::impl_field_definition_command_conversions!(OrderCreateInput, OrderUpdateInput);
flex::impl_field_definition_command_conversions!(ProductCreateInput, ProductUpdateInput);
flex::impl_field_definition_command_conversions!(TopicCreateInput, TopicUpdateInput);

fn field_definition_model_to_view<T: FieldDefinitionViewSource>(model: T) -> FieldDefinitionView {
    FieldDefinitionView::from_source(&model)
}

macro_rules! impl_field_definition_service_adapter {
    ($adapter:ty, $entity_type:literal, $service:ty) => {
        #[async_trait]
        impl FieldDefinitionService for $adapter {
            fn entity_type(&self) -> &'static str {
                $entity_type
            }

            async fn list_all(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
            ) -> Result<Vec<FieldDefinitionView>, FlexError> {
                let rows = <$service>::list_all(db, tenant_id).await?;
                Ok(rows
                    .into_iter()
                    .map(field_definition_model_to_view)
                    .collect())
            }

            async fn find_by_id(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
                id: Uuid,
            ) -> Result<Option<FieldDefinitionView>, FlexError> {
                let row = <$service>::find_by_id(db, tenant_id, id).await?;
                Ok(row.map(field_definition_model_to_view))
            }

            async fn reorder(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
                ids: &[Uuid],
            ) -> Result<Vec<FieldDefinitionView>, FlexError> {
                let rows = <$service>::reorder(db, tenant_id, ids).await?;
                Ok(rows
                    .into_iter()
                    .map(field_definition_model_to_view)
                    .collect())
            }

            async fn create(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
                actor_id: Option<Uuid>,
                input: CreateFieldDefinitionCommand,
            ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
                let (row, event) =
                    <$service>::create(db, tenant_id, actor_id, input.into()).await?;
                Ok((field_definition_model_to_view(row), event))
            }

            async fn update(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
                actor_id: Option<Uuid>,
                id: Uuid,
                input: UpdateFieldDefinitionCommand,
            ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
                let (row, event) =
                    <$service>::update(db, tenant_id, actor_id, id, input.into()).await?;
                Ok((field_definition_model_to_view(row), event))
            }

            async fn deactivate(
                &self,
                db: &DatabaseConnection,
                tenant_id: Uuid,
                actor_id: Option<Uuid>,
                id: Uuid,
            ) -> Result<EventEnvelope, FlexError> {
                <$service>::deactivate(db, tenant_id, actor_id, id).await
            }
        }
    };
}

impl_field_definition_service_adapter!(UserFieldDefinitionService, "user", UserFieldService);
impl_field_definition_service_adapter!(OrderFieldDefinitionService, "order", OrderFieldService);
impl_field_definition_service_adapter!(
    ProductFieldDefinitionService,
    "product",
    ProductFieldService
);
impl_field_definition_service_adapter!(TopicFieldDefinitionService, "topic", TopicFieldService);

pub fn build_field_def_registry() -> FieldDefRegistry {
    let mut registry = FieldDefRegistry::new();
    registry.register(Arc::new(UserFieldDefinitionService));
    registry.register(Arc::new(OrderFieldDefinitionService));
    registry.register(Arc::new(ProductFieldDefinitionService));
    registry.register(Arc::new(TopicFieldDefinitionService));
    registry
}

#[cfg(test)]
mod tests {
    use rustok_core::field_schema::FlexError;

    use super::build_field_def_registry;

    #[test]
    fn registry_bootstrap_registers_topic_entity_type() {
        let registry = build_field_def_registry();

        let topic_service = registry
            .get("topic")
            .expect("topic entity type should be registered");

        assert_eq!(topic_service.entity_type(), "topic");
    }

    #[test]
    fn registry_bootstrap_keeps_unknown_entity_type_error() {
        let registry = build_field_def_registry();

        let err = match registry.get("unknown") {
            Ok(_) => panic!("unknown entity type should return error"),
            Err(err) => err,
        };

        assert!(matches!(err, FlexError::UnknownEntityType(_)));
    }
}
