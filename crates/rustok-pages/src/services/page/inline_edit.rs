use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use rustok_api::{Action, Resource, SHA256_DIGEST_BYTES, fixed_work_sha256_eq, hmac_sha256};
use rustok_core::{
    SecurityActorKind, SecurityContext,
    error::{ErrorKind, RichError},
};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{page_body, page_translation};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_owned_scope;

use super::PageService;
use super::document::{ensure_document_is_mutable, page_document_revision};
use super::helpers::normalize_locale;

pub const PAGE_INLINE_EDIT_GRANT_INVALID: &str = "PAGE_INLINE_EDIT_GRANT_INVALID";
pub const PAGE_INLINE_EDIT_GRANT_EXPIRED: &str = "PAGE_INLINE_EDIT_GRANT_EXPIRED";
pub const PAGE_INLINE_EDIT_CONTEXT_MISMATCH: &str = "PAGE_INLINE_EDIT_CONTEXT_MISMATCH";
pub const PAGE_INLINE_EDIT_DOCUMENT_UNAVAILABLE: &str = "PAGE_INLINE_EDIT_DOCUMENT_UNAVAILABLE";

pub const PAGE_INLINE_EDIT_GRANT_VERSION: u16 = 1;
pub const DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS: u64 = 60_000;
pub const MAX_PAGE_INLINE_EDIT_GRANT_TTL_MS: u64 = 300_000;
pub const DEFAULT_PAGE_INLINE_EDIT_CLOCK_SKEW_MS: u64 = 2_000;
pub const MAX_PAGE_INLINE_EDIT_KEYS: usize = 8;
pub const MAX_PAGE_INLINE_EDIT_KEY_ID_BYTES: usize = 64;
pub const MAX_PAGE_INLINE_EDIT_PROOF_BYTES: usize = 32 * 1024;
pub const MAX_PAGE_INLINE_EDIT_PAYLOAD_BYTES: usize = 16 * 1024;

const MIN_PAGE_INLINE_EDIT_SECRET_BYTES: usize = 32;
const MAX_PAGE_INLINE_EDIT_SECRET_BYTES: usize = 4_096;
const MAX_INLINE_EDIT_ID_BYTES: usize = 255;
const MAX_INLINE_EDIT_REVISION_BYTES: usize = 512;
const PAGE_INLINE_EDIT_SIGNATURE_DOMAIN: &[u8] = b"rustok-pages-inline-edit-grant-v1\0";
const KEY_ID_SEPARATOR: &[u8] = b"\0";

#[derive(Clone, Eq, PartialEq)]
pub struct PageInlineEditSecret(String);

impl PageInlineEditSecret {
    pub fn new(value: impl AsRef<str>) -> Result<Self, PageInlineEditConfigError> {
        let value = value.as_ref();
        if value.len() < MIN_PAGE_INLINE_EDIT_SECRET_BYTES
            || value.len() > MAX_PAGE_INLINE_EDIT_SECRET_BYTES
            || !value.is_ascii()
            || value
                .as_bytes()
                .iter()
                .any(|byte| *byte <= b' ' || *byte == 0x7f)
        {
            return Err(PageInlineEditConfigError::InvalidSecret);
        }
        Ok(Self(value.to_string()))
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Debug for PageInlineEditSecret {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PageInlineEditSecret")
            .field(&"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct PageInlineEditKeyId(String);

impl PageInlineEditKeyId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, PageInlineEditConfigError> {
        let value = value.as_ref();
        if value.is_empty()
            || value.len() > MAX_PAGE_INLINE_EDIT_KEY_ID_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(PageInlineEditConfigError::InvalidKeyId);
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for PageInlineEditKeyId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PageInlineEditKeyId([CONFIGURED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageInlineEditConfigError {
    #[error("Pages inline edit secret must be 32..=4096 visible non-whitespace ASCII bytes")]
    InvalidSecret,
    #[error(
        "Pages inline edit key id must be 1..=64 ASCII letters, digits, dot, underscore, or hyphen"
    )]
    InvalidKeyId,
    #[error("Pages inline edit keyring must contain 1..=8 unique keys")]
    InvalidKeyCount,
    #[error("Pages inline edit active key must exist in the keyring")]
    ActiveKeyMissing,
    #[error("Pages inline edit grant TTL must be within 1..=300000 milliseconds")]
    InvalidTtl,
}

#[derive(Clone)]
pub struct PageInlineEditKeyring {
    active_key_id: PageInlineEditKeyId,
    keys: Arc<BTreeMap<PageInlineEditKeyId, PageInlineEditSecret>>,
    ttl_ms: u64,
    clock_skew_ms: u64,
}

impl PageInlineEditKeyring {
    pub fn single(secret: PageInlineEditSecret) -> Self {
        let key_id = PageInlineEditKeyId("active".to_string());
        let mut keys = BTreeMap::new();
        keys.insert(key_id.clone(), secret);
        Self {
            active_key_id: key_id,
            keys: Arc::new(keys),
            ttl_ms: DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS,
            clock_skew_ms: DEFAULT_PAGE_INLINE_EDIT_CLOCK_SKEW_MS,
        }
    }

    pub fn new(
        active_key_id: PageInlineEditKeyId,
        keys: Vec<(PageInlineEditKeyId, PageInlineEditSecret)>,
    ) -> Result<Self, PageInlineEditConfigError> {
        if keys.is_empty() || keys.len() > MAX_PAGE_INLINE_EDIT_KEYS {
            return Err(PageInlineEditConfigError::InvalidKeyCount);
        }
        let mut by_id = BTreeMap::new();
        for (key_id, secret) in keys {
            if by_id.insert(key_id, secret).is_some() {
                return Err(PageInlineEditConfigError::InvalidKeyCount);
            }
        }
        if !by_id.contains_key(&active_key_id) {
            return Err(PageInlineEditConfigError::ActiveKeyMissing);
        }
        Ok(Self {
            active_key_id,
            keys: Arc::new(by_id),
            ttl_ms: DEFAULT_PAGE_INLINE_EDIT_GRANT_TTL_MS,
            clock_skew_ms: DEFAULT_PAGE_INLINE_EDIT_CLOCK_SKEW_MS,
        })
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Result<Self, PageInlineEditConfigError> {
        let ttl_ms =
            u64::try_from(ttl.as_millis()).map_err(|_| PageInlineEditConfigError::InvalidTtl)?;
        if ttl_ms == 0 || ttl_ms > MAX_PAGE_INLINE_EDIT_GRANT_TTL_MS {
            return Err(PageInlineEditConfigError::InvalidTtl);
        }
        self.ttl_ms = ttl_ms;
        Ok(self)
    }

    pub fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }

    pub fn active_key_id(&self) -> &PageInlineEditKeyId {
        &self.active_key_id
    }

    pub fn issue(
        &self,
        context: PageInlineEditGrantContext,
        now_unix_ms: u64,
    ) -> PagesResult<IssuedPageInlineEditGrant> {
        context.validate()?;
        let expires_at_unix_ms = now_unix_ms
            .checked_add(self.ttl_ms)
            .ok_or_else(invalid_inline_edit_grant)?;
        let claims = PageInlineEditGrantClaims {
            version: PAGE_INLINE_EDIT_GRANT_VERSION,
            tenant_id: context.tenant_id,
            actor_id: context.actor_id,
            auth_session_id: context.auth_session_id,
            session_id: context.session_id,
            pages_page_id: context.pages_page_id,
            fly_page_id: context.fly_page_id,
            locale: context.locale,
            revision_id: context.revision_id,
            project_hash: context.project_hash,
            channel_id: context.channel_id,
            channel_slug: context.channel_slug,
            issued_at_unix_ms: now_unix_ms,
            expires_at_unix_ms,
        };
        let payload = serde_json::to_string(&claims).map_err(|_| invalid_inline_edit_grant())?;
        if payload.len() > MAX_PAGE_INLINE_EDIT_PAYLOAD_BYTES {
            return Err(invalid_inline_edit_grant());
        }
        let key_id = self.active_key_id.as_str().to_string();
        let secret = self
            .keys
            .get(&self.active_key_id)
            .expect("validated inline edit keyring must retain active key");
        let signature = inline_edit_signature(secret, &key_id, payload.as_bytes());
        let authorization_proof = serde_json::to_string(&SignedPageInlineEditGrant {
            key_id,
            payload,
            signature,
        })
        .map_err(|_| invalid_inline_edit_grant())?;
        if authorization_proof.len() > MAX_PAGE_INLINE_EDIT_PROOF_BYTES {
            return Err(invalid_inline_edit_grant());
        }
        Ok(IssuedPageInlineEditGrant {
            claims,
            authorization_proof,
        })
    }

    pub fn verify(
        &self,
        authorization_proof: &str,
        now_unix_ms: u64,
    ) -> PagesResult<PageInlineEditGrantClaims> {
        if authorization_proof.len() > MAX_PAGE_INLINE_EDIT_PROOF_BYTES {
            return Err(invalid_inline_edit_grant());
        }
        let signed = serde_json::from_str::<SignedPageInlineEditGrant>(authorization_proof)
            .map_err(|_| invalid_inline_edit_grant())?;
        if signed.payload.len() > MAX_PAGE_INLINE_EDIT_PAYLOAD_BYTES {
            return Err(invalid_inline_edit_grant());
        }
        let key_id =
            PageInlineEditKeyId::new(&signed.key_id).map_err(|_| invalid_inline_edit_grant())?;
        let secret = self
            .keys
            .get(&key_id)
            .ok_or_else(invalid_inline_edit_grant)?;
        let expected = inline_edit_signature(secret, &signed.key_id, signed.payload.as_bytes());
        if !fixed_work_sha256_eq(&expected, &signed.signature) {
            return Err(invalid_inline_edit_grant());
        }
        let claims = serde_json::from_str::<PageInlineEditGrantClaims>(&signed.payload)
            .map_err(|_| invalid_inline_edit_grant())?;
        claims.validate(now_unix_ms, self.ttl_ms, self.clock_skew_ms)?;
        Ok(claims)
    }
}

impl Debug for PageInlineEditKeyring {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PageInlineEditKeyring")
            .field("active_key_id", &"[CONFIGURED]")
            .field("key_count", &self.keys.len())
            .field("ttl_ms", &self.ttl_ms)
            .field("clock_skew_ms", &self.clock_skew_ms)
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageInlineEditGrantClaims {
    pub version: u16,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub auth_session_id: Uuid,
    pub session_id: Uuid,
    pub pages_page_id: Uuid,
    pub fly_page_id: String,
    pub locale: String,
    pub revision_id: String,
    pub project_hash: u64,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl PageInlineEditGrantClaims {
    fn validate(&self, now_unix_ms: u64, max_ttl_ms: u64, clock_skew_ms: u64) -> PagesResult<()> {
        let ttl_ms = self
            .expires_at_unix_ms
            .checked_sub(self.issued_at_unix_ms)
            .ok_or_else(invalid_inline_edit_grant)?;
        if self.version != PAGE_INLINE_EDIT_GRANT_VERSION
            || ttl_ms == 0
            || ttl_ms > max_ttl_ms
            || self.issued_at_unix_ms > now_unix_ms.saturating_add(clock_skew_ms)
        {
            return Err(invalid_inline_edit_grant());
        }
        if self.expires_at_unix_ms <= now_unix_ms {
            return Err(expired_inline_edit_grant());
        }
        PageInlineEditGrantContext {
            tenant_id: self.tenant_id,
            actor_id: self.actor_id,
            auth_session_id: self.auth_session_id,
            session_id: self.session_id,
            pages_page_id: self.pages_page_id,
            fly_page_id: self.fly_page_id.clone(),
            locale: self.locale.clone(),
            revision_id: self.revision_id.clone(),
            project_hash: self.project_hash,
            channel_id: self.channel_id,
            channel_slug: self.channel_slug.clone(),
        }
        .validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInlineEditGrantContext {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub auth_session_id: Uuid,
    pub session_id: Uuid,
    pub pages_page_id: Uuid,
    pub fly_page_id: String,
    pub locale: String,
    pub revision_id: String,
    pub project_hash: u64,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

impl PageInlineEditGrantContext {
    fn validate(&self) -> PagesResult<()> {
        if self.tenant_id.is_nil()
            || self.actor_id.is_nil()
            || self.auth_session_id.is_nil()
            || self.session_id.is_nil()
            || self.pages_page_id.is_nil()
            || !bounded_required(&self.fly_page_id, MAX_INLINE_EDIT_ID_BYTES)
            || !bounded_required(&self.locale, MAX_INLINE_EDIT_ID_BYTES)
            || !bounded_required(&self.revision_id, MAX_INLINE_EDIT_REVISION_BYTES)
            || self
                .channel_slug
                .as_ref()
                .is_some_and(|value| !bounded_required(value, MAX_INLINE_EDIT_ID_BYTES))
        {
            return Err(invalid_inline_edit_grant());
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IssuedPageInlineEditGrant {
    pub claims: PageInlineEditGrantClaims,
    authorization_proof: String,
}

impl IssuedPageInlineEditGrant {
    pub fn authorization_proof(&self) -> &str {
        &self.authorization_proof
    }
}

impl Debug for IssuedPageInlineEditGrant {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedPageInlineEditGrant")
            .field("claims", &self.claims)
            .field("authorization_proof", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageInlineEditDocument {
    pub pages_page_id: Uuid,
    pub locale: String,
    pub revision_id: String,
    pub project_data: Value,
}

impl PageService {
    pub async fn load_inline_edit_document(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        page_id: Uuid,
        locale: &str,
    ) -> PagesResult<PageInlineEditDocument> {
        self.ensure_builder_enabled(tenant_id).await?;
        self.ensure_builder_inline_edit_enabled_for_tenant(tenant_id)
            .await?;
        let page = self.find_page(tenant_id, page_id).await?;
        enforce_owned_scope(&security, Resource::Pages, Action::Update, page.author_id)?;
        if security.actor_kind != SecurityActorKind::User || security.user_id.is_none() {
            return Err(inline_edit_context_mismatch(
                "Authenticated user authority is required for inline editing",
            ));
        }
        ensure_document_is_mutable(&page)?;
        let locale = normalize_locale(locale)?;
        let translation_exists = page_translation::Entity::find()
            .filter(page_translation::Column::TenantId.eq(tenant_id))
            .filter(page_translation::Column::PageId.eq(page_id))
            .filter(page_translation::Column::Locale.eq(&locale))
            .one(&self.db)
            .await?
            .is_some();
        if !translation_exists {
            return Err(inline_edit_document_unavailable(
                "Inline edit locale does not have a matching page translation",
            ));
        }
        let body = page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::PageId.eq(page_id))
            .filter(page_body::Column::Locale.eq(&locale))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                inline_edit_document_unavailable(
                    "Inline editing requires an existing localized page body",
                )
            })?;
        if body.format != PAGE_BUILDER_DOCUMENT_FORMAT {
            return Err(inline_edit_document_unavailable(
                "Inline editing accepts only the current Fly/GrapesJS document format",
            ));
        }
        let project_data = serde_json::from_str(&body.content).map_err(|_| {
            inline_edit_document_unavailable(
                "The localized Fly/GrapesJS page body is not valid JSON",
            )
        })?;
        Ok(PageInlineEditDocument {
            pages_page_id: page_id,
            locale,
            revision_id: page_document_revision(page_id, Some(&body)),
            project_data,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct SignedPageInlineEditGrant {
    key_id: String,
    payload: String,
    signature: [u8; SHA256_DIGEST_BYTES],
}

impl Debug for SignedPageInlineEditGrant {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignedPageInlineEditGrant")
            .field("key_id", &"[CONFIGURED]")
            .field("payload_bytes", &self.payload.len())
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

fn inline_edit_signature(
    secret: &PageInlineEditSecret,
    key_id: &str,
    payload: &[u8],
) -> [u8; SHA256_DIGEST_BYTES] {
    hmac_sha256(
        secret.as_bytes(),
        &[
            PAGE_INLINE_EDIT_SIGNATURE_DOMAIN,
            key_id.as_bytes(),
            KEY_ID_SEPARATOR,
            payload,
        ],
    )
}

fn bounded_required(value: &str, max_bytes: usize) -> bool {
    let value = value.trim();
    !value.is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

pub fn inline_edit_context_mismatch(message: impl Into<String>) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Forbidden, message.into())
            .with_user_message("This inline edit grant does not match the current request.")
            .with_error_code(PAGE_INLINE_EDIT_CONTEXT_MISMATCH),
    ))
}

fn invalid_inline_edit_grant() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(
            ErrorKind::Forbidden,
            "Pages inline edit grant failed integrity validation",
        )
        .with_user_message("The inline edit session is invalid. Reload the page and try again.")
        .with_error_code(PAGE_INLINE_EDIT_GRANT_INVALID),
    ))
}

fn expired_inline_edit_grant() -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Conflict, "Pages inline edit grant has expired")
            .with_user_message("The inline edit session expired. Reload the page and try again.")
            .with_error_code(PAGE_INLINE_EDIT_GRANT_EXPIRED),
    ))
}

fn inline_edit_document_unavailable(message: impl Into<String>) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Conflict, message.into())
            .with_user_message("This page document is not available for inline editing.")
            .with_error_code(PAGE_INLINE_EDIT_DOCUMENT_UNAVAILABLE),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secret(value: &str) -> PageInlineEditSecret {
        PageInlineEditSecret::new(value).expect("secret")
    }

    fn context() -> PageInlineEditGrantContext {
        PageInlineEditGrantContext {
            tenant_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            auth_session_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            pages_page_id: Uuid::new_v4(),
            fly_page_id: "home".to_string(),
            locale: "en".to_string(),
            revision_id: "2026-08-06T09:00:00Z".to_string(),
            project_hash: 7,
            channel_id: Some(Uuid::new_v4()),
            channel_slug: Some("web".to_string()),
        }
    }

    #[test]
    fn grant_roundtrip_binds_auth_and_edit_sessions_and_redacts_secret_material() {
        let keyring =
            PageInlineEditKeyring::single(secret("pages-inline-edit-secret-material-00000001"));
        let issued = keyring.issue(context(), 1_000).expect("issued");
        let verified = keyring
            .verify(issued.authorization_proof(), 2_000)
            .expect("verified");
        assert_eq!(verified, issued.claims);
        assert_ne!(verified.auth_session_id, verified.session_id);
        assert!(!format!("{keyring:?}").contains("secret-material"));
        assert!(!format!("{issued:?}").contains(issued.authorization_proof()));
    }

    #[test]
    fn grant_rejects_tampering_and_expiry() {
        let keyring =
            PageInlineEditKeyring::single(secret("pages-inline-edit-secret-material-00000002"));
        let issued = keyring.issue(context(), 1_000).expect("issued");
        let mut signed: SignedPageInlineEditGrant =
            serde_json::from_str(issued.authorization_proof()).expect("signed");
        signed.payload.push('x');
        let tampered = serde_json::to_string(&signed).expect("tampered");
        assert!(keyring.verify(&tampered, 2_000).is_err());
        assert!(
            keyring
                .verify(issued.authorization_proof(), 61_000)
                .is_err()
        );
    }

    #[test]
    fn grant_rejects_nil_identity() {
        let keyring =
            PageInlineEditKeyring::single(secret("pages-inline-edit-secret-material-00000003"));
        let mut invalid = context();
        invalid.auth_session_id = Uuid::nil();
        assert!(keyring.issue(invalid, 1_000).is_err());
    }

    #[test]
    fn keyring_supports_bounded_rotation() {
        let active = PageInlineEditKeyId::new("2026-08").expect("active id");
        let previous = PageInlineEditKeyId::new("2026-07").expect("previous id");
        let keyring = PageInlineEditKeyring::new(
            active.clone(),
            vec![
                (
                    active.clone(),
                    secret("pages-inline-edit-secret-material-active-0001"),
                ),
                (
                    previous,
                    secret("pages-inline-edit-secret-material-previous-01"),
                ),
            ],
        )
        .expect("keyring");
        assert_eq!(keyring.active_key_id(), &active);
        assert_eq!(keyring.keys.len(), 2);
    }
}
