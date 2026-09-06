use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_fulfillment::error::{FulfillmentError, FulfillmentResult};
use rustok_fulfillment::providers::{
    ExternalFulfillmentProviderRegistration, FulfillmentProvider, FulfillmentProviderCapabilities,
    FulfillmentProviderDegradedMode, FulfillmentProviderDescriptor, FulfillmentProviderHealth,
    FulfillmentProviderOperationRequest, FulfillmentProviderOperationResult,
    FulfillmentProviderRegistry, FulfillmentProviderWebhookRequest,
    FulfillmentProviderWebhookResult, FulfillmentRateQuote, FulfillmentRateQuoteRequest,
    ManualFulfillmentProvider,
};
use serde_json::json;
use uuid::Uuid;

struct MockFulfillmentProvider {
    descriptor: FulfillmentProviderDescriptor,
    should_fail: bool,
    error_message: String,
}

#[async_trait]
impl FulfillmentProvider for MockFulfillmentProvider {
    fn descriptor(&self) -> FulfillmentProviderDescriptor {
        self.descriptor.clone()
    }

    async fn quote_rates(
        &self,
        request: FulfillmentRateQuoteRequest,
    ) -> FulfillmentResult<Vec<FulfillmentRateQuote>> {
        if self.should_fail {
            return Err(FulfillmentError::Validation(self.error_message.clone()));
        }
        Ok(vec![FulfillmentRateQuote {
            provider_id: self.descriptor.provider_id.clone(),
            service_code: "express".to_string(),
            amount: Decimal::new(15, 0),
            currency_code: request.currency_code,
            metadata: json!({}),
        }])
    }

    async fn create_label(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        if self.should_fail {
            return Err(FulfillmentError::Validation(self.error_message.clone()));
        }
        Ok(FulfillmentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-label-ref".to_string()),
            tracking_number: Some("TRK-MOCK-123".to_string()),
            metadata: request.metadata,
        })
    }

    async fn cancel(
        &self,
        request: FulfillmentProviderOperationRequest,
    ) -> FulfillmentResult<FulfillmentProviderOperationResult> {
        if self.should_fail {
            return Err(FulfillmentError::Validation(self.error_message.clone()));
        }
        Ok(FulfillmentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-cancel-ref".to_string()),
            tracking_number: None,
            metadata: request.metadata,
        })
    }

    async fn handle_tracking_webhook(
        &self,
        request: FulfillmentProviderWebhookRequest,
    ) -> FulfillmentResult<FulfillmentProviderWebhookResult> {
        if self.should_fail {
            return Err(FulfillmentError::Validation(self.error_message.clone()));
        }
        Ok(FulfillmentProviderWebhookResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: None,
            event_type: "fulfillment.updated".to_string(),
            replay_key: request.idempotency_key,
            tracking_number: None,
            metadata: json!({}),
        })
    }
}

#[tokio::test]
async fn test_manual_fulfillment_provider_capabilities() {
    let provider = ManualFulfillmentProvider;
    let descriptor = provider.descriptor();
    assert_eq!(descriptor.provider_id, "manual");
    assert!(!descriptor.capabilities.rate_quote);
    assert!(descriptor.capabilities.create_label);
    assert!(descriptor.capabilities.ship);
    assert!(descriptor.capabilities.cancel);
    assert!(!descriptor.capabilities.tracking_webhook_ingress);
}

#[tokio::test]
async fn test_manual_fulfillment_provider_operations_success() {
    let provider = ManualFulfillmentProvider;
    let request = FulfillmentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        fulfillment_id: Uuid::new_v4(),
        idempotency_key: Some("idemp-fulfillment-123".to_string()),
        metadata: json!({ "carrier": "manual" }),
    };

    let label_res = provider.create_label(request.clone()).await.unwrap();
    assert_eq!(label_res.provider_id, "manual");
    assert_eq!(label_res.metadata, json!({ "carrier": "manual" }));

    let ship_res = provider.ship(request.clone()).await.unwrap();
    assert_eq!(ship_res.provider_id, "manual");

    let cancel_res = provider.cancel(request).await.unwrap();
    assert_eq!(cancel_res.provider_id, "manual");
}

#[tokio::test]
async fn test_manual_fulfillment_provider_quote_rates_fails() {
    let provider = ManualFulfillmentProvider;
    let request = FulfillmentRateQuoteRequest {
        tenant_id: Uuid::new_v4(),
        cart_id: Some(Uuid::new_v4()),
        seller_id: None,
        shipping_profile_slug: Some("default".to_string()),
        currency_code: "USD".to_string(),
        metadata: json!({}),
    };

    let quote_res = provider.quote_rates(request).await;
    assert!(quote_res.is_err());
    match quote_res.err().unwrap() {
        FulfillmentError::Validation(msg) => {
            assert!(msg.contains("manual fulfillment provider does not quote dynamic rates"));
        }
        _ => panic!("Expected validation error"),
    }
}

#[tokio::test]
async fn test_mock_fulfillment_provider_idempotency_and_error_mapping() {
    let descriptor = FulfillmentProviderDescriptor {
        provider_id: "mock-carrier".to_string(),
        display_name: "Mock Carrier".to_string(),
        capabilities: FulfillmentProviderCapabilities {
            rate_quote: true,
            create_label: true,
            ship: true,
            cancel: true,
            tracking_webhook_ingress: true,
        },
        default_for_manual_options: false,
    };

    let success_provider = MockFulfillmentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };

    let quote_request = FulfillmentRateQuoteRequest {
        tenant_id: Uuid::new_v4(),
        cart_id: None,
        seller_id: None,
        shipping_profile_slug: None,
        currency_code: "EUR".to_string(),
        metadata: json!({}),
    };

    let quote_res = success_provider.quote_rates(quote_request).await.unwrap();
    assert_eq!(quote_res.len(), 1);
    assert_eq!(quote_res[0].provider_id, "mock-carrier");
    assert_eq!(quote_res[0].amount, Decimal::new(15, 0));

    let op_request = FulfillmentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        fulfillment_id: Uuid::new_v4(),
        idempotency_key: Some("idemp-carrier-key".to_string()),
        metadata: json!({}),
    };

    let label_res = success_provider
        .create_label(op_request.clone())
        .await
        .unwrap();
    assert_eq!(label_res.provider_id, "mock-carrier");
    assert_eq!(label_res.tracking_number, Some("TRK-MOCK-123".to_string()));

    let failing_provider = MockFulfillmentProvider {
        descriptor,
        should_fail: true,
        error_message: "Carrier API auth failed".to_string(),
    };

    let label_err = failing_provider.create_label(op_request).await;
    assert!(label_err.is_err());
    match label_err.err().unwrap() {
        FulfillmentError::Validation(msg) => {
            assert_eq!(msg, "Carrier API auth failed");
        }
        _ => panic!("Expected validation error from failing provider"),
    }
}

#[test]
fn test_external_fulfillment_provider_registration_contract() {
    let registration = ExternalFulfillmentProviderRegistration {
        descriptor: FulfillmentProviderDescriptor {
            provider_id: "mock-carrier".to_string(),
            display_name: "Mock Carrier".to_string(),
            capabilities: FulfillmentProviderCapabilities {
                rate_quote: true,
                create_label: true,
                ship: true,
                cancel: true,
                tracking_webhook_ingress: true,
            },
            default_for_manual_options: false,
        },
        health: FulfillmentProviderHealth::Degraded,
        degraded_mode: Some(FulfillmentProviderDegradedMode {
            reason: "carrier_rate_limit".to_string(),
            fallback_profile: "manual_shipping".to_string(),
        }),
    };

    assert!(registration.validate("mock-carrier").is_ok());
    assert!(registration.validate("other-carrier").is_err());
}

#[test]
fn test_fulfillment_provider_runtime_mode_maps_degraded_and_capability_guards() {
    let mut registry = FulfillmentProviderRegistry::with_manual_provider();
    let descriptor = FulfillmentProviderDescriptor {
        provider_id: "slow-carrier".to_string(),
        display_name: "Slow Carrier".to_string(),
        capabilities: FulfillmentProviderCapabilities {
            rate_quote: true,
            create_label: true,
            ship: true,
            cancel: false,
            tracking_webhook_ingress: true,
        },
        default_for_manual_options: false,
    };
    let provider = MockFulfillmentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };
    registry
        .register_external(
            "slow-carrier",
            std::sync::Arc::new(provider),
            ExternalFulfillmentProviderRegistration {
                descriptor,
                health: FulfillmentProviderHealth::Degraded,
                degraded_mode: Some(FulfillmentProviderDegradedMode {
                    reason: "carrier_rate_limit".to_string(),
                    fallback_profile: "manual_shipping".to_string(),
                }),
            },
        )
        .unwrap();

    let mode = registry
        .runtime_mode("slow-carrier", "create_label")
        .unwrap();
    assert!(mode.can_execute);
    assert_eq!(
        mode.degraded_mode.unwrap().fallback_profile,
        "manual_shipping"
    );

    assert!(registry.runtime_mode("slow-carrier", "ship").is_ok());
    assert!(registry.runtime_mode("slow-carrier", "cancel").is_err());
    assert!(registry.runtime_mode("slow-carrier", "unknown").is_err());
    assert!(
        registry
            .runtime_mode("missing-carrier", "create_label")
            .is_err()
    );
}
