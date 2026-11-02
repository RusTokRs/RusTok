//! Local provider-result recovery is a Fulfillment owner command. No carrier invocation,
//! peer implementation/table read, synthetic actor or newly-derived mutation key is allowed.
use super::*;

impl InProcessFulfillmentAdminCommandPort {
    pub(super) async fn retry_local_persistence_impl(
        &self,
        context: PortContext,
        request: RetryLocalFulfillmentPersistenceRequest,
    ) -> Result<FulfillmentResponse, PortError> {
        const OPERATION: &str = "retry_local_fulfillment_persistence";
        let started = std::time::Instant::now();
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, OPERATION)?;
        if request.operation_id.is_nil() {
            return Err(PortError::validation(
                "fulfillment.operation_id_nil",
                "provider operation id must not be nil",
            ));
        }
        let operation = self
            .operation_journal
            .get(tenant_id, request.operation_id)
            .await
            .map_err(|error| map_fulfillment_error(&context, OPERATION, error))?;
        if context
            .idempotency_key
            .as_deref()
            .is_some_and(|key| key != operation.idempotency_key)
        {
            return Err(PortError::invariant_violation(
                "fulfillment.shipment_stock_recovery_identity_unverified",
                "recovery must retain the original provider operation key, including deployed legacy keys",
            ));
        }
        // Identity comes only from the scoped owner journal; all other real operator context
        // (actor, claims, roles, locale, channel, tracing and current deadline) stays intact.
        let preserved_context = context.with_idempotency_key(operation.idempotency_key.clone());
        let context = crate::shipment_stock::remaining_stock_context(&preserved_context, started)?;
        require_write_admission(&context, OPERATION)?;
        if operation.status == PROVIDER_OPERATION_COMMITTED {
            return self
                .service
                .get_fulfillment(tenant_id, operation.fulfillment_id)
                .await
                .map_err(|error| map_fulfillment_error(&context, OPERATION, error));
        }
        if !matches!(
            operation.status.as_str(),
            PROVIDER_OPERATION_SUCCEEDED | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
        ) {
            return Err(PortError::conflict(
                "fulfillment.reconciliation_required",
                "local recovery requires a retained provider result; an executing or legacy error row is not permission to reinvoke a carrier",
            ));
        }
        let saved: FulfillmentProviderOperationRequest =
            serde_json::from_value(operation.request_payload.clone())
                .map_err(|_| recovery_invalid())?;
        if saved.tenant_id != tenant_id
            || saved.fulfillment_id != operation.fulfillment_id
            || saved.idempotency_key.as_deref() != Some(operation.idempotency_key.as_str())
        {
            return Err(recovery_invalid());
        }
        let result = deserialize_provider_result(&operation)?;
        let descriptor = saved
            .metadata
            .get("commerce_orchestration")
            .and_then(Value::as_object);
        let mut caller_metadata = saved.metadata.clone();
        if let Some(metadata) = caller_metadata.as_object_mut() {
            metadata.remove("commerce_orchestration");
        }

        let updated = match operation.operation.as_str() {
            "ship" | "reship" => {
                let descriptor = descriptor.ok_or_else(recovery_invalid)?;
                if descriptor.get("operation").and_then(Value::as_str)
                    != Some(operation.operation.as_str())
                {
                    return Err(recovery_invalid());
                }
                let carrier = required_text(descriptor, "carrier")?;
                let requested_tracking = required_text(descriptor, "tracking_number")?;
                let tracking_number = result.tracking_number.clone().unwrap_or(requested_tracking);
                let items = match descriptor.get("items") {
                    Some(Value::Null) => None,
                    Some(value) => Some(
                        serde_json::from_value::<Vec<crate::dto::FulfillmentItemQuantityInput>>(
                            value.clone(),
                        )
                        .map_err(|_| recovery_invalid())?,
                    ),
                    None => return Err(recovery_invalid()),
                };
                if operation.operation == "ship" {
                    self.service
                        .ship_fulfillment_with_provider_result(
                            crate::shipment_stock::remaining_stock_context(
                                &preserved_context,
                                started,
                            )?,
                            operation.fulfillment_id,
                            ShipFulfillmentInput {
                                carrier,
                                tracking_number,
                                items,
                                metadata: caller_metadata.clone(),
                            },
                            result.metadata.clone(),
                            operation.id,
                        )
                        .await
                } else {
                    self.service
                        .reship_fulfillment_with_prepared_provider_result(
                            crate::shipment_stock::remaining_stock_context(
                                &preserved_context,
                                started,
                            )?,
                            None,
                            operation.fulfillment_id,
                            ReshipFulfillmentInput {
                                carrier,
                                tracking_number,
                                items,
                                metadata: caller_metadata.clone(),
                            },
                            result.metadata.clone(),
                            operation.id,
                        )
                        .await
                }
            }
            "cancel" => {
                let descriptor = descriptor.ok_or_else(recovery_invalid)?;
                if descriptor.get("operation").and_then(Value::as_str) != Some("cancel") {
                    return Err(recovery_invalid());
                }
                let reason = match descriptor.get("reason") {
                    Some(Value::Null) => None,
                    Some(Value::String(value)) => Some(value.clone()),
                    _ => return Err(recovery_invalid()),
                };
                self.service
                    .cancel_fulfillment_with_provider_result(
                        crate::shipment_stock::remaining_stock_context(
                            &preserved_context,
                            started,
                        )?,
                        operation.fulfillment_id,
                        CancelFulfillmentInput {
                            reason,
                            metadata: caller_metadata.clone(),
                        },
                        result.metadata.clone(),
                        operation.id,
                    )
                    .await
            }
            "create_label" => {
                self.service
                    .commit_create_label_provider_result(
                        tenant_id,
                        operation.fulfillment_id,
                        operation.id,
                        &result,
                    )
                    .await
            }
            _ => return Err(recovery_invalid()),
        };
        match updated {
            Ok(response) => Ok(response), // the domain already committed its journal atomically
            Err(error) => {
                self.mark_local_persistence_reconciliation(
                    &context,
                    tenant_id,
                    OPERATION,
                    operation.id,
                    "recovery",
                )
                .await;
                Err(map_fulfillment_error(&context, OPERATION, error).with_retryable(false))
            }
        }
    }
}

fn required_text(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, PortError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(recovery_invalid)
}
fn recovery_invalid() -> PortError {
    PortError::invariant_violation(
        "fulfillment.recovery_scope_unverified",
        "retained provider request/result does not prove the original local lifecycle scope; reconcile without historical backfill or a new key",
    )
}
