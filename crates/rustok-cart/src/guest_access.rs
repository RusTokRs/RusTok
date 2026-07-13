use rustok_api::PortContext;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::dto::CartResponse;

pub const GUEST_CART_TOKEN_HEADER: &str = "x-cart-access-token";
pub const GUEST_CART_TOKEN_COOKIE: &str = "rustok-cart-access-token";
const GUEST_CART_TOKEN_HASH_METADATA_KEY: &str = "__rustok_guest_cart_token_sha256";
const GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY: &str = "__rustok_guest_cart_access_token";
const GUEST_CART_CLAIM_PREFIX: &str = "cart:guest:";

pub fn prepare_guest_cart_metadata(
    customer_id: Option<Uuid>,
    metadata: Value,
) -> (Value, Option<String>) {
    if customer_id.is_some() {
        return (sanitize_guest_cart_metadata(metadata), None);
    }

    let token = format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let mut object = metadata.as_object().cloned().unwrap_or_default();
    object.remove(GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY);
    object.insert(
        GUEST_CART_TOKEN_HASH_METADATA_KEY.to_string(),
        Value::String(hash_guest_cart_token(&token)),
    );
    (Value::Object(object), Some(token))
}

pub fn sanitize_guest_cart_metadata(metadata: Value) -> Value {
    let Some(mut object) = metadata.as_object().cloned() else {
        return metadata;
    };
    object.remove(GUEST_CART_TOKEN_HASH_METADATA_KEY);
    object.remove(GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY);
    Value::Object(object)
}

pub fn attach_transient_guest_token(cart: &mut CartResponse, token: String) {
    let mut metadata = cart.metadata.as_object().cloned().unwrap_or_default();
    metadata.insert(
        GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY.to_string(),
        Value::String(token),
    );
    cart.metadata = Value::Object(metadata);
}

pub fn take_transient_guest_token(cart: &mut CartResponse) -> Option<String> {
    let object = cart.metadata.as_object_mut()?;
    object
        .remove(GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY)
        .and_then(|value| value.as_str().map(str::to_string))
}

pub fn guest_cart_claim(token: &str) -> Option<String> {
    normalize_guest_cart_token(token).map(|token| format!("{GUEST_CART_CLAIM_PREFIX}{token}"))
}

pub fn guest_cart_token_from_context(context: &PortContext) -> Option<&str> {
    context
        .claims
        .iter()
        .find_map(|claim| claim.strip_prefix(GUEST_CART_CLAIM_PREFIX))
        .and_then(normalize_guest_cart_token)
}

pub fn verify_guest_cart_token(metadata: &Value, presented_token: Option<&str>) -> bool {
    let Some(expected_hash) = metadata
        .get(GUEST_CART_TOKEN_HASH_METADATA_KEY)
        .and_then(Value::as_str)
    else {
        return false;
    };
    let Some(token) = presented_token.and_then(normalize_guest_cart_token) else {
        return false;
    };

    constant_time_hex_eq(expected_hash, &hash_guest_cart_token(token))
}

pub fn hash_guest_cart_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn normalize_guest_cart_token(token: &str) -> Option<&str> {
    let token = token.trim();
    if !(32..=256).contains(&token.len())
        || !token
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
    {
        return None;
    }
    Some(token)
}

fn constant_time_hex_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |diff, (left, right)| diff | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn guest_metadata_stores_only_hash_and_returns_transient_token() {
        let (metadata, token) = prepare_guest_cart_metadata(None, json!({"source": "store"}));
        let token = token.expect("guest token");

        assert!(metadata.get(GUEST_CART_TOKEN_HASH_METADATA_KEY).is_some());
        assert!(!metadata.to_string().contains(&token));
        assert!(verify_guest_cart_token(&metadata, Some(&token)));
        assert!(!verify_guest_cart_token(&metadata, Some("wrong-token-value-12345678901234567890")));
    }

    #[test]
    fn customer_cart_does_not_receive_guest_capability() {
        let (metadata, token) =
            prepare_guest_cart_metadata(Some(Uuid::new_v4()), json!({"source": "account"}));
        assert!(token.is_none());
        assert!(metadata.get(GUEST_CART_TOKEN_HASH_METADATA_KEY).is_none());
    }

    #[test]
    fn sanitization_removes_reserved_security_fields() {
        let sanitized = sanitize_guest_cart_metadata(json!({
            GUEST_CART_TOKEN_HASH_METADATA_KEY: "hash",
            GUEST_CART_TRANSIENT_TOKEN_METADATA_KEY: "token",
            "public": true
        }));
        assert_eq!(sanitized, json!({"public": true}));
    }
}