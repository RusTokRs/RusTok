use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Client, StatusCode, Url};
use rust_decimal::Decimal;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::Sha256;
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use uuid::Uuid;

use crate::{
    PaymentError, PaymentProvider, PaymentProviderCapabilities, PaymentProviderDescriptor,
    PaymentProviderOperationRequest, PaymentProviderOperationResult, PaymentProviderWebhookRequest,
    PaymentProviderWebhookResult, PaymentResult,
};

pub const STRIPE_PAYMENT_PROVIDER_ID: &str = "stripe";
const DEFAULT_STRIPE_API_BASE: &str = "https://api.stripe.com";
const DEFAULT_WEBHOOK_TOLERANCE_SECONDS: i64 = 300;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 30;
const MAX_STRIPE_ID_LENGTH: usize = 191;
const WEBHOOK_OPERATION: &str = "webhook";

#[derive(Clone)]
pub struct StripeCredentials {
    pub secret_key: SecretString,
    pub webhook_secret: SecretString,
}

impl StripeCredentials {
    pub fn new(secret_key: SecretString, webhook_secret: SecretString) -> PaymentResult<Self> {
        let credentials = Self {
            secret_key,
            webhook_secret,
        };
        credentials.validate()?;
        Ok(credentials)
    }

    fn validate(&self) -> PaymentResult<()> {
        if self.secret_key.expose_secret().trim().is_empty()
            || self.webhook_secret.expose_secret().trim().is_empty()
        {
            return Err(PaymentError::provider_configuration(
                STRIPE_PAYMENT_PROVIDER_ID,
            ));
        }
        Ok(())
    }
}

#[async_trait]
pub trait StripeCredentialProvider: Send + Sync {
    async fn credentials_for_tenant(&self, tenant_id: Uuid) -> PaymentResult<StripeCredentials>;
}

/// Explicit static credential map for tests and local single-process profiles.
/// Production hosts must resolve tenant-owned secret references instead.
#[derive(Clone, Default)]
pub struct StaticStripeCredentialProvider {
    credentials: Arc<HashMap<Uuid, StripeCredentials>>,
}

impl StaticStripeCredentialProvider {
    pub fn new(credentials: HashMap<Uuid, StripeCredentials>) -> Self {
        Self {
            credentials: Arc::new(credentials),
        }
    }

    pub fn for_tenant(tenant_id: Uuid, credentials: StripeCredentials) -> Self {
        Self::new(HashMap::from([(tenant_id, credentials)]))
    }
}

#[async_trait]
impl StripeCredentialProvider for StaticStripeCredentialProvider {
    async fn credentials_for_tenant(&self, tenant_id: Uuid) -> PaymentResult<StripeCredentials> {
        self.credentials
            .get(&tenant_id)
            .cloned()
            .ok_or_else(|| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))
    }
}

#[derive(Clone)]
pub struct StripePaymentProviderConfig {
    pub api_base: String,
    pub webhook_tolerance_seconds: i64,
    pub request_timeout_seconds: u64,
    pub default_for_new_collections: bool,
}

impl Default for StripePaymentProviderConfig {
    fn default() -> Self {
        Self {
            api_base: DEFAULT_STRIPE_API_BASE.to_string(),
            webhook_tolerance_seconds: DEFAULT_WEBHOOK_TOLERANCE_SECONDS,
            request_timeout_seconds: DEFAULT_REQUEST_TIMEOUT_SECONDS,
            default_for_new_collections: false,
        }
    }
}

impl StripePaymentProviderConfig {
    pub fn validate(&self) -> PaymentResult<()> {
        if !(1..=3600).contains(&self.webhook_tolerance_seconds)
            || !(1..=120).contains(&self.request_timeout_seconds)
        {
            return Err(PaymentError::provider_configuration(
                STRIPE_PAYMENT_PROVIDER_ID,
            ));
        }
        validate_api_base(self.api_base.as_str())
    }
}

#[derive(Clone)]
pub struct StripePaymentProvider {
    client: Client,
    config: StripePaymentProviderConfig,
    credentials: Arc<dyn StripeCredentialProvider>,
}

impl StripePaymentProvider {
    pub fn new(
        config: StripePaymentProviderConfig,
        credentials: Arc<dyn StripeCredentialProvider>,
    ) -> PaymentResult<Self> {
        config.validate()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .build()
            .map_err(|_| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))?;
        Ok(Self {
            client,
            config,
            credentials,
        })
    }

    /// Test and host-integration seam for a preconfigured client. The host remains
    /// responsible for applying an equivalent bounded timeout.
    pub fn with_client(
        config: StripePaymentProviderConfig,
        credentials: Arc<dyn StripeCredentialProvider>,
        client: Client,
    ) -> PaymentResult<Self> {
        config.validate()?;
        Ok(Self {
            client,
            config,
            credentials,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.api_base.trim_end_matches('/'), path)
    }

    async fn resolve_credentials(&self, tenant_id: Uuid) -> PaymentResult<StripeCredentials> {
        let credentials = self.credentials.credentials_for_tenant(tenant_id).await?;
        credentials.validate()?;
        Ok(credentials)
    }

    async fn post_form<T: for<'de> Deserialize<'de>>(
        &self,
        credentials: &StripeCredentials,
        operation: &str,
        path: &str,
        idempotency_key: Option<&str>,
        form: &[(String, String)],
    ) -> PaymentResult<T> {
        let mut request = self
            .client
            .post(self.endpoint(path))
            .bearer_auth(credentials.secret_key.expose_secret())
            .form(form);
        if let Some(key) = idempotency_key.map(str::trim).filter(|key| !key.is_empty()) {
            request = request.header("Idempotency-Key", key);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_connect() {
                PaymentError::provider_unavailable(STRIPE_PAYMENT_PROVIDER_ID, operation)
            } else {
                PaymentError::provider_outcome_unknown(STRIPE_PAYMENT_PROVIDER_ID, operation)
            }
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(map_stripe_status(status, operation));
        }
        response.json::<T>().await.map_err(|_| {
            PaymentError::provider_outcome_unknown(STRIPE_PAYMENT_PROVIDER_ID, operation)
        })
    }

    fn payment_intent_id(request: &PaymentProviderOperationRequest) -> PaymentResult<String> {
        required_metadata_string(&request.metadata, "provider_payment_id")
    }

    fn operation_metadata(
        request: &PaymentProviderOperationRequest,
        provider_payment_id: Option<&str>,
    ) -> Value {
        json!({
            "stripe": {
                "collection_id": request.collection_id,
                "currency_code": request.currency_code.to_ascii_uppercase(),
                "provider_payment_id": provider_payment_id,
            }
        })
    }

    fn verify_webhook_signature(
        &self,
        credentials: &StripeCredentials,
        request: &PaymentProviderWebhookRequest,
    ) -> PaymentResult<()> {
        let signature = request
            .signature
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(webhook_rejected)?;
        let parsed = parse_stripe_signature(signature)?;
        let age = Utc::now().timestamp().abs_diff(parsed.timestamp);
        if age > self.config.webhook_tolerance_seconds as u64 {
            return Err(webhook_rejected());
        }
        let mut signed_payload = parsed.timestamp.to_string().into_bytes();
        signed_payload.push(b'.');
        signed_payload.extend_from_slice(&request.raw_payload);
        let verified = parsed.v1_signatures.iter().any(|signature| {
            let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(
                credentials.webhook_secret.expose_secret().as_bytes(),
            ) else {
                return false;
            };
            mac.update(&signed_payload);
            mac.verify_slice(signature).is_ok()
        });
        if !verified {
            return Err(webhook_rejected());
        }
        Ok(())
    }

    fn normalize_webhook(
        &self,
        credentials: &StripeCredentials,
        request: &PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        self.verify_webhook_signature(credentials, request)?;
        let event: StripeEvent =
            serde_json::from_slice(&request.raw_payload).map_err(|_| webhook_invalid_response())?;
        validate_webhook_id(&event.id)?;
        let object = event.data.object;
        let object_id = required_webhook_value(&object, "id")?;
        validate_webhook_id(&object_id)?;
        let currency = required_webhook_value(&object, "currency")?.to_ascii_uppercase();
        let amount_minor = stripe_event_amount_minor(&event.event_type, &object)?;
        let amount = from_minor_units(amount_minor, currency.as_str())
            .map_err(|_| webhook_invalid_response())?;
        let object_metadata = object
            .get("metadata")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let (event_type, owner_key, owner_id) = match event.event_type.as_str() {
            "payment_intent.amount_capturable_updated" => (
                "payment.authorized",
                "collection_id",
                required_webhook_metadata(&object_metadata, "rustok_collection_id")?,
            ),
            "payment_intent.succeeded" => (
                "payment.captured",
                "collection_id",
                required_webhook_metadata(&object_metadata, "rustok_collection_id")?,
            ),
            "payment_intent.canceled" => (
                "payment.cancelled",
                "collection_id",
                required_webhook_metadata(&object_metadata, "rustok_collection_id")?,
            ),
            "refund.updated" | "refund.created"
                if object.get("status").and_then(Value::as_str) == Some("succeeded") =>
            {
                (
                    "refund.completed",
                    "refund_id",
                    required_webhook_metadata(&object_metadata, "rustok_refund_id")?,
                )
            }
            _ => return Err(webhook_rejected()),
        };
        let mut metadata = Map::new();
        metadata.insert(owner_key.to_string(), Value::String(owner_id));
        metadata.insert(
            "amount".to_string(),
            Value::String(amount.normalize().to_string()),
        );
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
        let credentials = self.resolve_credentials(request.tenant_id).await?;
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let payment_method = required_metadata_string(&request.metadata, "payment_method_id")?;
        let form = vec![
            ("amount".to_string(), amount.to_string()),
            (
                "currency".to_string(),
                request.currency_code.to_ascii_lowercase(),
            ),
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
                &credentials,
                "authorize",
                "/v1/payment_intents",
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "requires_capture" {
            return Err(PaymentError::provider_outcome_unknown(
                STRIPE_PAYMENT_PROVIDER_ID,
                "authorize",
            ));
        }
        let authorized_minor = if intent.amount_capturable > 0 {
            intent.amount_capturable
        } else {
            intent.amount
        };
        let authorized_amount = from_minor_units(authorized_minor, intent.currency.as_str())
            .map_err(|_| {
                PaymentError::provider_outcome_unknown(STRIPE_PAYMENT_PROVIDER_ID, "authorize")
            })?;
        let provider_payment_id = intent.id.clone();
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id),
            authorized_amount,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request, Some(provider_payment_id.as_str())),
        })
    }

    async fn capture(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let credentials = self.resolve_credentials(request.tenant_id).await?;
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id)?;
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let form = vec![("amount_to_capture".to_string(), amount.to_string())];
        let path = format!("/v1/payment_intents/{intent_id}/capture");
        let intent: StripePaymentIntent = self
            .post_form(
                &credentials,
                "capture",
                path.as_str(),
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "succeeded" {
            return Err(PaymentError::provider_outcome_unknown(
                STRIPE_PAYMENT_PROVIDER_ID,
                "capture",
            ));
        }
        let captured_amount = from_minor_units(intent.amount_received, intent.currency.as_str())
            .map_err(|_| {
                PaymentError::provider_outcome_unknown(STRIPE_PAYMENT_PROVIDER_ID, "capture")
            })?;
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id.clone()),
            authorized_amount: request.amount,
            captured_amount,
            metadata: Self::operation_metadata(&request, Some(intent.id.as_str())),
        })
    }

    async fn cancel(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let credentials = self.resolve_credentials(request.tenant_id).await?;
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id)?;
        let form: Vec<(String, String)> = Vec::new();
        let path = format!("/v1/payment_intents/{intent_id}/cancel");
        let intent: StripePaymentIntent = self
            .post_form(
                &credentials,
                "cancel",
                path.as_str(),
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if intent.status != "canceled" {
            return Err(PaymentError::provider_outcome_unknown(
                STRIPE_PAYMENT_PROVIDER_ID,
                "cancel",
            ));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(intent.id.clone()),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request, Some(intent.id.as_str())),
        })
    }

    async fn refund(
        &self,
        request: PaymentProviderOperationRequest,
    ) -> PaymentResult<PaymentProviderOperationResult> {
        let credentials = self.resolve_credentials(request.tenant_id).await?;
        let intent_id = Self::payment_intent_id(&request)?;
        validate_stripe_id(&intent_id)?;
        let refund_id = required_metadata_string(&request.metadata, "refund_id")?;
        let amount = to_minor_units(request.amount, request.currency_code.as_str())?;
        let form = vec![
            ("payment_intent".to_string(), intent_id.clone()),
            ("amount".to_string(), amount.to_string()),
            ("metadata[rustok_refund_id]".to_string(), refund_id),
        ];
        let refund: StripeRefund = self
            .post_form(
                &credentials,
                "refund",
                "/v1/refunds",
                request.idempotency_key.as_deref(),
                form.as_slice(),
            )
            .await?;
        if refund.status.as_deref() != Some("succeeded") {
            return Err(PaymentError::provider_outcome_unknown(
                STRIPE_PAYMENT_PROVIDER_ID,
                "refund",
            ));
        }
        Ok(PaymentProviderOperationResult {
            provider_id: STRIPE_PAYMENT_PROVIDER_ID.to_string(),
            external_reference: Some(refund.id),
            authorized_amount: Decimal::ZERO,
            captured_amount: Decimal::ZERO,
            metadata: Self::operation_metadata(&request, Some(intent_id.as_str())),
        })
    }

    async fn handle_webhook(
        &self,
        request: PaymentProviderWebhookRequest,
    ) -> PaymentResult<PaymentProviderWebhookResult> {
        let credentials = self.resolve_credentials(request.tenant_id).await?;
        self.normalize_webhook(&credentials, &request)
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
            "t" => timestamp = Some(value.parse::<i64>().map_err(|_| webhook_rejected())?),
            "v1" => signatures.push(decode_hex(value)?),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or_else(webhook_rejected)?;
    if signatures.is_empty() {
        return Err(webhook_rejected());
    }
    Ok(ParsedStripeSignature {
        timestamp,
        v1_signatures: signatures,
    })
}

fn decode_hex(value: &str) -> PaymentResult<Vec<u8>> {
    if value.is_empty() || !value.is_ascii() || value.len() % 2 != 0 {
        return Err(webhook_rejected());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| webhook_rejected())
        })
        .collect()
}

fn validate_api_base(value: &str) -> PaymentResult<()> {
    let url = Url::parse(value.trim())
        .map_err(|_| PaymentError::provider_configuration(STRIPE_PAYMENT_PROVIDER_ID))?;
    if url.query().is_some() || url.fragment().is_some() {
        return Err(PaymentError::provider_configuration(
            STRIPE_PAYMENT_PROVIDER_ID,
        ));
    }
    let local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !local_http {
        return Err(PaymentError::provider_configuration(
            STRIPE_PAYMENT_PROVIDER_ID,
        ));
    }
    Ok(())
}

fn map_stripe_status(status: StatusCode, operation: &str) -> PaymentError {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        PaymentError::provider_unavailable(STRIPE_PAYMENT_PROVIDER_ID, operation)
    } else {
        PaymentError::provider_rejected(STRIPE_PAYMENT_PROVIDER_ID, operation)
    }
}

fn webhook_rejected() -> PaymentError {
    PaymentError::provider_rejected(STRIPE_PAYMENT_PROVIDER_ID, WEBHOOK_OPERATION)
}

fn webhook_invalid_response() -> PaymentError {
    PaymentError::provider_invalid_response(STRIPE_PAYMENT_PROVIDER_ID, WEBHOOK_OPERATION)
}

fn required_metadata_string(metadata: &Value, key: &str) -> PaymentResult<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PaymentError::Validation(format!("stripe operation metadata requires `{key}`"))
        })
}

fn required_webhook_value(value: &Value, key: &str) -> PaymentResult<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(webhook_invalid_response)
}

fn required_webhook_metadata(values: &Map<String, Value>, key: &str) -> PaymentResult<String> {
    values
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(webhook_invalid_response)
}

fn validate_stripe_id(value: &str) -> PaymentResult<()> {
    if !valid_stripe_id(value) {
        return Err(PaymentError::Validation(
            "stripe provider payment identity is invalid".to_string(),
        ));
    }
    Ok(())
}

fn validate_webhook_id(value: &str) -> PaymentResult<()> {
    if !valid_stripe_id(value) {
        return Err(webhook_invalid_response());
    }
    Ok(())
}

fn valid_stripe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_STRIPE_ID_LENGTH
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
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
        .ok_or_else(webhook_invalid_response)
}

fn currency_exponent(currency: &str) -> u32 {
    match currency.to_ascii_uppercase().as_str() {
        "BIF" | "CLP" | "DJF" | "GNF" | "JPY" | "KMF" | "KRW" | "MGA" | "PYG" | "RWF" | "UGX"
        | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
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
    i64::from_str(scaled.normalize().to_string().as_str())
        .map_err(|_| PaymentError::Validation("stripe amount exceeds supported range".to_string()))
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

    #[test]
    fn api_base_rejects_non_loopback_http_and_url_suffixes() {
        assert!(validate_api_base("http://localhost:12111").is_ok());
        assert!(validate_api_base("http://127.0.0.1:12111").is_ok());
        assert!(validate_api_base("http://localhost.evil.example").is_err());
        assert!(validate_api_base("https://api.stripe.com?secret=x").is_err());
    }

    #[test]
    fn signature_hex_rejects_unicode_without_panicking() {
        assert!(decode_hex("éé").is_err());
        assert!(decode_hex("").is_err());
    }
}
