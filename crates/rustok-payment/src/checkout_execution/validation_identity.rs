struct JournaledProviderResult {
    operation_id: Uuid,
    result: PaymentProviderOperationResult,
}

fn validate_identity(identity: &CheckoutPaymentIdentity) -> Result<(), PortError> {
    if identity.checkout_operation_id.is_nil()
        || identity.cart_id.is_nil()
        || identity.order_id.is_nil()
        || identity.amount <= Decimal::ZERO
    {
        return Err(PortError::validation(
            "payment.checkout_identity_invalid",
            "checkout payment identity contains invalid UUID or amount fields",
        ));
    }
    let currency = identity.currency_code.trim();
    if currency.len() != 3
        || !currency
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return Err(PortError::validation(
            "payment.checkout_currency_invalid",
            "checkout payment currency must be a three-letter alphabetic code",
        ));
    }
    let plan_hash = identity.order_plan_hash.trim();
    if plan_hash.len() != 64 || !plan_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PortError::validation(
            "payment.checkout_plan_hash_invalid",
            "checkout payment order plan hash must be a 64-character hexadecimal value",
        ));
    }
    Ok(())
}

fn validate_optional_collection_identity(
    collection: &PaymentCollectionResponse,
    identity: &CheckoutPaymentIdentity,
) -> Result<(), PortError> {
    let checkout = collection.metadata.get("checkout");
    if let Some(operation_id) = checkout
        .and_then(|value| value.get("operation_id"))
        .and_then(Value::as_str)
    {
        if operation_id != identity.checkout_operation_id.to_string() {
            return Err(PortError::conflict(
                "payment.checkout_collection_operation_conflict",
                "payment collection belongs to another checkout operation",
            ));
        }
    }
    if let Some(plan_hash) = checkout
        .and_then(|value| value.get("order_plan_hash"))
        .and_then(Value::as_str)
    {
        if plan_hash != identity.order_plan_hash {
            return Err(PortError::conflict(
                "payment.checkout_collection_plan_conflict",
                "payment collection belongs to another checkout order plan",
            ));
        }
    }
    Ok(())
}

fn validate_collection(
    collection: &PaymentCollectionResponse,
    tenant_id: Uuid,
    identity: &CheckoutPaymentIdentity,
) -> Result<(), PortError> {
    if collection.tenant_id != tenant_id
        || collection.cart_id != Some(identity.cart_id)
        || collection.order_id != Some(identity.order_id)
        || collection.customer_id != identity.customer_id
        || !collection
            .currency_code
            .eq_ignore_ascii_case(identity.currency_code.as_str())
        || collection.amount != identity.amount
    {
        return Err(PortError::conflict(
            "payment.checkout_collection_identity_conflict",
            "payment collection does not match the checkout identity",
        ));
    }
    validate_optional_collection_identity(collection, identity)?;
    let checkout = collection
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PortError::conflict(
                "payment.checkout_collection_identity_missing",
                "payment collection has no checkout identity",
            )
        })?;
    if checkout.get("operation_id").and_then(Value::as_str)
        != Some(identity.checkout_operation_id.to_string().as_str())
        || checkout.get("order_plan_hash").and_then(Value::as_str)
            != Some(identity.order_plan_hash.as_str())
    {
        return Err(PortError::conflict(
            "payment.checkout_collection_identity_conflict",
            "payment collection has mismatched checkout identity",
        ));
    }
    Ok(())
}

fn checkout_stage_metadata(base: Value, identity: &CheckoutPaymentIdentity, stage: &str) -> Value {
    let mut root = match base {
        Value::Object(root) => root,
        _ => Default::default(),
    };
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert(
        "operation_id".to_string(),
        Value::String(identity.checkout_operation_id.to_string()),
    );
    checkout.insert(
        "order_id".to_string(),
        Value::String(identity.order_id.to_string()),
    );
    checkout.insert(
        "order_plan_hash".to_string(),
        Value::String(identity.order_plan_hash.clone()),
    );
    checkout.insert(
        "payment_stage".to_string(),
        Value::String(stage.to_string()),
    );
    root.insert("checkout".to_string(), Value::Object(checkout));
    root.insert(
        "commerce_orchestration".to_string(),
        serde_json::json!({"operation": format!("checkout_payment_{stage}")}),
    );
    Value::Object(root)
}

fn provider_id_for_collection(collection: &PaymentCollectionResponse) -> String {
    collection
        .provider_id
        .clone()
        .unwrap_or_else(|| MANUAL_PAYMENT_PROVIDER_ID.to_string())
}
