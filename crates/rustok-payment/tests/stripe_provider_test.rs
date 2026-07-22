#![cfg(feature = "stripe")]

use chrono::Utc;
use hmac::{Hmac, Mac};
use rustok_payment::{
    PaymentError, PaymentProvider, PaymentProviderWebhookRequest, StaticStripeCredentialProvider,
    StripeCredentials, StripePaymentProvider, StripePaymentProviderConfig,
};
use secrecy::SecretString;
use sha2::Sha256;
use std::sync::Arc;
use uuid::Uuid;

fn provider(tenant_id: Uuid, webhook_secret: &str) -> StripePaymentProvider {
    let credentials = StripeCredentials::new(
        SecretString::from("sk_test_local".to_string()),
        SecretString::from(webhook_secret.to_string()),
    )
    .unwrap();
    StripePaymentProvider::new(
        StripePaymentProviderConfig::default(),
        Arc::new(StaticStripeCredentialProvider::for_tenant(
            tenant_id,
            credentials,
        )),
    )
    .unwrap()
}

fn signature(secret: &str, timestamp: i64, payload: &[u8]) -> String {
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(&signed);
    format!(
        "t={timestamp},v1={}",
        hex(mac.finalize().into_bytes().as_slice())
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn authorized_payload(collection_id: Uuid) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "id": "evt_rustok_authorized_1",
        "type": "payment_intent.amount_capturable_updated",
        "data": {
            "object": {
                "id": "pi_rustok_authorized_1",
                "currency": "usd",
                "amount": 2500,
                "amount_capturable": 2500,
                "metadata": {
                    "rustok_collection_id": collection_id.to_string()
                }
            }
        }
    }))
    .unwrap()
}

#[tokio::test]
async fn stripe_webhook_uses_signature_verified_event_identity() {
    let tenant_id = Uuid::new_v4();
    let collection_id = Uuid::new_v4();
    let collection_id_text = collection_id.to_string();
    let secret = "whsec_rustok_test";
    let payload = authorized_payload(collection_id);
    let timestamp = Utc::now().timestamp();
    let result = provider(tenant_id, secret)
        .handle_webhook(PaymentProviderWebhookRequest {
            tenant_id,
            provider_id: "stripe".to_string(),
            delivery_id: None,
            idempotency_key: None,
            signature: Some(signature(secret, timestamp, payload.as_slice())),
            raw_payload: payload,
        })
        .await
        .unwrap();

    assert_eq!(result.delivery_id, "evt_rustok_authorized_1");
    assert_eq!(result.replay_key, "evt_rustok_authorized_1");
    assert_eq!(result.event_type, "payment.authorized");
    assert_eq!(
        result
            .metadata
            .get("collection_id")
            .and_then(serde_json::Value::as_str),
        Some(collection_id_text.as_str())
    );
}

#[tokio::test]
async fn stripe_webhook_rejects_body_changed_after_signature() {
    let tenant_id = Uuid::new_v4();
    let secret = "whsec_rustok_test";
    let original = authorized_payload(Uuid::new_v4());
    let timestamp = Utc::now().timestamp();
    let header = signature(secret, timestamp, original.as_slice());
    let changed = authorized_payload(Uuid::new_v4());

    let error = provider(tenant_id, secret)
        .handle_webhook(PaymentProviderWebhookRequest {
            tenant_id,
            provider_id: "stripe".to_string(),
            delivery_id: None,
            idempotency_key: None,
            signature: Some(header),
            raw_payload: changed,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        PaymentError::ProviderRejected {
            ref provider_id,
            ref operation,
        } if provider_id == "stripe" && operation == "webhook"
    ));
}

#[tokio::test]
async fn stripe_missing_tenant_credentials_are_configuration_error() {
    let tenant_id = Uuid::new_v4();
    let provider = StripePaymentProvider::new(
        StripePaymentProviderConfig::default(),
        Arc::new(StaticStripeCredentialProvider::default()),
    )
    .unwrap();
    let payload = authorized_payload(Uuid::new_v4());
    let error = provider
        .handle_webhook(PaymentProviderWebhookRequest {
            tenant_id,
            provider_id: "stripe".to_string(),
            delivery_id: None,
            idempotency_key: None,
            signature: Some("t=1,v1=00".to_string()),
            raw_payload: payload,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        PaymentError::ProviderConfiguration { ref provider_id } if provider_id == "stripe"
    ));
}
