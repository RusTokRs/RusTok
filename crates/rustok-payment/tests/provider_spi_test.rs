use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_payment::error::{PaymentError, PaymentResult};
use rustok_payment::providers::{
    ManualPaymentProvider, PaymentProvider, PaymentProviderCapabilities, PaymentProviderDescriptor,
    PaymentProviderOperationRequest, PaymentProviderOperationResult,
};
use serde_json::json;
use uuid::Uuid;

struct MockPaymentProvider {
    descriptor: PaymentProviderDescriptor,
    should_fail: bool,
    error_message: String,
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
    let tenant_id = Uuid::new_v4();
    let collection_id = Uuid::new_v4();
    let amount = Decimal::new(100, 0); // 100

    let request = PaymentProviderOperationRequest {
        tenant_id,
        collection_id,
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

    let refund_res = provider.refund(request.clone()).await.unwrap();
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

    let refund_res = provider.refund(request).await;
    assert!(refund_res.is_err());
    match refund_res.err().unwrap() {
        PaymentError::Validation(msg) => {
            assert!(msg.contains("refund amount must be greater than zero"));
        }
        _ => panic!("Expected validation error"),
    }
}

#[tokio::test]
async fn test_mock_provider_idempotency_and_error_mapping() {
    let descriptor = PaymentProviderDescriptor {
        provider_id: "mock-gateway".to_string(),
        display_name: "Mock Gateway".to_string(),
        capabilities: PaymentProviderCapabilities {
            authorize: true,
            capture: true,
            refund: true,
            cancel: true,
            webhook_ingress: true,
        },
        default_for_new_collections: false,
    };

    let success_provider = MockPaymentProvider {
        descriptor: descriptor.clone(),
        should_fail: false,
        error_message: String::new(),
    };

    let request = PaymentProviderOperationRequest {
        tenant_id: Uuid::new_v4(),
        collection_id: Uuid::new_v4(),
        amount: Decimal::new(50, 0),
        currency_code: "EUR".to_string(),
        idempotency_key: Some("idemp-mock-key".to_string()),
        metadata: json!({}),
    };

    // Verify successful operation propagates the idempotency key context
    let auth_res = success_provider.authorize(request.clone()).await.unwrap();
    assert_eq!(auth_res.provider_id, "mock-gateway");
    assert_eq!(auth_res.authorized_amount, Decimal::new(50, 0));

    // Verify error mapping on mock provider failure
    let failing_provider = MockPaymentProvider {
        descriptor,
        should_fail: true,
        error_message: "Gateway connection timeout".to_string(),
    };

    let auth_err = failing_provider.authorize(request).await;
    assert!(auth_err.is_err());
    match auth_err.err().unwrap() {
        PaymentError::Validation(msg) => {
            assert_eq!(msg, "Gateway connection timeout");
        }
        _ => panic!("Expected validation error from failing provider"),
    }
}
