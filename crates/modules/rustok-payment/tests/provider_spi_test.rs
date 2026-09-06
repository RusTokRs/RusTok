use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_payment::error::{PaymentError, PaymentResult};
use rustok_payment::providers::{
    ExternalPaymentProviderRegistration, ManualPaymentProvider, PaymentProvider,
    PaymentProviderCapabilities, PaymentProviderDegradedMode, PaymentProviderDescriptor,
    PaymentProviderHealth, PaymentProviderOperationRequest, PaymentProviderOperationResult,
    PaymentProviderRegistry, PaymentProviderWebhookRequest, PaymentProviderWebhookResult,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

struct MockPaymentProvider {
    descriptor: PaymentProviderDescriptor,
    should_fail: bool,
    error_message: String,
}

impl MockPaymentProvider {
    fn descriptor(provider_id: &str) -> PaymentProviderDescriptor {
        PaymentProviderDescriptor {
            provider_id: provider_id.to_string(),
            display_name: "Mock Gateway".to_string(),
            capabilities: PaymentProviderCapabilities {
                authorize: true,
                capture: true,
                refund: true,
                cancel: true,
                webhook_ingress: true,
            },
            default_for_new_collections: false,
        }
    }
}

#[async_trait]
impl PaymentProvider for MockPaymentProvider {
    fn descriptor(&self) -> PaymentProviderDescriptor {
        self.descriptor.clone()
    }

    async fn authorize(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        if self.should_fail {
            return Err(PaymentError::Validation(self.error_message.clone()));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-auth-ref".to_string()),
            authorized_amount: request.amount,
            captured_amount: Decimal::ZERO,
            metadata: request.metadata,
        })
    }

    async fn capture(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        if self.should_fail {
            return Err(PaymentError::Validation(self.error_message.clone()));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-cap-ref".to_string()),
            authorized_amount: request.amount,
            captured_amount: request.amount,
            metadata: request.metadata,
        })
    }

    async fn cancel(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        if self.should_fail {
            return Err(PaymentError::Validation(self.error_message.clone()));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-cancel-ref".to_string()),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: request.metadata,
        })
    }

    async fn refund(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        if self.should_fail {
            return Err(PaymentError::Validation(self.error_message.clone()));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: self.descriptor.provider_id.clone(),
            external_reference: Some("mock-refund-ref".to_string()),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: request.metadata,
        })
    }

    async fn handle_webhook(
        &self,
        _request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        if self.should_fail {
            return Err(PaymentError::Validation(self.error_message.clone()));
        }
        Ok(PaymentProviderWebhookResult {
            provider_id: self.descriptor.provider_id.clone(),
            delivery_id: "evt_verified_1".to_string(),
            external_reference: None,
            event_type: "payment.updated".to_string(),
            replay_key: "evt_verified_1".to_string(),
            metadata: json!({}),
        })
    }
}

fn registered_mock_registry(provider_id: &str) -> PaymentProviderRegistry {
    let descriptor = MockPaymentProvider::descriptor(provider_id);
    let provider = MockPaymentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };
    let mut registry = PaymentProviderRegistry::new();
    registry
        .register_external(
            provider_id,
            Arc::new(provider),
            ExternalPaymentProviderRegistration {
                descriptor,
                health: PaymentProviderHealth::Ready,
                degraded_mode: None,
            },
        )
        .unwrap();
    registry
}

#[tokio::test]
async fn test_manual_provider_capabilities() {
    let provider = ManualPaymentProvider;
    let descriptor = provider.descriptor();
    assert_eq!(descriptor.provider_id, "manual");
    assert!(descriptor.capabilities.authorize);
    assert!(descriptor.capabilities.capture);
    assert!(descriptor.capabilities.refund);
    assert!(descriptor.capabilities.cancel);
    assert!(!descriptor.capabilities.webhook_ingress);
}

#[tokio::test]
async fn test_manual_provider_operations_success() {
    let provider = ManualPaymentProvider;
    let amount = Decimal::new(100, 0);
    let request = PaymentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        collection_id: Uuid::new_v4(),
        amount,
        currency_code: "USD".to_string(),
        idempotency_key: Some("idemp-key-123".to_string()),
        metadata: json!({ "custom": "data" }),
    };

    let auth_res = provider.authorize(request.clone()).await.unwrap();
    assert_eq!(auth_res.provider_id, "manual");
    assert_eq!(auth_res.authorized_amount, amount);
    assert_eq!(auth_res.captured_amount, Decimal::ZERO);
    assert_eq!(auth_res.metadata, json!({ "custom": "data" }));

    let cap_res = provider.capture(request.clone()).await.unwrap();
    assert_eq!(cap_res.captured_amount, amount);
    let cancel_res = provider.cancel(request.clone()).await.unwrap();
    assert_eq!(cancel_res.authorized_amount, Decimal::ZERO);
    let refund_res = provider.refund(request).await.unwrap();
    assert_eq!(refund_res.captured_amount, Decimal::ZERO);
}

#[tokio::test]
async fn test_manual_provider_refund_invalid_amount() {
    let provider = ManualPaymentProvider;
    let request = PaymentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        collection_id: Uuid::new_v4(),
        amount: Decimal::ZERO,
        currency_code: "USD".to_string(),
        idempotency_key: None,
        metadata: json!({}),
    };
    let error = provider.refund(request).await.unwrap_err();
    assert!(
        matches!(error, PaymentError::Validation(message) if message.contains("refund amount must be greater than zero"))
    );
}

#[tokio::test]
async fn test_mock_provider_idempotency_and_error_mapping() {
    let descriptor = MockPaymentProvider::descriptor("mock-gateway");
    let request = PaymentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        collection_id: Uuid::new_v4(),
        amount: Decimal::new(50, 0),
        currency_code: "EUR".to_string(),
        idempotency_key: Some("idemp-mock-key".to_string()),
        metadata: json!({}),
    };
    let success_provider = MockPaymentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };
    let auth_res = success_provider.authorize(request.clone()).await.unwrap();
    assert_eq!(auth_res.provider_id, "mock-gateway");
    assert_eq!(auth_res.authorized_amount, Decimal::new(50, 0));

    let failing_provider = MockPaymentProvider {
        descriptor,
        should_fail: true,
        error_message: "Gateway connection timeout".to_string(),
    };
    let error = failing_provider.authorize(request).await.unwrap_err();
    assert!(
        matches!(error, PaymentError::Validation(message) if message == "Gateway connection timeout")
    );
}

#[test]
fn test_external_payment_provider_registration_contract() {
    let registration = ExternalPaymentProviderRegistration {
        descriptor: MockPaymentProvider::descriptor("mock-gateway"),
        health: PaymentProviderHealth::Degraded,
        degraded_mode: Some(PaymentProviderDegradedMode {
            reason: "gateway_timeout".to_string(),
            fallback_profile: "manual_review".to_string(),
        }),
    };
    assert!(registration.validate("mock-gateway").is_ok());
    assert!(registration.validate("other-gateway").is_err());
}

#[test]
fn test_payment_provider_runtime_mode_maps_degraded_and_capability_guards() {
    let mut descriptor = MockPaymentProvider::descriptor("slow-gateway");
    descriptor.capabilities.refund = false;
    let provider = MockPaymentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };
    let mut registry = PaymentProviderRegistry::with_manual_provider();
    registry
        .register_external(
            "slow-gateway",
            Arc::new(provider),
            ExternalPaymentProviderRegistration {
                descriptor,
                health: PaymentProviderHealth::Degraded,
                degraded_mode: Some(PaymentProviderDegradedMode {
                    reason: "gateway_timeout".to_string(),
                    fallback_profile: "manual_review".to_string(),
                }),
            },
        )
        .unwrap();

    let mode = registry.runtime_mode("slow-gateway", "authorize").unwrap();
    assert!(mode.can_execute);
    assert_eq!(
        mode.degraded_mode.unwrap().fallback_profile,
        "manual_review"
    );
    assert!(registry.runtime_mode("slow-gateway", "refund").is_err());
    assert!(registry.runtime_mode("slow-gateway", "unknown").is_err());
    assert!(
        registry
            .runtime_mode("missing-gateway", "authorize")
            .is_err()
    );
}

#[tokio::test]
async fn verified_webhook_identity_does_not_require_transport_hints() {
    let registry = registered_mock_registry("mock-gateway");
    let result = registry
        .execute_webhook(
            "mock-gateway",
            PaymentProviderWebhookRequest {
                tenant_id: Uuid::new_v4(),
                provider_id: "mock-gateway".to_string(),
                delivery_id: None,
                idempotency_key: None,
                signature: Some("verified-signature".to_string()),
                raw_payload: br#"{"id":"evt_verified_1"}"#.to_vec(),
            },
        )
        .await
        .unwrap();
    assert_eq!(result.delivery_id, "evt_verified_1");
    assert_eq!(result.replay_key, "evt_verified_1");
}

#[tokio::test]
async fn verified_webhook_identity_rejects_conflicting_transport_hint() {
    let registry = registered_mock_registry("mock-gateway");
    let error = registry
        .execute_webhook(
            "mock-gateway",
            PaymentProviderWebhookRequest {
                tenant_id: Uuid::new_v4(),
                provider_id: "mock-gateway".to_string(),
                delivery_id: Some("wrong-delivery".to_string()),
                idempotency_key: Some("evt_verified_1".to_string()),
                signature: Some("verified-signature".to_string()),
                raw_payload: br#"{"id":"evt_verified_1"}"#.to_vec(),
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, PaymentError::Validation(message) if message.contains("delivery identity"))
    );
}
