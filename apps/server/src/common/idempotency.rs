//! Host HTTP normalization for UUID-backed idempotency receipts.

use axum::http::HeaderMap;
use uuid::Uuid;

use crate::error::{Error, Result};

pub(crate) const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

/// Accept one non-nil UUID, bare or enclosed in exactly one pair of quotes.
/// Do not repair malformed wire identities into valid receipt keys.
pub(crate) fn optional_uuid_header(headers: &HeaderMap) -> Result<Option<Uuid>> {
    let mut values = headers.get_all(IDEMPOTENCY_KEY_HEADER).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(Error::BadRequest(
            "Duplicate Idempotency-Key headers are forbidden".to_string(),
        ));
    }
    let value = first
        .to_str()
        .map_err(|_| Error::BadRequest("Idempotency-Key header is invalid".to_string()))?
        .trim();
    let invalid_uuid = || Error::BadRequest("Idempotency-Key header must be a UUID".to_string());
    let value = if let Some(quoted) = value.strip_prefix('"') {
        quoted.strip_suffix('"').ok_or_else(invalid_uuid)?
    } else {
        value
    };
    let key = value.parse::<Uuid>().map_err(|_| invalid_uuid())?;
    if key.is_nil() {
        return Err(Error::BadRequest(
            "Idempotency-Key header must not be the nil UUID".to_string(),
        ));
    }
    Ok(Some(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn malformed_quoting_and_multiple_values_do_not_alias_valid_keys() {
        let key = Uuid::new_v4();
        let mut headers = HeaderMap::new();
        for invalid in [
            format!("\"{key}"),
            format!("{key}\""),
            format!("\"\"{key}\"\""),
            format!("\"{key} \""),
            format!("{key}, {key}"),
        ] {
            headers.insert(
                IDEMPOTENCY_KEY_HEADER,
                HeaderValue::from_str(&invalid).expect("header value"),
            );
            assert!(matches!(
                optional_uuid_header(&headers),
                Err(Error::BadRequest(_))
            ));
        }
        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&key.to_string()).expect("bare UUID"),
        );
        headers.append(
            IDEMPOTENCY_KEY_HEADER,
            HeaderValue::from_str(&key.to_string()).expect("duplicate UUID"),
        );
        assert!(matches!(
            optional_uuid_header(&headers),
            Err(Error::BadRequest(_))
        ));
    }
}
