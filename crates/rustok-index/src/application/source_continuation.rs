use std::{collections::BTreeMap, fmt, sync::Arc, time::Duration};

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{LocaleKey, SchemaRef};

use super::{IndexSourceCursor, SharedIndexSourceRegistry};

const CONTINUATION_DOMAIN: &[u8] = b"rustok-index-source-continuation";
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: usize = 16;
const MAX_KEY_ID_BYTES: usize = 64;
const MAX_KEYS: usize = 16;
const MIN_LIFETIME_MILLIS: u128 = 1_000;
const MAX_LIFETIME_MILLIS: u128 = 15 * 60 * 1_000;
const MAX_CLOCK_SKEW_MILLIS: i64 = 30 * 1_000;
const MAX_PLAINTEXT_BYTES: usize = 10 * 1024;
const MAX_DECODED_TOKEN_BYTES: usize = 12 * 1024;
const MAX_ENCODED_TOKEN_BYTES: usize = 16 * 1024;

/// Canonical tenant/schema/source/locale identity to which a continuation token is sealed.
///
/// The scope can only be constructed from the frozen source registry. This prevents a transport
/// from inventing a source name independently of the schema owner selected during composition.
/// `locale = None` is the schema-wide scan identity; `Some(LocaleKey)` is one exact canonical locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSourceContinuationScope {
    tenant_id: Uuid,
    schema: SchemaRef,
    owner_module: String,
    source_name: String,
    locale: Option<LocaleKey>,
}

impl IndexSourceContinuationScope {
    /// Construct the canonical schema-wide continuation scope.
    pub fn from_registry(
        tenant_id: Uuid,
        schema: SchemaRef,
        sources: &SharedIndexSourceRegistry,
    ) -> Result<Self, IndexSourceContinuationError> {
        Self::from_registry_with_locale(tenant_id, schema, None, sources)
    }

    /// Construct the canonical exact-locale continuation scope.
    pub fn for_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: LocaleKey,
        sources: &SharedIndexSourceRegistry,
    ) -> Result<Self, IndexSourceContinuationError> {
        Self::from_registry_with_locale(tenant_id, schema, Some(locale), sources)
    }

    fn from_registry_with_locale(
        tenant_id: Uuid,
        schema: SchemaRef,
        locale: Option<LocaleKey>,
        sources: &SharedIndexSourceRegistry,
    ) -> Result<Self, IndexSourceContinuationError> {
        if tenant_id.is_nil() {
            return Err(IndexSourceContinuationError::NilTenantId);
        }
        let descriptor = sources
            .source_for_schema(&schema)
            .ok_or_else(|| IndexSourceContinuationError::UnknownSchemaSource(schema.clone()))?;
        Ok(Self {
            tenant_id,
            schema,
            owner_module: descriptor.owner_module().to_owned(),
            source_name: descriptor.source_name().to_owned(),
            locale,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }

    pub fn owner_module(&self) -> &str {
        &self.owner_module
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn locale(&self) -> Option<&LocaleKey> {
        self.locale.as_ref()
    }
}

/// Bounded encoded continuation value suitable for a transport boundary.
///
/// Debug deliberately reveals only length. Callers must use `as_str` explicitly when serializing
/// the token and must never log it as an identifier.
#[derive(Clone, PartialEq, Eq)]
pub struct IndexSourceContinuationToken(String);

impl IndexSourceContinuationToken {
    pub fn parse(encoded: impl Into<String>) -> Result<Self, IndexSourceContinuationError> {
        let encoded = encoded.into();
        if encoded.is_empty() {
            return Err(IndexSourceContinuationError::EmptyToken);
        }
        if encoded.len() > MAX_ENCODED_TOKEN_BYTES {
            return Err(IndexSourceContinuationError::TokenTooLarge {
                actual: encoded.len(),
                max: MAX_ENCODED_TOKEN_BYTES,
            });
        }
        Ok(Self(encoded))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for IndexSourceContinuationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceContinuationToken")
            .field("encoded_bytes", &self.0.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContinuationClaims {
    tenant_id: Uuid,
    schema: SchemaRef,
    owner_module: String,
    source_name: String,
    locale: Option<LocaleKey>,
    issued_at_unix_millis: i64,
    expires_at_unix_millis: i64,
    cursor: IndexSourceCursor,
}

/// Authenticated and confidential codec for owner-source continuation state.
///
/// One active key is used for sealing. Every key retained in the bounded keyring can decrypt, so a
/// key can be rotated without invalidating already-issued tokens until its maximum lifetime passes.
/// The repository keeps one current unversioned envelope; superseded pre-release token formats are
/// not decoded or retained as compatibility paths.
#[derive(Clone)]
pub struct IndexSourceContinuationCodec {
    active_key_id: String,
    keys: Arc<BTreeMap<String, [u8; KEY_BYTES]>>,
}

impl IndexSourceContinuationCodec {
    pub fn new(
        active_key_id: impl Into<String>,
        keys: BTreeMap<String, [u8; KEY_BYTES]>,
    ) -> Result<Self, IndexSourceContinuationError> {
        let active_key_id = active_key_id.into();
        validate_key_id(&active_key_id)?;
        if keys.is_empty() || keys.len() > MAX_KEYS {
            return Err(IndexSourceContinuationError::InvalidKeyringSize {
                actual: keys.len(),
                max: MAX_KEYS,
            });
        }
        for (key_id, key) in &keys {
            validate_key_id(key_id)?;
            if key.iter().all(|byte| *byte == 0) {
                return Err(IndexSourceContinuationError::InvalidKeyMaterial(
                    key_id.clone(),
                ));
            }
        }
        if !keys.contains_key(&active_key_id) {
            return Err(IndexSourceContinuationError::ActiveKeyUnavailable(
                active_key_id,
            ));
        }
        Ok(Self {
            active_key_id,
            keys: Arc::new(keys),
        })
    }

    pub fn active_key_id(&self) -> &str {
        &self.active_key_id
    }

    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Seal one raw source cursor under the canonical frozen source and locale identity.
    pub fn seal(
        &self,
        scope: &IndexSourceContinuationScope,
        cursor: &IndexSourceCursor,
        issued_at: DateTime<Utc>,
        lifetime: Duration,
    ) -> Result<IndexSourceContinuationToken, IndexSourceContinuationError> {
        let lifetime_millis = lifetime.as_millis();
        if !(MIN_LIFETIME_MILLIS..=MAX_LIFETIME_MILLIS).contains(&lifetime_millis) {
            return Err(IndexSourceContinuationError::InvalidLifetime {
                actual_millis: lifetime_millis,
                min_millis: MIN_LIFETIME_MILLIS,
                max_millis: MAX_LIFETIME_MILLIS,
            });
        }
        let lifetime_millis = i64::try_from(lifetime_millis)
            .map_err(|_| IndexSourceContinuationError::TimestampOverflow)?;
        let issued_at_unix_millis = issued_at.timestamp_millis();
        let expires_at_unix_millis = issued_at_unix_millis
            .checked_add(lifetime_millis)
            .ok_or(IndexSourceContinuationError::TimestampOverflow)?;
        let claims = ContinuationClaims {
            tenant_id: scope.tenant_id,
            schema: scope.schema.clone(),
            owner_module: scope.owner_module.clone(),
            source_name: scope.source_name.clone(),
            locale: scope.locale.clone(),
            issued_at_unix_millis,
            expires_at_unix_millis,
            cursor: cursor.clone(),
        };
        let plaintext = postcard::to_stdvec(&claims)?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(IndexSourceContinuationError::PlaintextTooLarge {
                actual: plaintext.len(),
                max: MAX_PLAINTEXT_BYTES,
            });
        }

        let key = self.keys.get(&self.active_key_id).ok_or_else(|| {
            IndexSourceContinuationError::ActiveKeyUnavailable(self.active_key_id.clone())
        })?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| {
            IndexSourceContinuationError::InvalidKeyMaterial(self.active_key_id.clone())
        })?;
        let mut nonce = [0_u8; NONCE_BYTES];
        let random_bytes = Uuid::new_v4();
        nonce.copy_from_slice(&random_bytes.as_bytes()[..NONCE_BYTES]);
        let aad = associated_data(&self.active_key_id);
        let ciphertext = cipher
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| IndexSourceContinuationError::EncryptionFailed)?;
        let key_id_bytes = self.active_key_id.as_bytes();
        let mut decoded =
            Vec::with_capacity(1 + key_id_bytes.len() + NONCE_BYTES + ciphertext.len());
        decoded.push(key_id_bytes.len() as u8);
        decoded.extend_from_slice(key_id_bytes);
        decoded.extend_from_slice(&nonce);
        decoded.extend_from_slice(&ciphertext);
        if decoded.len() > MAX_DECODED_TOKEN_BYTES {
            return Err(IndexSourceContinuationError::DecodedTokenTooLarge {
                actual: decoded.len(),
                max: MAX_DECODED_TOKEN_BYTES,
            });
        }
        let encoded = URL_SAFE_NO_PAD.encode(decoded);
        IndexSourceContinuationToken::parse(encoded)
    }

    /// Authenticate, decrypt, scope-check, and expire one token before a source scan is built.
    pub fn open_encoded(
        &self,
        expected_scope: &IndexSourceContinuationScope,
        encoded: &str,
        now: DateTime<Utc>,
    ) -> Result<IndexSourceCursor, IndexSourceContinuationError> {
        let token = IndexSourceContinuationToken::parse(encoded.to_owned())?;
        self.open(expected_scope, &token, now)
    }

    pub fn open(
        &self,
        expected_scope: &IndexSourceContinuationScope,
        token: &IndexSourceContinuationToken,
        now: DateTime<Utc>,
    ) -> Result<IndexSourceCursor, IndexSourceContinuationError> {
        let decoded = URL_SAFE_NO_PAD.decode(token.as_str())?;
        if decoded.len() > MAX_DECODED_TOKEN_BYTES {
            return Err(IndexSourceContinuationError::DecodedTokenTooLarge {
                actual: decoded.len(),
                max: MAX_DECODED_TOKEN_BYTES,
            });
        }
        if decoded.len() < 1 + 1 + NONCE_BYTES + TAG_BYTES {
            return Err(IndexSourceContinuationError::MalformedEnvelope);
        }
        let key_id_len = decoded[0] as usize;
        if !(1..=MAX_KEY_ID_BYTES).contains(&key_id_len) {
            return Err(IndexSourceContinuationError::MalformedEnvelope);
        }
        let key_id_end = 1_usize
            .checked_add(key_id_len)
            .ok_or(IndexSourceContinuationError::MalformedEnvelope)?;
        let nonce_end = key_id_end
            .checked_add(NONCE_BYTES)
            .ok_or(IndexSourceContinuationError::MalformedEnvelope)?;
        let minimum_len = nonce_end
            .checked_add(TAG_BYTES)
            .ok_or(IndexSourceContinuationError::MalformedEnvelope)?;
        if decoded.len() < minimum_len {
            return Err(IndexSourceContinuationError::MalformedEnvelope);
        }
        let key_id = std::str::from_utf8(&decoded[1..key_id_end])
            .map_err(|_| IndexSourceContinuationError::MalformedEnvelope)?;
        validate_key_id(key_id)?;
        let key = self
            .keys
            .get(key_id)
            .ok_or_else(|| IndexSourceContinuationError::KeyUnavailable(key_id.to_owned()))?;
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|_| IndexSourceContinuationError::InvalidKeyMaterial(key_id.to_owned()))?;
        let nonce: &[u8; NONCE_BYTES] = decoded[key_id_end..nonce_end]
            .try_into()
            .map_err(|_| IndexSourceContinuationError::MalformedEnvelope)?;
        let ciphertext = &decoded[nonce_end..];
        let aad = associated_data(key_id);
        let plaintext = cipher
            .decrypt(
                nonce.into(),
                Payload {
                    msg: ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| IndexSourceContinuationError::InvalidToken)?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(IndexSourceContinuationError::PlaintextTooLarge {
                actual: plaintext.len(),
                max: MAX_PLAINTEXT_BYTES,
            });
        }
        let claims: ContinuationClaims = postcard::from_bytes(&plaintext)?;
        validate_claims(&claims, expected_scope, now)?;
        Ok(claims.cursor)
    }
}

impl fmt::Debug for IndexSourceContinuationCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IndexSourceContinuationCodec")
            .field("active_key_id", &self.active_key_id)
            .field("key_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Error)]
pub enum IndexSourceContinuationError {
    #[error("Index source continuation tenant id must not be nil")]
    NilTenantId,
    #[error("No Index source owns continuation schema {0}")]
    UnknownSchemaSource(SchemaRef),
    #[error("Index source continuation key id is invalid: {0}")]
    InvalidKeyId(String),
    #[error("Index source continuation key material is invalid: {0}")]
    InvalidKeyMaterial(String),
    #[error("Index source continuation keyring size is invalid: actual={actual}, max={max}")]
    InvalidKeyringSize { actual: usize, max: usize },
    #[error("Index source continuation active key is unavailable: {0}")]
    ActiveKeyUnavailable(String),
    #[error("Index source continuation token key is unavailable: {0}")]
    KeyUnavailable(String),
    #[error(
        "Index source continuation lifetime is invalid: actual={actual_millis}ms, min={min_millis}ms, max={max_millis}ms"
    )]
    InvalidLifetime {
        actual_millis: u128,
        min_millis: u128,
        max_millis: u128,
    },
    #[error("Index source continuation timestamp overflow")]
    TimestampOverflow,
    #[error("Index source continuation token must not be empty")]
    EmptyToken,
    #[error("Index source continuation token is too large: actual={actual}, max={max}")]
    TokenTooLarge { actual: usize, max: usize },
    #[error("Index source continuation decoded token is too large: actual={actual}, max={max}")]
    DecodedTokenTooLarge { actual: usize, max: usize },
    #[error("Index source continuation plaintext is too large: actual={actual}, max={max}")]
    PlaintextTooLarge { actual: usize, max: usize },
    #[error("Index source continuation encoding is invalid")]
    Base64(#[from] base64::DecodeError),
    #[error("Index source continuation envelope is malformed")]
    MalformedEnvelope,
    #[error("Index source continuation encryption failed")]
    EncryptionFailed,
    #[error("Index source continuation token authentication failed")]
    InvalidToken,
    #[error("Index source continuation payload serialization failed")]
    Postcard(#[from] postcard::Error),
    #[error("Index source continuation tenant does not match request scope")]
    TenantMismatch,
    #[error("Index source continuation schema does not match request scope")]
    SchemaMismatch,
    #[error("Index source continuation owner module does not match the frozen source")]
    SourceOwnerMismatch,
    #[error("Index source continuation source name does not match the frozen source")]
    SourceNameMismatch,
    #[error("Index source continuation locale scope does not match request scope")]
    LocaleScopeMismatch,
    #[error("Index source continuation claims contain an invalid lifetime")]
    InvalidClaimsLifetime,
    #[error("Index source continuation token was issued too far in the future")]
    IssuedAtInFuture,
    #[error("Index source continuation token has expired")]
    Expired,
}

fn validate_claims(
    claims: &ContinuationClaims,
    expected_scope: &IndexSourceContinuationScope,
    now: DateTime<Utc>,
) -> Result<(), IndexSourceContinuationError> {
    if claims.tenant_id != expected_scope.tenant_id {
        return Err(IndexSourceContinuationError::TenantMismatch);
    }
    if claims.schema != expected_scope.schema {
        return Err(IndexSourceContinuationError::SchemaMismatch);
    }
    if claims.owner_module != expected_scope.owner_module {
        return Err(IndexSourceContinuationError::SourceOwnerMismatch);
    }
    if claims.source_name != expected_scope.source_name {
        return Err(IndexSourceContinuationError::SourceNameMismatch);
    }
    if claims.locale != expected_scope.locale {
        return Err(IndexSourceContinuationError::LocaleScopeMismatch);
    }
    let lifetime = claims
        .expires_at_unix_millis
        .checked_sub(claims.issued_at_unix_millis)
        .ok_or(IndexSourceContinuationError::InvalidClaimsLifetime)?;
    if lifetime < MIN_LIFETIME_MILLIS as i64 || lifetime > MAX_LIFETIME_MILLIS as i64 {
        return Err(IndexSourceContinuationError::InvalidClaimsLifetime);
    }
    let now = now.timestamp_millis();
    let latest_acceptable_issue = now
        .checked_add(MAX_CLOCK_SKEW_MILLIS)
        .ok_or(IndexSourceContinuationError::TimestampOverflow)?;
    if claims.issued_at_unix_millis > latest_acceptable_issue {
        return Err(IndexSourceContinuationError::IssuedAtInFuture);
    }
    if claims.expires_at_unix_millis <= now {
        return Err(IndexSourceContinuationError::Expired);
    }
    Ok(())
}

fn associated_data(key_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(CONTINUATION_DOMAIN.len() + 1 + key_id.len());
    aad.extend_from_slice(CONTINUATION_DOMAIN);
    aad.push(key_id.len() as u8);
    aad.extend_from_slice(key_id.as_bytes());
    aad
}

fn validate_key_id(key_id: &str) -> Result<(), IndexSourceContinuationError> {
    if key_id.is_empty()
        || key_id.len() > MAX_KEY_ID_BYTES
        || !key_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(IndexSourceContinuationError::InvalidKeyId(
            key_id.to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, time::Duration};

    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::domain::{EntityName, ModuleName, SchemaVersion};

    fn schema() -> SchemaRef {
        SchemaRef {
            module: ModuleName::new("source-continuation").unwrap(),
            entity: EntityName::new("item").unwrap(),
            version: SchemaVersion::INITIAL,
        }
    }

    fn scope(tenant_id: Uuid) -> IndexSourceContinuationScope {
        IndexSourceContinuationScope {
            tenant_id,
            schema: schema(),
            owner_module: "source-continuation".to_string(),
            source_name: "source-continuation-primary".to_string(),
            locale: None,
        }
    }

    fn locale_scope(tenant_id: Uuid, locale: &str) -> IndexSourceContinuationScope {
        IndexSourceContinuationScope {
            tenant_id,
            schema: schema(),
            owner_module: "source-continuation".to_string(),
            source_name: "source-continuation-primary".to_string(),
            locale: Some(LocaleKey::new(locale).unwrap()),
        }
    }

    fn codec(active: &str, keys: &[(&str, u8)]) -> IndexSourceContinuationCodec {
        IndexSourceContinuationCodec::new(
            active,
            keys.iter()
                .map(|(key_id, byte)| ((*key_id).to_string(), [*byte; KEY_BYTES]))
                .collect::<BTreeMap<_, _>>(),
        )
        .unwrap()
    }

    fn now() -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_780_000_000_000)
            .single()
            .unwrap()
    }

    fn cursor() -> IndexSourceCursor {
        IndexSourceCursor::new(json!({
            "after": "internal-owner-id",
            "partition": 7,
        }))
        .unwrap()
    }

    #[test]
    fn sealed_cursor_round_trips_only_under_exact_scope() {
        let tenant_id = Uuid::new_v4();
        let base_scope = scope(tenant_id);
        let codec = codec("current", &[("current", 7)]);
        let token = codec
            .seal(&base_scope, &cursor(), now(), Duration::from_secs(300))
            .unwrap();

        assert_eq!(codec.open(&base_scope, &token, now()).unwrap(), cursor());

        let other_tenant = scope(Uuid::new_v4());
        assert!(matches!(
            codec.open(&other_tenant, &token, now()),
            Err(IndexSourceContinuationError::TenantMismatch)
        ));

        let mut other_schema = base_scope.clone();
        other_schema.schema.version = SchemaVersion::new(2);
        assert!(matches!(
            codec.open(&other_schema, &token, now()),
            Err(IndexSourceContinuationError::SchemaMismatch)
        ));
    }

    #[test]
    fn schema_wide_and_exact_locale_continuations_cannot_cross_scopes() {
        let tenant_id = Uuid::new_v4();
        let schema_wide = scope(tenant_id);
        let locale_alias = locale_scope(tenant_id, "EN-us");
        let locale_canonical = locale_scope(tenant_id, "en-US");
        let other_locale = locale_scope(tenant_id, "de");
        assert_eq!(locale_alias.locale(), locale_canonical.locale());

        let codec = codec("current", &[("current", 8)]);
        let schema_wide_token = codec
            .seal(&schema_wide, &cursor(), now(), Duration::from_secs(300))
            .unwrap();
        assert!(matches!(
            codec.open(&locale_canonical, &schema_wide_token, now()),
            Err(IndexSourceContinuationError::LocaleScopeMismatch)
        ));

        let locale_token = codec
            .seal(&locale_alias, &cursor(), now(), Duration::from_secs(300))
            .unwrap();
        assert_eq!(
            codec.open(&locale_canonical, &locale_token, now()).unwrap(),
            cursor()
        );
        assert!(matches!(
            codec.open(&schema_wide, &locale_token, now()),
            Err(IndexSourceContinuationError::LocaleScopeMismatch)
        ));
        assert!(matches!(
            codec.open(&other_locale, &locale_token, now()),
            Err(IndexSourceContinuationError::LocaleScopeMismatch)
        ));
    }

    #[test]
    fn tampering_fails_authentication() {
        let scope = scope(Uuid::new_v4());
        let codec = codec("current", &[("current", 9)]);
        let token = codec
            .seal(&scope, &cursor(), now(), Duration::from_secs(60))
            .unwrap();
        let mut decoded = URL_SAFE_NO_PAD.decode(token.as_str()).unwrap();
        *decoded.last_mut().unwrap() ^= 1;
        let tampered =
            IndexSourceContinuationToken::parse(URL_SAFE_NO_PAD.encode(decoded)).unwrap();

        assert!(matches!(
            codec.open(&scope, &tampered, now()),
            Err(IndexSourceContinuationError::InvalidToken)
        ));
    }

    #[test]
    fn expiry_and_future_issue_time_fail_before_cursor_return() {
        let scope = scope(Uuid::new_v4());
        let codec = codec("current", &[("current", 11)]);
        let token = codec
            .seal(&scope, &cursor(), now(), Duration::from_secs(60))
            .unwrap();
        assert!(matches!(
            codec.open(&scope, &token, now() + chrono::Duration::seconds(60)),
            Err(IndexSourceContinuationError::Expired)
        ));

        let future = codec
            .seal(
                &scope,
                &cursor(),
                now() + chrono::Duration::seconds(31),
                Duration::from_secs(60),
            )
            .unwrap();
        assert!(matches!(
            codec.open(&scope, &future, now()),
            Err(IndexSourceContinuationError::IssuedAtInFuture)
        ));
    }

    #[test]
    fn rotation_decodes_retained_old_key_and_rejects_removed_key() {
        let scope = scope(Uuid::new_v4());
        let old = codec("old", &[("old", 13)]);
        let token = old
            .seal(&scope, &cursor(), now(), Duration::from_secs(300))
            .unwrap();

        let rotated = codec("new", &[("new", 17), ("old", 13)]);
        assert_eq!(rotated.open(&scope, &token, now()).unwrap(), cursor());

        let retired = codec("new", &[("new", 17)]);
        assert!(matches!(
            retired.open(&scope, &token, now()),
            Err(IndexSourceContinuationError::KeyUnavailable(key_id)) if key_id == "old"
        ));
    }

    #[test]
    fn token_and_codec_debug_do_not_expose_secret_material() {
        let scope = scope(Uuid::new_v4());
        let codec = codec("current", &[("current", 23)]);
        let token = codec
            .seal(&scope, &cursor(), now(), Duration::from_secs(60))
            .unwrap();

        let codec_debug = format!("{codec:?}");
        assert!(codec_debug.contains("key_count"));
        assert!(!codec_debug.contains("23"));
        let token_debug = format!("{token:?}");
        assert!(token_debug.contains("encoded_bytes"));
        assert!(!token_debug.contains(token.as_str()));
    }

    #[test]
    fn invalid_lifetime_and_oversized_encoded_input_fail_closed() {
        let scope = scope(Uuid::new_v4());
        let codec = codec("current", &[("current", 29)]);
        assert!(matches!(
            codec.seal(&scope, &cursor(), now(), Duration::from_millis(999)),
            Err(IndexSourceContinuationError::InvalidLifetime { .. })
        ));
        assert!(matches!(
            codec.open_encoded(&scope, &"a".repeat(MAX_ENCODED_TOKEN_BYTES + 1), now()),
            Err(IndexSourceContinuationError::TokenTooLarge { .. })
        ));
    }
}
