from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one match, found {count}: {old[:100]!r}")
    p.write_text(text.replace(old, new, 1))


def replace_count(path: str, old: str, new: str, expected: int) -> None:
    p = Path(path)
    text = p.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} matches, found {count}: {old[:100]!r}")
    p.write_text(text.replace(old, new))


# Preserve owner-port errors in the existing post-order envelope.
replace_once(
    "crates/rustok-commerce/src/services/post_order.rs",
    "use rust_decimal::Decimal;\n",
    "use rust_decimal::Decimal;\nuse rustok_api::PortError;\n",
)
replace_once(
    "crates/rustok-commerce/src/services/post_order.rs",
    "    #[error(\"order error: {0}\")]\n    Order(#[from] rustok_order::error::OrderError),\n",
    "    #[error(\"order error: {0}\")]\n    Order(#[from] rustok_order::error::OrderError),\n    #[error(\"order owner port error: {0}\")]\n    OrderPort(PortError),\n",
)

core = "crates/rustok-commerce/src/services/return_completion_orchestration.rs"
replace_once(
    core,
    "use rust_decimal::Decimal;\nuse rustok_core::generate_id;\nuse rustok_order::OrderService;\nuse rustok_order::dto::{\n    CompleteOrderReturnInput, CreateOrderChangeInput, ListOrderChangesInput, OrderChangeResponse,\n    OrderReturnResponse,\n};\nuse rustok_order::error::OrderError;\nuse rustok_outbox::TransactionalEventBus;\n",
    "use std::{sync::Arc, time::Duration};\n\nuse rust_decimal::Decimal;\nuse rustok_api::{PortActor, PortContext, PortError, PortErrorKind};\nuse rustok_core::generate_id;\nuse rustok_order::{\n    CompleteOrderReturnInput, CompleteOrderReturnRequest, CreateOrderChangeInput,\n    CreateOrderChangeRequest, ListOrderChangeProjectionsRequest, OrderChangeResponse,\n    OrderPostOrderCommandPort, OrderReadPort, OrderReturnResponse, ReadOrderChangeProjectionRequest,\n    ReadOrderReturnProjectionRequest, in_process_order_post_order_command_port,\n    in_process_order_read_port,\n};\nuse rustok_order::error::OrderError;\nuse rustok_outbox::TransactionalEventBus;\n",
)
replace_once(
    core,
    "pub struct ReturnCompletionOrchestrationService {\n    db: DatabaseConnection,\n    event_bus: TransactionalEventBus,\n    payment_provider_registry: PaymentProviderRegistry,\n}\n",
    "pub struct ReturnCompletionOrchestrationService {\n    db: DatabaseConnection,\n    order_reads: Arc<dyn OrderReadPort>,\n    order_commands: Arc<dyn OrderPostOrderCommandPort>,\n    payment_provider_registry: PaymentProviderRegistry,\n}\n",
)
replace_once(
    core,
    "    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        Self {\n            db,\n            event_bus,\n            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),\n        }\n    }\n",
    "    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        let order_reads = in_process_order_read_port(db.clone(), event_bus.clone());\n        let order_commands = in_process_order_post_order_command_port(db.clone(), event_bus);\n        Self::from_order_ports(db, order_reads, order_commands)\n    }\n\n    pub fn from_order_ports(\n        db: DatabaseConnection,\n        order_reads: Arc<dyn OrderReadPort>,\n        order_commands: Arc<dyn OrderPostOrderCommandPort>,\n    ) -> Self {\n        Self {\n            db,\n            order_reads,\n            order_commands,\n            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),\n        }\n    }\n",
)
replace_once(
    core,
    "        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());\n\n",
    "",
)
replace_once(
    core,
    "            \"completed\" => {\n                return order_service\n                    .get_return(tenant_id, return_id)\n                    .await\n                    .map_err(Into::into);\n            }\n",
    "            \"completed\" => {\n                return self\n                    .read_return(\n                        tenant_id,\n                        actor_id,\n                        return_id,\n                        operation.id,\n                        \"adopt_completed_return\",\n                    )\n                    .await;\n            }\n",
)
replace_once(
    core,
    "            if current.status == \"completed\" {\n                return order_service\n                    .get_return(tenant_id, return_id)\n                    .await\n                    .map_err(Into::into);\n            }\n",
    "            if current.status == \"completed\" {\n                return self\n                    .read_return(\n                        tenant_id,\n                        actor_id,\n                        return_id,\n                        operation.id,\n                        \"adopt_leased_completed_return\",\n                    )\n                    .await;\n            }\n",
)
replace_once(
    core,
    "            .execute_claimed(\n                &journal,\n                &order_service,\n                tenant_id,\n",
    "            .execute_claimed(\n                &journal,\n                tenant_id,\n",
)
replace_once(
    core,
    "        journal: &ReturnCompletionOperationJournal,\n        order_service: &OrderService,\n        tenant_id: Uuid,\n",
    "        journal: &ReturnCompletionOperationJournal,\n        tenant_id: Uuid,\n",
)
replace_once(
    core,
    "        let mut current_return = order_service.get_return(tenant_id, return_id).await?;\n",
    "        let mut current_return = self\n            .read_return(tenant_id, actor_id, return_id, operation.id, \"read_return\")\n            .await?;\n",
)
replace_once(
    core,
    "        self.validate_explicit_resolution_links(\n            order_service,\n            tenant_id,\n            &current_return,\n            refund_id,\n            order_change_id,\n        )\n",
    "        self.validate_explicit_resolution_links(\n            tenant_id,\n            actor_id,\n            operation.id,\n            &current_return,\n            refund_id,\n            order_change_id,\n        )\n",
)
replace_count(
    core,
    "                    journal,\n                    order_service,\n                    tenant_id,\n",
    "                    journal,\n                    tenant_id,\n",
    2,
)
replace_once(
    core,
    "        current_return = order_service.get_return(tenant_id, return_id).await?;\n        let completed = if current_return.status == \"completed\" {\n            current_return\n        } else {\n            match order_service\n                .complete_return(tenant_id, return_id, owner_input)\n                .await\n            {\n                Ok(value) => value,\n                Err(OrderError::InvalidTransition { .. }) => {\n                    let adopted = order_service.get_return(tenant_id, return_id).await?;\n                    if adopted.status == \"completed\" {\n                        adopted\n                    } else {\n                        return Err(OrderError::InvalidTransition {\n                            from: adopted.status,\n                            to: \"completed\".to_string(),\n                        }\n                        .into());\n                    }\n                }\n                Err(error) => return Err(error.into()),\n            }\n        };\n",
    "        current_return = self\n            .read_return(\n                tenant_id,\n                actor_id,\n                return_id,\n                operation.id,\n                \"read_return_before_complete\",\n            )\n            .await?;\n        let completed = if current_return.status == \"completed\" {\n            current_return\n        } else {\n            let context = order_command_context(\n                tenant_id,\n                actor_id,\n                operation.id,\n                \"complete_return\",\n                return_id,\n            );\n            match self\n                .order_commands\n                .complete_return(\n                    context,\n                    CompleteOrderReturnRequest {\n                        return_id,\n                        input: owner_input,\n                    },\n                )\n                .await\n            {\n                Ok(value) => value,\n                Err(error) if matches!(&error.kind, PortErrorKind::Conflict) => {\n                    let adopted = self\n                        .read_return(\n                            tenant_id,\n                            actor_id,\n                            return_id,\n                            operation.id,\n                            \"adopt_complete_return_conflict\",\n                        )\n                        .await?;\n                    if adopted.status == \"completed\" {\n                        adopted\n                    } else {\n                        return Err(PostOrderOrchestrationError::OrderPort(error));\n                    }\n                }\n                Err(error) => return Err(PostOrderOrchestrationError::OrderPort(error)),\n            }\n        };\n",
)
replace_once(
    core,
    "    async fn validate_explicit_resolution_links(\n        &self,\n        order_service: &OrderService,\n        tenant_id: Uuid,\n        order_return: &OrderReturnResponse,\n",
    "    async fn validate_explicit_resolution_links(\n        &self,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        operation_id: Uuid,\n        order_return: &OrderReturnResponse,\n",
)
replace_once(
    core,
    "        if let Some(order_change_id) = order_change_id {\n            let order_change = order_service\n                .get_order_change(tenant_id, order_change_id)\n                .await?;\n",
    "        if let Some(order_change_id) = order_change_id {\n            let order_change = self\n                .read_change(\n                    tenant_id,\n                    actor_id,\n                    order_change_id,\n                    operation_id,\n                    \"validate_explicit_order_change\",\n                )\n                .await?;\n",
)
replace_once(
    core,
    "        journal: &ReturnCompletionOperationJournal,\n        order_service: &OrderService,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        return_id: Uuid,\n",
    "        journal: &ReturnCompletionOperationJournal,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        return_id: Uuid,\n",
)
replace_once(
    core,
    "        let order_change = if let Some(order_change_id) = operation.order_change_id {\n            order_service\n                .get_order_change(tenant_id, order_change_id)\n                .await?\n",
    "        let order_change = if let Some(order_change_id) = operation.order_change_id {\n            self.read_change(\n                tenant_id,\n                actor_id,\n                order_change_id,\n                operation.id,\n                \"adopt_order_change\",\n            )\n            .await?\n",
)
replace_once(
    core,
    "                .find_resolution_order_change(\n                    order_service,\n                    tenant_id,\n                    order_return.order_id,\n                    operation.id,\n                    change_type,\n                )\n",
    "                .find_resolution_order_change(\n                    tenant_id,\n                    actor_id,\n                    order_return.order_id,\n                    operation.id,\n                    change_type,\n                )\n",
)
replace_once(
    core,
    "                order_service\n                    .create_order_change(\n                        tenant_id,\n                        actor_id,\n                        order_return.order_id,\n                        build_resolution_order_change(\n                            change_type,\n                            description,\n                            preview,\n                            metadata,\n                            return_id,\n                            operation.id,\n                        )?,\n                    )\n                    .await?\n",
    "                let context = order_command_context(\n                    tenant_id,\n                    actor_id,\n                    operation.id,\n                    \"create_change\",\n                    order_return.order_id,\n                );\n                self.order_commands\n                    .create_change(\n                        context,\n                        CreateOrderChangeRequest {\n                            order_id: order_return.order_id,\n                            input: build_resolution_order_change(\n                                change_type,\n                                description,\n                                preview,\n                                metadata,\n                                return_id,\n                                operation.id,\n                            )?,\n                        },\n                    )\n                    .await\n                    .map_err(PostOrderOrchestrationError::OrderPort)?\n",
)
replace_once(
    core,
    "    async fn find_resolution_order_change(\n        &self,\n        order_service: &OrderService,\n        tenant_id: Uuid,\n        order_id: Uuid,\n        operation_id: Uuid,\n        change_type: &str,\n    ) -> PostOrderOrchestrationResult<Option<OrderChangeResponse>> {\n        let (changes, _) = order_service\n            .list_order_changes(\n                tenant_id,\n                ListOrderChangesInput {\n                    page: 1,\n                    per_page: 100,\n                    order_id: Some(order_id),\n                    status: None,\n                    change_type: Some(change_type.to_string()),\n                },\n            )\n            .await?;\n        let operation_id = operation_id.to_string();\n        Ok(changes.into_iter().find(|change| {\n",
    "    async fn find_resolution_order_change(\n        &self,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        order_id: Uuid,\n        operation_id: Uuid,\n        change_type: &str,\n    ) -> PostOrderOrchestrationResult<Option<OrderChangeResponse>> {\n        let page = self\n            .order_reads\n            .list_order_change_projections(\n                order_read_context(\n                    tenant_id,\n                    actor_id,\n                    operation_id,\n                    \"find_resolution_order_change\",\n                    order_id,\n                ),\n                ListOrderChangeProjectionsRequest {\n                    page: 1,\n                    per_page: 100,\n                    order_id: Some(order_id),\n                    status: None,\n                    change_type: Some(change_type.to_string()),\n                },\n            )\n            .await\n            .map_err(PostOrderOrchestrationError::OrderPort)?;\n        let operation_id = operation_id.to_string();\n        Ok(page.items.into_iter().find(|change| {\n",
)
replace_once(
    core,
    "    async fn record_failure(\n",
    "    async fn read_return(\n        &self,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        return_id: Uuid,\n        operation_id: Uuid,\n        operation: &'static str,\n    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {\n        self.order_reads\n            .read_order_return_projection(\n                order_read_context(tenant_id, actor_id, operation_id, operation, return_id),\n                ReadOrderReturnProjectionRequest { return_id },\n            )\n            .await\n            .map_err(PostOrderOrchestrationError::OrderPort)\n    }\n\n    async fn read_change(\n        &self,\n        tenant_id: Uuid,\n        actor_id: Uuid,\n        change_id: Uuid,\n        operation_id: Uuid,\n        operation: &'static str,\n    ) -> PostOrderOrchestrationResult<OrderChangeResponse> {\n        self.order_reads\n            .read_order_change_projection(\n                order_read_context(tenant_id, actor_id, operation_id, operation, change_id),\n                ReadOrderChangeProjectionRequest { change_id },\n            )\n            .await\n            .map_err(PostOrderOrchestrationError::OrderPort)\n    }\n\n    async fn record_failure(\n",
)
replace_once(
    core,
    "#[derive(Clone, Copy)]\nenum FailureDisposition {\n",
    "fn order_read_context(\n    tenant_id: Uuid,\n    actor_id: Uuid,\n    operation_id: Uuid,\n    operation: &'static str,\n    resource_id: Uuid,\n) -> PortContext {\n    PortContext::new(\n        tenant_id.to_string(),\n        PortActor::user(actor_id.to_string()),\n        \"und\",\n        format!(\"commerce-return-completion:{operation_id}:{operation}:{resource_id}\"),\n    )\n    .with_deadline(Duration::from_secs(2))\n}\n\nfn order_command_context(\n    tenant_id: Uuid,\n    actor_id: Uuid,\n    operation_id: Uuid,\n    operation: &'static str,\n    resource_id: Uuid,\n) -> PortContext {\n    order_read_context(tenant_id, actor_id, operation_id, operation, resource_id)\n        .with_idempotency_key(format!(\n            \"return-completion:{operation_id}:{operation}:{resource_id}\"\n        ))\n}\n\n#[derive(Clone, Copy)]\nenum FailureDisposition {\n",
)
replace_once(
    core,
    "    match error {\n        PostOrderOrchestrationError::Payment(error) => payment_failure_disposition(error),\n",
    "    match error {\n        PostOrderOrchestrationError::OrderPort(error) => order_port_failure_disposition(error),\n        PostOrderOrchestrationError::Payment(error) => payment_failure_disposition(error),\n",
)
replace_once(
    core,
    "fn payment_failure_disposition(error: &PaymentError) -> FailureDisposition {\n",
    "fn order_port_failure_disposition(error: &PortError) -> FailureDisposition {\n    if error.retryable\n        || matches!(\n            &error.kind,\n            PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation\n        )\n    {\n        FailureDisposition::Retryable\n    } else {\n        FailureDisposition::Failed\n    }\n}\n\nfn payment_failure_disposition(error: &PaymentError) -> FailureDisposition {\n",
)

recovery = "crates/rustok-commerce/src/services/return_completion_recovery.rs"
replace_once(
    recovery,
    "use std::collections::HashMap;\n\nuse chrono::Utc;\nuse rustok_core::generate_id;\nuse rustok_order::OrderService;\nuse rustok_order::dto::OrderReturnResponse;\nuse rustok_order::error::OrderError;\nuse rustok_outbox::TransactionalEventBus;\n",
    "use std::{collections::HashMap, sync::Arc, time::Duration};\n\nuse chrono::Utc;\nuse rustok_api::{PortActor, PortContext};\nuse rustok_core::generate_id;\nuse rustok_order::{\n    OrderPostOrderCommandPort, OrderReadPort, ReadOrderReturnProjectionRequest,\n    dto::OrderReturnResponse, in_process_order_post_order_command_port, in_process_order_read_port,\n};\nuse rustok_order::error::OrderError;\nuse rustok_outbox::TransactionalEventBus;\n",
)
replace_once(
    recovery,
    "pub struct ReturnCompletionOrchestrationService {\n    db: DatabaseConnection,\n    event_bus: TransactionalEventBus,\n    payment_provider_registry: PaymentProviderRegistry,\n}\n",
    "pub struct ReturnCompletionOrchestrationService {\n    db: DatabaseConnection,\n    order_reads: Arc<dyn OrderReadPort>,\n    order_commands: Arc<dyn OrderPostOrderCommandPort>,\n    payment_provider_registry: PaymentProviderRegistry,\n}\n",
)
replace_once(
    recovery,
    "    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        Self {\n            db,\n            event_bus,\n            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),\n        }\n    }\n",
    "    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {\n        let order_reads = in_process_order_read_port(db.clone(), event_bus.clone());\n        let order_commands = in_process_order_post_order_command_port(db.clone(), event_bus);\n        Self::from_order_ports(db, order_reads, order_commands)\n    }\n\n    pub fn from_order_ports(\n        db: DatabaseConnection,\n        order_reads: Arc<dyn OrderReadPort>,\n        order_commands: Arc<dyn OrderPostOrderCommandPort>,\n    ) -> Self {\n        Self {\n            db,\n            order_reads,\n            order_commands,\n            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),\n        }\n    }\n",
)
replace_once(
    recovery,
    "        validate_completion_shape(&input)?;\n        OrderService::new(self.db.clone(), self.event_bus.clone())\n            .get_return(tenant_id, return_id)\n            .await?;\n",
    "        validate_completion_shape(&input)?;\n        self.order_reads\n            .read_order_return_projection(\n                admission_order_read_context(tenant_id, actor_id, return_id),\n                ReadOrderReturnProjectionRequest { return_id },\n            )\n            .await\n            .map_err(PostOrderOrchestrationError::OrderPort)?;\n",
)
replace_once(
    recovery,
    "    fn core_service(&self) -> core::ReturnCompletionOrchestrationService {\n        core::ReturnCompletionOrchestrationService::new(self.db.clone(), self.event_bus.clone())\n            .with_payment_provider_registry(self.payment_provider_registry.clone())\n    }\n",
    "    fn core_service(&self) -> core::ReturnCompletionOrchestrationService {\n        core::ReturnCompletionOrchestrationService::from_order_ports(\n            self.db.clone(),\n            self.order_reads.clone(),\n            self.order_commands.clone(),\n        )\n        .with_payment_provider_registry(self.payment_provider_registry.clone())\n    }\n",
)
replace_once(
    recovery,
    "fn storage_error(error: sea_orm::DbErr) -> PostOrderOrchestrationError {\n",
    "fn admission_order_read_context(tenant_id: Uuid, actor_id: Uuid, return_id: Uuid) -> PortContext {\n    PortContext::new(\n        tenant_id.to_string(),\n        PortActor::user(actor_id.to_string()),\n        \"und\",\n        format!(\"commerce-return-completion-admission:{return_id}\"),\n    )\n    .with_deadline(Duration::from_secs(2))\n}\n\nfn storage_error(error: sea_orm::DbErr) -> PostOrderOrchestrationError {\n",
)

admin_mod = "crates/rustok-commerce/src/controllers/admin/mod.rs"
replace_once(
    admin_mod,
    "use rust_decimal::Decimal;\n",
    "use rust_decimal::Decimal;\nuse rustok_api::{PortError, PortErrorKind};\n",
)
replace_once(
    admin_mod,
    "pub(crate) fn map_post_order_orchestration_error(error: PostOrderOrchestrationError) -> HttpError {\n",
    "pub(crate) fn map_order_port_error(error: PortError) -> HttpError {\n    let (status, code, message, error_kind) = match &error.kind {\n        PortErrorKind::Validation => (axum::http::StatusCode::BAD_REQUEST, \"commerce_admin_order_invalid\", \"Order request is invalid\", \"validation\"),\n        PortErrorKind::NotFound => (axum::http::StatusCode::NOT_FOUND, \"commerce_admin_not_found\", \"Commerce resource not found\", \"not_found\"),\n        PortErrorKind::Conflict => (axum::http::StatusCode::CONFLICT, \"commerce_admin_order_state_conflict\", \"Order operation conflicts with the current state\", \"state_conflict\"),\n        PortErrorKind::Forbidden => (axum::http::StatusCode::UNAUTHORIZED, \"commerce_permission_denied\", \"Permission denied\", \"forbidden\"),\n        PortErrorKind::Unavailable | PortErrorKind::Timeout => (axum::http::StatusCode::SERVICE_UNAVAILABLE, \"commerce_admin_order_storage_unavailable\", \"Order storage is temporarily unavailable\", \"temporarily_unavailable\"),\n        PortErrorKind::InvariantViolation => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, \"commerce_admin_order_failed\", \"Order operation could not be completed safely\", \"invariant_violation\"),\n    };\n    admin_public_error(&error, \"rustok_order\", error_kind, status, code, message)\n}\n\npub(crate) fn map_post_order_orchestration_error(error: PostOrderOrchestrationError) -> HttpError {\n",
)
replace_once(
    admin_mod,
    "        PostOrderOrchestrationError::Order(error) => map_order_error(error),\n",
    "        PostOrderOrchestrationError::Order(error) => map_order_error(error),\n        PostOrderOrchestrationError::OrderPort(error) => map_order_port_error(error),\n",
)

returns = "crates/rustok-commerce/src/controllers/admin/returns.rs"
replace_once(
    returns,
    "        PostOrderOrchestrationError::Payment(source) => {\n",
    "        PostOrderOrchestrationError::OrderPort(source) => {\n            let (status, code, message, error_kind) = admin_order_port_error_policy(source);\n            (status, code, message, error_kind, \"rustok_order\")\n        }\n        PostOrderOrchestrationError::Payment(source) => {\n",
)
replace_once(
    returns,
    "    let item = ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())\n        .with_payment_provider_registry(runtime.payment_provider_registry())\n",
    "    let item = ReturnCompletionOrchestrationService::from_order_ports(\n        runtime.db_clone(),\n        runtime.order_read_port(),\n        runtime.order_post_order_command_port(),\n    )\n    .with_payment_provider_registry(runtime.payment_provider_registry())\n",
)

operator = "crates/rustok-commerce/src/controllers/return_completion_operations.rs"
replace_once(
    operator,
    "use rustok_api::{AuthContext, Permission, TenantContext};\n",
    "use rustok_api::{AuthContext, Permission, PortErrorKind, TenantContext};\n",
)
replace_count(
    operator,
    "ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())",
    "ReturnCompletionOrchestrationService::from_order_ports(\n            runtime.db_clone(),\n            runtime.order_read_port(),\n            runtime.order_post_order_command_port(),\n        )",
    3,
)
replace_once(
    operator,
    "        PostOrderOrchestrationError::Order(\n            rustok_order::error::OrderError::Database(_) | rustok_order::error::OrderError::Core(_),\n        ) => Some((\n",
    "        PostOrderOrchestrationError::OrderPort(source)\n            if matches!(\n                &source.kind,\n                PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation\n            ) => Some((\n            StatusCode::SERVICE_UNAVAILABLE,\n            \"return_completion_storage_unavailable\",\n            \"Return completion recovery storage is unavailable\",\n            \"storage_unavailable\",\n        )),\n        PostOrderOrchestrationError::Order(\n            rustok_order::error::OrderError::Database(_) | rustok_order::error::OrderError::Core(_),\n        ) => Some((\n",
)

graphql = "crates/rustok-commerce/src/graphql/mutations/fulfillment.rs"
replace_once(
    graphql,
    "        PostOrderOrchestrationError::Order(source) => order_error_envelope(source),\n",
    "        PostOrderOrchestrationError::Order(source) => order_error_envelope(source),\n        PostOrderOrchestrationError::OrderPort(source) => {\n            let (message, code, retryable, _) = order_port_error_envelope(source);\n            (message, code, retryable)\n        }\n",
)

runtime = "crates/rustok-commerce/src/graphql_runtime.rs"
replace_once(
    runtime,
    "pub(crate) fn return_completion_orchestration_from_context(\n    ctx: &Context<'_>,\n    db: DatabaseConnection,\n    event_bus: rustok_outbox::TransactionalEventBus,\n) -> crate::ReturnCompletionOrchestrationService {\n    crate::ReturnCompletionOrchestrationService::new(db, event_bus)\n        .with_payment_provider_registry(payment_provider_registry_from_context(ctx))\n}\n",
    "pub(crate) fn return_completion_orchestration_from_context(\n    ctx: &Context<'_>,\n    db: DatabaseConnection,\n    event_bus: rustok_outbox::TransactionalEventBus,\n) -> crate::ReturnCompletionOrchestrationService {\n    let service = match ctx.data_opt::<CommerceGraphqlRuntimeData>() {\n        Some(runtime) => crate::ReturnCompletionOrchestrationService::from_order_ports(\n            db,\n            runtime.order_read_runtime().order_read_port(),\n            runtime.order_post_order_command_runtime().command_port(),\n        ),\n        None => crate::ReturnCompletionOrchestrationService::new(db, event_bus),\n    };\n    service.with_payment_provider_registry(payment_provider_registry_from_context(ctx))\n}\n",
)

hardening = ".github/workflows/ecommerce-hardening.yml"
replace_once(
    hardening,
    "      - name: Verify GraphQL return-decision owner ports\n        run: node scripts/verify/verify-commerce-graphql-return-decision-owner-port-cutover.mjs\n",
    "      - name: Verify GraphQL return-decision owner ports\n        run: node scripts/verify/verify-commerce-graphql-return-decision-owner-port-cutover.mjs\n      - name: Verify return-completion Order owner ports\n        run: node scripts/verify/verify-commerce-return-completion-order-owner-port-cutover.mjs\n",
)

verifier = Path("scripts/verify/verify-commerce-return-completion-order-owner-port-cutover.mjs")
if verifier.exists():
    raise SystemExit(f"{verifier}: already exists")
verifier.write_text('''import fs from "node:fs";\n\nconst read = (path) => fs.readFileSync(path, "utf8");\nconst requireText = (source, text, label) => { if (!source.includes(text)) throw new Error(`missing ${label}: ${text}`); };\nconst forbidText = (source, text, label) => { if (source.includes(text)) throw new Error(`forbidden ${label}: ${text}`); };\n\nconst core = read("crates/rustok-commerce/src/services/return_completion_orchestration.rs");\nconst recovery = read("crates/rustok-commerce/src/services/return_completion_recovery.rs");\nconst postOrder = read("crates/rustok-commerce/src/services/post_order.rs");\nconst returns = read("crates/rustok-commerce/src/controllers/admin/returns.rs");\nconst operator = read("crates/rustok-commerce/src/controllers/return_completion_operations.rs");\nconst graphqlRuntime = read("crates/rustok-commerce/src/graphql_runtime.rs");\nconst graphql = read("crates/rustok-commerce/src/graphql/mutations/fulfillment.rs");\nconst hardening = read(".github/workflows/ecommerce-hardening.yml");\n\nforbidText(core, "OrderService::new", "core direct Order service construction");\nforbidText(core, "order_service: &OrderService", "core concrete Order service parameter");\nforbidText(recovery, "OrderService::new", "recovery direct Order service construction");\nrequireText(core, "Arc<dyn OrderReadPort>", "core Order read owner port");\nrequireText(core, "Arc<dyn OrderPostOrderCommandPort>", "core Order command owner port");\nrequireText(core, ".read_order_return_projection(", "return projection owner read");\nrequireText(core, ".read_order_change_projection(", "order-change projection owner read");\nrequireText(core, ".list_order_change_projections(", "order-change list owner read");\nrequireText(core, ".complete_return(", "return completion owner command");\nrequireText(core, ".create_change(", "resolution order-change owner command");\nrequireText(core, "return-completion:{operation_id}:{operation}:{resource_id}", "durable owner idempotency key");\nrequireText(recovery, "ReturnCompletionOrchestrationService::from_order_ports", "recovery core owner composition");\nrequireText(recovery, "ReadOrderReturnProjectionRequest", "recovery admission owner read");\nrequireText(postOrder, "OrderPort(PortError)", "typed post-order Order port error");\nrequireText(returns, "ReturnCompletionOrchestrationService::from_order_ports", "REST mounted owner composition");\nrequireText(returns, "runtime.order_read_port()", "REST host-selected Order read port");\nrequireText(returns, "runtime.order_post_order_command_port()", "REST host-selected Order command port");\nforbidText(operator, "ReturnCompletionOrchestrationService::new(runtime.db_clone(), runtime.event_bus())", "operator in-process composition");\nrequireText(operator, "ReturnCompletionOrchestrationService::from_order_ports", "operator host-selected owner composition");\nrequireText(graphqlRuntime, "ReturnCompletionOrchestrationService::from_order_ports", "GraphQL host-selected owner composition");\nrequireText(graphqlRuntime, "runtime.order_read_runtime().order_read_port()", "GraphQL Order read runtime");\nrequireText(graphqlRuntime, "runtime.order_post_order_command_runtime().command_port()", "GraphQL Order command runtime");\nrequireText(graphql, "PostOrderOrchestrationError::OrderPort(source)", "GraphQL bounded Order port error mapping");\nrequireText(hardening, "verify-commerce-return-completion-order-owner-port-cutover.mjs", "hardening workflow registration");\n\nconsole.log("commerce return-completion Order owner-port cutover verified");\n''')

record = Path("crates/rustok-commerce/docs/return-completion-order-owner-port-cutover-2026-08-12.md")
if record.exists():
    raise SystemExit(f"{record}: already exists")
record.write_text('''# Return Completion Order Owner-Port Cutover — 2026-08-12\n\nStatus: source_complete_validation_complete\n\n## Scope\n\nThis bounded ecommerce-hardening slice removes direct `OrderService` construction from the durable return-completion core and recovery facade. Mounted REST, operator REST, and GraphQL composition now supply host-selected `OrderReadPort` and `OrderPostOrderCommandPort` implementations.\n\n## Preserved behavior\n\n- Durable return-completion admission, lease, checkpoint, retry, and reconciliation semantics are unchanged.\n- Completed-return adoption still re-reads the Order owner projection after a completion conflict.\n- Exchange/claim resolution still adopts an existing operation-bound order change before creating a new one.\n- Owner write idempotency keys are derived from the durable return-completion operation id and operation/resource identity.\n- Existing Payment service/orchestration behavior is intentionally unchanged in this slice.\n\n## Owner boundaries\n\n- Return and order-change reads use `OrderReadPort`.\n- Resolution order-change creation and return completion use `OrderPostOrderCommandPort`.\n- `PostOrderOrchestrationError::OrderPort` preserves bounded owner error kind/code/retryability through the existing recovery envelope.\n\n## Validation\n\nThe source commit is admitted only after the temporary apply workflow runs `cargo fmt --all`, `node scripts/verify/verify-commerce-return-completion-order-owner-port-cutover.mjs`, and `cargo check -p rustok-commerce --all-features` successfully.\n\n## Remaining work\n\nPayment reads and refund execution inside return-completion still construct Payment services/orchestration directly. That remains a separate bounded owner-port cutover so Order and Payment migration risk are not mixed in one PR.\n''')
