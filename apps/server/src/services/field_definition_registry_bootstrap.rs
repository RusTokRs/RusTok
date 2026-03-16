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
use crate::services::field_definition_registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};
use crate::services::order_field_service::OrderFieldService;
use crate::services::product_field_service::ProductFieldService;
use crate::services::topic_field_service::TopicFieldService;
use crate::services::user_field_service::UserFieldService;

struct UserFieldDefinitionService;
struct OrderFieldDefinitionService;
struct ProductFieldDefinitionService;
struct TopicFieldDefinitionService;

#[async_trait]
impl FieldDefinitionService for UserFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        "user"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = UserFieldService::list_all(db, tenant_id).await?;
        Ok(rows.into_iter().map(user_model_to_view).collect())
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        let row = UserFieldService::find_by_id(db, tenant_id, id).await?;
        Ok(row.map(user_model_to_view))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = UserFieldService::reorder(db, tenant_id, ids).await?;
        Ok(rows.into_iter().map(user_model_to_view).collect())
    }

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let user_input = UserCreateInput {
            field_key: input.field_key,
            field_type: input.field_type,
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
        };

        let (row, event) = UserFieldService::create(db, tenant_id, actor_id, user_input).await?;
        Ok((user_model_to_view(row), event))
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let user_input = UserUpdateInput {
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (row, event) =
            UserFieldService::update(db, tenant_id, actor_id, id, user_input).await?;
        Ok((user_model_to_view(row), event))
    }

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        UserFieldService::deactivate(db, tenant_id, actor_id, id).await
    }
}

#[async_trait]
impl FieldDefinitionService for OrderFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        "order"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = OrderFieldService::list_all(db, tenant_id).await?;
        Ok(rows.into_iter().map(order_model_to_view).collect())
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        let row = OrderFieldService::find_by_id(db, tenant_id, id).await?;
        Ok(row.map(order_model_to_view))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = OrderFieldService::reorder(db, tenant_id, ids).await?;
        Ok(rows.into_iter().map(order_model_to_view).collect())
    }

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let order_input = OrderCreateInput {
            field_key: input.field_key,
            field_type: input.field_type,
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
        };

        let (row, event) = OrderFieldService::create(db, tenant_id, actor_id, order_input).await?;
        Ok((order_model_to_view(row), event))
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let order_input = OrderUpdateInput {
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (row, event) =
            OrderFieldService::update(db, tenant_id, actor_id, id, order_input).await?;
        Ok((order_model_to_view(row), event))
    }

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        OrderFieldService::deactivate(db, tenant_id, actor_id, id).await
    }
}

#[async_trait]
impl FieldDefinitionService for ProductFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        "product"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = ProductFieldService::list_all(db, tenant_id).await?;
        Ok(rows.into_iter().map(product_model_to_view).collect())
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        let row = ProductFieldService::find_by_id(db, tenant_id, id).await?;
        Ok(row.map(product_model_to_view))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = ProductFieldService::reorder(db, tenant_id, ids).await?;
        Ok(rows.into_iter().map(product_model_to_view).collect())
    }

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let product_input = ProductCreateInput {
            field_key: input.field_key,
            field_type: input.field_type,
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
        };

        let (row, event) =
            ProductFieldService::create(db, tenant_id, actor_id, product_input).await?;
        Ok((product_model_to_view(row), event))
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let product_input = ProductUpdateInput {
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (row, event) =
            ProductFieldService::update(db, tenant_id, actor_id, id, product_input).await?;
        Ok((product_model_to_view(row), event))
    }

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        ProductFieldService::deactivate(db, tenant_id, actor_id, id).await
    }
}

#[async_trait]
impl FieldDefinitionService for TopicFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        "topic"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = TopicFieldService::list_all(db, tenant_id).await?;
        Ok(rows.into_iter().map(topic_model_to_view).collect())
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        let row = TopicFieldService::find_by_id(db, tenant_id, id).await?;
        Ok(row.map(topic_model_to_view))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = TopicFieldService::reorder(db, tenant_id, ids).await?;
        Ok(rows.into_iter().map(topic_model_to_view).collect())
    }

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let topic_input = TopicCreateInput {
            field_key: input.field_key,
            field_type: input.field_type,
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
        };

        let (row, event) = TopicFieldService::create(db, tenant_id, actor_id, topic_input).await?;
        Ok((topic_model_to_view(row), event))
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
        let topic_input = TopicUpdateInput {
            label: input.label,
            description: input.description,
            is_required: input.is_required,
            default_value: input.default_value,
            validation: input.validation,
            position: input.position,
            is_active: input.is_active,
        };

        let (row, event) =
            TopicFieldService::update(db, tenant_id, actor_id, id, topic_input).await?;
        Ok((topic_model_to_view(row), event))
    }

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<EventEnvelope, FlexError> {
        TopicFieldService::deactivate(db, tenant_id, actor_id, id).await
    }
}

fn user_model_to_view(m: UserModel) -> FieldDefinitionView {
    FieldDefinitionView {
        id: m.id,
        field_key: m.field_key,
        field_type: m.field_type,
        label: m.label,
        description: m.description,
        is_required: m.is_required,
        default_value: m.default_value,
        validation: m.validation,
        position: m.position,
        is_active: m.is_active,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn order_model_to_view(m: OrderModel) -> FieldDefinitionView {
    FieldDefinitionView {
        id: m.id,
        field_key: m.field_key,
        field_type: m.field_type,
        label: m.label,
        description: m.description,
        is_required: m.is_required,
        default_value: m.default_value,
        validation: m.validation,
        position: m.position,
        is_active: m.is_active,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn product_model_to_view(m: ProductModel) -> FieldDefinitionView {
    FieldDefinitionView {
        id: m.id,
        field_key: m.field_key,
        field_type: m.field_type,
        label: m.label,
        description: m.description,
        is_required: m.is_required,
        default_value: m.default_value,
        validation: m.validation,
        position: m.position,
        is_active: m.is_active,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn topic_model_to_view(m: TopicModel) -> FieldDefinitionView {
    FieldDefinitionView {
        id: m.id,
        field_key: m.field_key,
        field_type: m.field_type,
        label: m.label,
        description: m.description,
        is_required: m.is_required,
        default_value: m.default_value,
        validation: m.validation,
        position: m.position,
        is_active: m.is_active,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

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
    fn registry_bootstrap_registers_order_entity_type() {
        let registry = build_field_def_registry();

        let order_service = registry
            .get("order")
            .expect("order entity type should be registered");

        assert_eq!(order_service.entity_type(), "order");
    }

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

        let err = registry
            .get("unknown")
            .expect_err("unknown entity type should return error");

        assert!(matches!(err, FlexError::UnknownEntityType(_)));
    }
}
