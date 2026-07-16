use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Client, StatusCode};
use rust_decimal::Decimal;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::Sha256;
use std::{str::FromStr, sync::Arc};

use crate::{
    PaymentError, PaymentProvider, PaymentProviderCapabilities, PaymentProviderDescriptor,
    PaymentProviderOperationRequest, PaymentProviderOperationResult, PaymentProviderWebhookRequest,
    PaymentProviderWebhookResult, PaymentResult,
};

pub const STRIPE_PAYMENT_PROVIDER_ID: &str = "stripe";
const DEFAULT_STRIPE_API_BASE: &str = "https://api.stripe.com";
const DEFAULT_WEBHOOK_TOLERANCE_SECONDS: i64 = 300;
const MAX_STRIPE_ID_LENGTH: usize = 191;

pub struct StripePaymentProviderConfig {
    pub secret_key: SecretString,
    pub webhook_secret: SecretString,
    pub api_base: String,
    pub webhook_tolerance_seconds: i64,
    pub default_for_new_collections: bool,
}

impl StripePaymentProviderConfig {
    pub fn new(secret_key: SecretString, webhook_secret: SecretString) -> Self {
        Self {
            secret_key,
            webhook_secret,
            api_base: DEFAULT_STRIPE_API_BASE.to_string(),
            webhook_tolerance_seconds: DEFAULT_WEBHOOK_TOLERANCE_SECONDS,
            default_for_new_collections: false,
        }
    }

    pub fn validate(&self) -> PaymentResult<()> {
        if self.secret_key.expose_secret().trim().is_empty() {
            return Err(PaymentError::Validation(
                "stripe secret key must not be empty".to_string(),
            ));
        }
        if self.webhook_secret.expose_secret().trim().is_empty() {
            return Err(PaymentError::Validation(
                "stripe webhook secret must not be empty".to_string(),
            ));
        }
        if !(1..=3600).contains(&self.webhook_tolerance_seconds) {
            return Err(PaymentError::Validation(
                "stripe webhook tolerance must be between 1 and 3600 seconds".to_string(),
            ));
        }
        let api_base = self.api_base.trim();
        if !api_base.starts_with("https://") && !api_base.starts_with("http://localhost") {
            return Err(PaymentError::Validation(
                "stripe api base must use https or localhost".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct StripePaymentProvider {
    client: Client,
    config: Arc<StripePaymentProviderConfig>,
}

impl StripePaymentProvider {
    pub fn new(config: StripePaymentProviderConfig) -> PaymentResult<Self> {
        config.validate()?;
        Ok(Self {
            client: Client::new(),
            config: Arc::new(config),
        })
    }

    pub fn with_client(
        config: StripePaymentProviderConfig,
        client: Client,
    ) -> PaymentResult<Self> {
        config.validate()?;
        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.api_base.trim_end_matches('/'), path)
    }

    async fn post_form<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        idempotency_key: Option<&str>,
        form: &[(String, String)],
    ) -> PaymentResult<T> {
        let mut request = self
            .client
            .post(self.endpoint(path))
            .bearer_auth(self.config.secret_key.expose_secret())
            .form(form);
        if let Some(key) = idempotency_key.map(str::trim).filter(|key| !key.is_empty()) {
            request = request.header("Idempotency-Key", key);
        }
        let response = request.send().await.map_err(|_| {
            PaymentError::Validation("stripe provider request is unavailable".to_string())
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(map_stripe_status(status));
        }
        response.json::<T>().await.map_err(|_| {
            PaymentError::Validation("stripe provider returned an invalid response".to_string())
        })
    }

    fn payment_intent_id(request: &PaymentProviderOperationRequest) -> PaymentResult<String> {
        required_metadata_string(&request.metadata, "provider_payment_id")
    }

    fn operation_metadata(request: &PaymentProviderOperationRequest) -> Value {
        json!({
            "stripe": {
                "collection_id": request.collection_id,
                "currency_code": request.currency_code.to_ascii_uppercase(),
            }
        })
    }

    fn verify_webhook_signature(&self, request: &PaymentProviderWebhookRequest) -> PaymentResult<()> {
        let signature = request
            .signature
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                PaymentError::Validation("stripe signature header is required".to_string())
            })?;
        let parsed = parse_stripe_signature(signature)?;
        let age = (Utc::now().timestamp() - parsed.timestamp).abs();
        if age > self.config.webhook_tolerance_seconds {
            return Err(PaymentError::Validation(
                "stripe webhook timestamp is outside the allowed tolerance".to_string(),
            ));
        }
        let mut signed_payload = parsed.timestamp.to_string().into_bytes();
        signed_payload.push(b'.');
        signed_payload.extend_from_slice(&request.raw_payload);
        let verified = parsed.v1_signatures.iter().any(|signature| {
            let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(
                self.config.webhook_secret.expose_secret().as_bytes(),
            ) else {
                return false;
            };
            mac.update(&signed_payload);
            mac.verify_slice(signature).is_ok()
        });
        if !verified {
            return Err(PaymentError::Validation(
                "stripe webhook signature verification failed".to_string(),
            ));
        }
        Ok(())
    }

    fn normalize_webhook(
        &self,
        request: &PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        self.verify_webhook_signature(request)?;
        let event: StripeEvent = serde_json::from_slice(&request.raw_payload).map_err(|_| {
            PaymentError::Validation("stripe webhook payload is not valid JSON".to_string())
        })?;
        validate_stripe_id(&event.id, "event id")?;
        let object = event.data.object;
        let object_id = required_value_string(&object, "id")?;
        validate_stripe_id(&object_id, "object id")?;
        let currency = required_value_string(&object, "currency")?.to_ascii_uppercase();
        let amount_minor = stripe_event_amount_minor(&event.event_type, &object)?;
        let amount = from_minor_units(amount_minor, currency.as_str())?;
        let object_metadata = object
            .get("metadata")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let (event_type, owner_key, owner_id) = match event.event_type.as_str() {
            "payment_intent.amount_capturable_updated" => (
                "payment.authorized",
                "collection_id",
                required_map_string(&object_metadata, "rustok_collection_id")?,
            ),
            "payment_intent.succeeded" => (
                "payment.captured",
                "collection_id",
                required_map_string(&object_metadata, "rustok_collection_id")?,
            ),
            "payment_intent.canceled" => (
                "payment.cancelled",
                "collection_id",
                required_map_string(&object_metadata, "rustok_collection_id")?,
            ),
            "refund.updated" | "refund.created"
                if object.get("status").and_then(Value::as_str) == Some("succeeded") => (
                    "refund.completed",
                    "refund_id",
                    required_map_string(&object_metadata, "rustok_refund_id")?,
                ),
            other => {
                return Err(PaymentError::Validation(format!(
                    "stripe webhook event `{other}` is unsupported"
                )))
            }
        };
        let mut metadata = Map::new();
        metadata.insert(owner_key.to_string(), Value::String(owner_id));
        metadata.insert("amount".to_string(), Value::String(amount.normalize().to_string()));
        metadata.insert("currency_code".to_string(), Value::String(currency));
        metadata.insert(
            "metadata".to_string(),
            json!({
                "stripe_event_id": event.id,
                "stripe_object_id": object_id,
                "stripe_event_type": event.event_type,
            }),
        );
        Ok(PaymentProviderWebhookResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            delivery_id: event.id.clone(),
            external_reference: Some(object_id),
            event_type: event_type.to_string(),
            replay_key: event.id,
            metadata: Value::Object(metadata),
        })
    }
}

#[async_trait]
impl PaymentProvider for StripePaymentProvider {
    fn descriptor(&self) -> PaymentProviderDescriptor {
        PaymentProviderDescriptor {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            display_name: "Stripe".to_string(),
            capabilities: PaymentProviderCapabilities {
                authorize: true,
                capture: true,
                refund: true,
                cancel: true,
                webhook_ingress: true,
            },
            default_for_new_collections: self.config.default_for_new_collections,
        }
    }

    async fn authorize(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let payment_method = required_metadata_string(&request.metadata, "payment_method_id")?;
        let form = vec![
            ("amount".to_string(), amount.to_string()),
            ("currency".to_string(), request.currency_code.to_ascii_lowercase()),
            ("payment_method".to_string(), payment_method),
            ("confirm".to_string(), "true".to_string()),
            ("capture_method".to_string(), "manual".to_string()),
            (
                "metadata[rustok_collection_id]".to_string(),
                request.collection_id.to_string(),
            ),
        ];
        let intent: StripePaymentIntent = self
            .post_form(
                "/v1/payment_intents",
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "requires_capture" {
            return Err(PaymentError::Validation(format!(
                "stripe payment intent authorization returned status `{}`",
                intent.status
            )));
        }
        let authorized_minor = intent.amount_capturable.max(intent.amount);
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id),
            authorized_amount: from_minor_units(authorized_minor, intent.currency.as_str())?,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request),
        })
    }

    async fn capture(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id, "payment intent id")?;
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let form = vec![("amount_to_capture".to_string(), amount.to_string())];
        let intent: StripePaymentIntent = self
            .post_form(
                format!("/v1/payment_intents/{intent_id}/capture").as_str(),
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "succeeded" {
            return Err(PaymentError::Validation(format!(
                "stripe payment intent capture returned status `{}`",
                intent.status
            )));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id),
            authorized_amount: request.amount,
            captured_amount: from_minor_units(intent.amount_received, intent.currency.as_str())?,
            metadata: Self::operation_metadata(&request),
        })
    }

    async fn cancel(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id, "payment intent id")?;
        let form: Vec<(String, String)> = Vec::new();
        let intent: StripePaymentIntent = self
            .post_form(
                format!("/v1/payment_intents/{intent_id}/cancel").as_str(),
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "canceled" {
            return Err(PaymentError::Validation(format!(
                "stripe payment intent cancel returned status `{}`",
                intent.status
            )));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request),
        })
    }

    async fn refund(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id, "payment intent id")?;
        let refund_id = required_metadata_string(&request.metadata, "refund_id")?;
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let form = vec![
            ("payment_intent".to_string(), intent_id),
            ("amount".to_string(), amount.to_string()),
            ("metadata[rustok_refund_id]".to_string(), refund_id),
        ];
        let refund: StripeRefund = self
            .post_form(
                "/v1/refunds",
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if refund.status.as_deref() != Some("succeeded") {
            return Err(PaymentError::Validation(
                "stripe refund is not completed".to_string(),
            ));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(refund.id),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request),
        })
    }

    async fn handle_webhook(
        &self,
        request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        self.normalize_webhook(&request)
    }
}

#[derive(Debug, Deserialize)]
struct StripePaymentIntent {
    id: String,
    status: String,
    amount: i64,
    #[serde(default)]
    amount_capturable: i64,
    #[serde(default)]
    amount_received: i64,
    currency: String,
}

#[derive(Debug, Deserialize)]
struct StripeRefund {
    id: String,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StripeEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: StripeEventData,
}

#[derive(Debug, Deserialize)]
struct StripeEventData {
    object: Value,
}

struct ParsedStripeSignature {
    timestamp: i64,
    v1_signatures: Vec<Vec<u8>>,
}

fn parse_stripe_signature(value: &str) -> PaymentResult<ParsedStripeSignature> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in value.split(',') {
        let Some((name, value)) = part.trim().split_once('=') else {
            continue;
        };
        match name {
            "t" => {
                timestamp = Some(value.parse::<i64>().map_err(|_| {
                    PaymentError::Validation(
                        "stripe signature timestamp is invalid".to_string(),
                    )
                })?);
            }
            "v1" => signatures.push(decode_hex(value)?),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or_else(|| {
        PaymentError::Validation("stripe signature timestamp is missing".to_string())
    })?;
    if signatures.is_empty() {
        return Err(PaymentError::Validation(
            "stripe signature v1 digest is missing".to_string(),
        ));
    }
    Ok(ParsedStripeSignature {
        timestamp,
        v1_signatures: signatures,
    })
}

fn decode_hex(value: &str) -> PaymentResult<Vec<u8>> {
    if value.len() % 2 != 0 {
        return Err(PaymentError::Validation(
            "stripe signature digest is invalid".to_string(),
        ));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| {
                PaymentError::Validation("stripe signature digest is invalid".to_string())
            })
        })
        .collect()
}

fn map_stripe_status(status: StatusCode) -> PaymentError {
    let message = if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        "stripe provider is temporarily unavailable"
    } else {
        "stripe provider rejected the operation"
    };
    PaymentError::Validation(message.to_string())
}

fn required_metadata_string(metadata: &Value, key: &str) -> PaymentResult<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "stripe operation metadata requires `{key}`"
            ))
        })
}

fn required_value_string(value: &Value, key: &str) -> PaymentResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "stripe webhook object requires `{key}`"
            ))
        })
}

fn required_map_string(values: &Map<String, Value>, key: &str) -> PaymentResult<String> {
    values
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PaymentError::Validation(format!(
                "stripe webhook metadata requires `{key}`"
            ))
        })
}

fn validate_stripe_id(value: &str, label: &str) -> PaymentResult<()> {
    if value.is_empty()
        || value.len() > MAX_STRIPE_ID_LENGTH
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(PaymentError::Validation(format!(
            "stripe {label} is invalid"
        )));
    }
    Ok(())
}

fn stripe_event_amount_minor(event_type: &str, object: &Value) -> PaymentResult<i64> {
    let keys: &[&str] = match event_type {
        "payment_intent.amount_capturable_updated" => &["amount_capturable", "amount"],
        "payment_intent.succeeded" => &["amount_received", "amount"],
        "payment_intent.canceled" => &["amount_capturable", "amount"],
        "refund.updated" | "refund.created" => &["amount"],
        _ => &["amount"],
    };
    keys.iter()
        .filter_map(|key| object.get(*key).and_then(Value::as_i64))
        .find(|amount| *amount > 0)
        .ok_or_else(|| {
            PaymentError::Validation(
                "stripe webhook object has no positive amount".to_string(),
            )
        })
}

fn currency_exponent(currency: &str) -> u32 {
    match currency.to_ascii_uppercase().as_str() {
        "BIF" | "CLP" | "DJF" | "GNF" | "JPY" | "KMF" | "KRW" | "MGA"
        | "PYG" | "RWF" | "UGX" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "JOD" | "KWD" | "OMR" | "TND" => 3,
        _ => 2,
    }
}

fn to_minor_units(amount: Decimal, currency: &str) -> PaymentResult<i64> {
    if amount <= Decimal::ZERO {
        return Err(PaymentError::Validation(
            "stripe amount must be positive".to_string(),
        ));
    }
    let factor = Decimal::from(10u64.pow(currency_exponent(currency)));
    let scaled = amount * factor;
    if scaled.fract() != Decimal::ZERO {
        return Err(PaymentError::Validation(
            "stripe amount has unsupported fractional precision".to_string(),
        ));
    }
    i64::from_str(scaled.normalize().to_string().as_str()).map_err(|_| {
        PaymentError::Validation("stripe amount exceeds supported range".to_string())
    })
}

fn from_minor_units(amount: i64, currency: &str) -> PaymentResult<Decimal> {
    if amount < 0 {
        return Err(PaymentError::Validation(
            "stripe returned a negative amount".to_string(),
        ));
    }
    let factor = Decimal::from(10u64.pow(currency_exponent(currency)));
    Ok(Decimal::from(amount) / factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minor_units_reject_excess_precision() {
        assert_eq!(to_minor_units(Decimal::new(2500, 2), "USD").unwrap(), 2500);
        assert!(to_minor_units(Decimal::new(251, 3), "USD").is_err());
        assert_eq!(to_minor_units(Decimal::new(2500, 2), "JPY").unwrap(), 25);
    }
}
