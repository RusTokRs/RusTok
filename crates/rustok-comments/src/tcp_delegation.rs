use std::{
    collections::HashMap,
    fmt,
    hash::Hash,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use rustok_api::{
    PortActor, PortActorKind, PortError, SHA256_DIGEST_BYTES, fixed_work_sha256_eq, hmac_sha256,
    sha256_digest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerAuthorityResolver, CommentsTcpBearerToken,
    CommentsTcpCredential, CommentsTcpOperation, CommentsThreadRequest,
    TrustedCommentsTcpAuthority,
};

pub const COMMENTS_TCP_DELEGATION_VERSION: u16 = 1;
pub const DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS: u64 = 5_000;
pub const MAX_COMMENTS_TCP_DELEGATION_TTL_MS: u64 = 30_000;
pub const DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS: u64 = 2_000;
pub const DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY: usize = 4_096;
pub const MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY: usize = 65_536;
pub const MAX_COMMENTS_TCP_DELEGATION_KEYS: usize = 8;
pub const MAX_COMMENTS_TCP_DELEGATION_KEY_ID_BYTES: usize = 64;

const MIN_DELEGATION_SECRET_BYTES: usize = 32;
const MAX_DELEGATION_SECRET_BYTES: usize = 4_096;
const MAX_DELEGATION_TOKEN_BYTES: usize = 32 * 1_024;
const MAX_DELEGATION_PAYLOAD_BYTES: usize = 16 * 1_024;
const LEGACY_DELEGATION_KEY_ID: &str = "legacy";
const DELEGATION_SCHEME: &str = "delegated_hmac_sha256";
const DELEGATION_SIGNATURE_DOMAIN: &[u8] = b"rustok-comments-tcp-user-delegation-v1\0";
const KEY_ID_SEPARATOR: &[u8] = b"\0";
const COMPOSITE_SERVICE_OPERATIONS: [CommentsTcpOperation; 4] = [
    CommentsTcpOperation::GetComment,
    CommentsTcpOperation::ListCommentsForTarget,
    CommentsTcpOperation::ListPublicCommentsForTarget,
    CommentsTcpOperation::SetCommentStatus,
];

#[derive(Clone, Eq, PartialEq)]
pub struct CommentsTcpDelegationSecret {
    secret: String,
}

impl CommentsTcpDelegationSecret {
    pub fn new(secret: impl AsRef<str>) -> Result<Self, CommentsTcpDelegationConfigError> {
        let secret = secret.as_ref();
        if !valid_secret_text(secret) {
            return Err(CommentsTcpDelegationConfigError::InvalidSecret);
        }
        Ok(Self {
            secret: secret.to_string(),
        })
    }

    fn as_bytes(&self) -> &[u8] {
        self.secret.as_bytes()
    }
}

impl fmt::Debug for CommentsTcpDelegationSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationSecret")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct CommentsTcpDelegationKeyId(String);

impl CommentsTcpDelegationKeyId {
    pub fn new(value: impl AsRef<str>) -> Result<Self, CommentsTcpDelegationConfigError> {
        let value = value.as_ref();
        if !valid_key_id(value) {
            return Err(CommentsTcpDelegationConfigError::InvalidKeyId);
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CommentsTcpDelegationKeyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CommentsTcpDelegationKeyId([CONFIGURED])")
    }
}

#[derive(Clone)]
pub struct CommentsTcpDelegationKeyring {
    active_key_id: CommentsTcpDelegationKeyId,
    keys: Arc<HashMap<CommentsTcpDelegationKeyId, CommentsTcpDelegationSecret>>,
    legacy_unkeyed_key_id: Option<CommentsTcpDelegationKeyId>,
}

impl CommentsTcpDelegationKeyring {
    pub fn single(secret: CommentsTcpDelegationSecret) -> Self {
        let key_id = CommentsTcpDelegationKeyId(LEGACY_DELEGATION_KEY_ID.to_string());
        let mut keys = HashMap::new();
        keys.insert(key_id.clone(), secret);
        Self {
            active_key_id: key_id.clone(),
            keys: Arc::new(keys),
            legacy_unkeyed_key_id: Some(key_id),
        }
    }

    pub fn new(
        active_key_id: CommentsTcpDelegationKeyId,
        keys: Vec<(CommentsTcpDelegationKeyId, CommentsTcpDelegationSecret)>,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        if keys.is_empty() || keys.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS {
            return Err(CommentsTcpDelegationConfigError::InvalidKeyCount);
        }
        let mut by_id = HashMap::with_capacity(keys.len());
        for (key_id, secret) in keys {
            if by_id.insert(key_id, secret).is_some() {
                return Err(CommentsTcpDelegationConfigError::DuplicateKeyId);
            }
        }
        if !by_id.contains_key(&active_key_id) {
            return Err(CommentsTcpDelegationConfigError::ActiveKeyMissing);
        }
        Ok(Self {
            active_key_id,
            keys: Arc::new(by_id),
            legacy_unkeyed_key_id: None,
        })
    }

    pub fn with_legacy_unkeyed_key_id(
        mut self,
        key_id: CommentsTcpDelegationKeyId,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        if !self.keys.contains_key(&key_id) {
            return Err(CommentsTcpDelegationConfigError::LegacyKeyMissing);
        }
        self.legacy_unkeyed_key_id = Some(key_id);
        Ok(self)
    }

    pub fn active_key_id(&self) -> &CommentsTcpDelegationKeyId {
        &self.active_key_id
    }

    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    pub fn accepts_legacy_unkeyed_tokens(&self) -> bool {
        self.legacy_unkeyed_key_id.is_some()
    }

    fn active_secret(&self) -> &CommentsTcpDelegationSecret {
        self.keys
            .get(&self.active_key_id)
            .expect("validated delegation keyring must retain its active key")
    }

    fn verification_secret(
        &self,
        key_id: &CommentsTcpDelegationKeyId,
    ) -> Option<&CommentsTcpDelegationSecret> {
        self.keys.get(key_id)
    }

    fn legacy_unkeyed_secret(&self) -> Option<&CommentsTcpDelegationSecret> {
        self.legacy_unkeyed_key_id
            .as_ref()
            .and_then(|key_id| self.keys.get(key_id))
    }
}

impl fmt::Debug for CommentsTcpDelegationKeyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationKeyring")
            .field("active_key_id", &"[CONFIGURED]")
            .field("key_count", &self.keys.len())
            .field(
                "legacy_unkeyed_tokens",
                &self.legacy_unkeyed_key_id.is_some(),
            )
            .field("keys", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum CommentsTcpDelegationConfigError {
    #[error(
        "Comments TCP delegation secret must be 32..=4096 visible non-whitespace ASCII bytes"
    )]
    InvalidSecret,
    #[error("Comments TCP delegation key ID must be 1..=64 ASCII letters, digits, dot, underscore, or hyphen")]
    InvalidKeyId,
    #[error("Comments TCP delegation keyring must contain 1..=8 keys")]
    InvalidKeyCount,
    #[error("Comments TCP delegation key IDs must be unique")]
    DuplicateKeyId,
    #[error("Comments TCP delegation active key ID must exist in the keyring")]
    ActiveKeyMissing,
    #[error("Comments TCP delegation legacy key ID must exist in the keyring")]
    LegacyKeyMissing,
    #[error("Comments TCP delegation TTL must be within 1..=30000 milliseconds")]
    InvalidTtl,
    #[error("Comments TCP delegation replay capacity must be within 1..=65536")]
    InvalidReplayCapacity,
}

#[derive(Clone, Debug)]
pub struct CommentsTcpDelegationSigner {
    keyring: CommentsTcpDelegationKeyring,
    ttl_ms: u64,
}

impl CommentsTcpDelegationSigner {
    pub fn new(secret: CommentsTcpDelegationSecret) -> Self {
        Self::with_keyring(CommentsTcpDelegationKeyring::single(secret))
    }

    pub fn with_keyring(keyring: CommentsTcpDelegationKeyring) -> Self {
        Self {
            keyring,
            ttl_ms: DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS,
        }
    }

    pub fn with_ttl(
        secret: CommentsTcpDelegationSecret,
        ttl: Duration,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        Self::with_keyring_and_ttl(CommentsTcpDelegationKeyring::single(secret), ttl)
    }

    pub fn with_keyring_and_ttl(
        keyring: CommentsTcpDelegationKeyring,
        ttl: Duration,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        let ttl_ms = duration_ms(ttl).ok_or(CommentsTcpDelegationConfigError::InvalidTtl)?;
        if ttl_ms == 0 || ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
            return Err(CommentsTcpDelegationConfigError::InvalidTtl);
        }
        Ok(Self { keyring, ttl_ms })
    }

    pub fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }

    pub fn active_key_id(&self) -> &CommentsTcpDelegationKeyId {
        self.keyring.active_key_id()
    }

    pub fn credential_for(
        &self,
        request: &CommentsThreadRequest,
    ) -> Result<CommentsTcpCredential, PortError> {
        self.credential_for_at(request, current_unix_ms()?)
    }

    fn credential_for_at(
        &self,
        request: &CommentsThreadRequest,
        now_ms: u64,
    ) -> Result<CommentsTcpCredential, PortError> {
        let operation = CommentsTcpOperation::for_request(request);
        if !operation.is_write() || operation == CommentsTcpOperation::SetCommentStatus {
            return Err(PortError::validation(
                "comments.tcp_delegation_user_write_required",
                "Comments TCP user delegation is issued for user-owned write operations",
            ));
        }

        let context = request.context();
        context.require_write_semantics()?;
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        if context.actor.kind != PortActorKind::User
            || !is_canonical_uuid(&context.tenant_id)
            || !is_canonical_uuid(&context.actor.id)
            || context.claims.is_empty()
            || context.roles.len() != 1
            || context.correlation_id.is_empty()
            || idempotency_key.is_empty()
        {
            return Err(delegation_context_invalid());
        }

        let request_digest = request_digest(request)?;
        let expires_at_unix_ms = now_ms
            .checked_add(self.ttl_ms)
            .ok_or_else(delegation_context_invalid)?;
        let claims = CommentsTcpDelegationClaims {
            version: COMMENTS_TCP_DELEGATION_VERSION,
            tenant_id: context.tenant_id.clone(),
            actor_id: context.actor.id.clone(),
            claims: context.claims.clone(),
            roles: context.roles.clone(),
            operation: operation.as_str().to_string(),
            correlation_id: context.correlation_id.clone(),
            idempotency_key: idempotency_key.to_string(),
            issued_at_unix_ms: now_ms,
            expires_at_unix_ms,
            nonce: Uuid::new_v4().to_string(),
            request_digest,
        };
        let payload = serde_json::to_string(&claims).map_err(|_| {
            PortError::invariant_violation(
                "comments.tcp_delegation_encode",
                "Comments TCP delegation claims could not be encoded",
            )
        })?;
        if payload.len() > MAX_DELEGATION_PAYLOAD_BYTES {
            return Err(delegation_context_invalid());
        }
        let key_id = self.keyring.active_key_id().as_str().to_string();
        let signature = keyed_delegation_signature(
            self.keyring.active_secret(),
            &key_id,
            payload.as_bytes(),
        );
        let token = serde_json::to_string(&SignedCommentsTcpDelegation {
            key_id: Some(key_id),
            payload,
            signature,
        })
        .map_err(|_| {
            PortError::invariant_violation(
                "comments.tcp_delegation_encode",
                "Comments TCP signed delegation could not be encoded",
            )
        })?;
        if token.len() > MAX_DELEGATION_TOKEN_BYTES {
            return Err(delegation_context_invalid());
        }
        Ok(CommentsTcpCredential::delegated(token))
    }
}

#[derive(Clone)]
pub struct CommentsTcpDelegatingAuthorityResolver {
    service_authority: CommentsTcpBearerAuthorityResolver,
    keyring: CommentsTcpDelegationKeyring,
    max_ttl_ms: u64,
    clock_skew_ms: u64,
    replay: Arc<Mutex<DelegationReplayState>>,
}

impl CommentsTcpDelegatingAuthorityResolver {
    pub fn new(
        bearer_token: CommentsTcpBearerToken,
        service_actor: PortActor,
        delegation_secret: CommentsTcpDelegationSecret,
    ) -> Self {
        Self::with_keyring(
            bearer_token,
            service_actor,
            CommentsTcpDelegationKeyring::single(delegation_secret),
        )
    }

    pub fn with_keyring(
        bearer_token: CommentsTcpBearerToken,
        service_actor: PortActor,
        keyring: CommentsTcpDelegationKeyring,
    ) -> Self {
        Self {
            service_authority: CommentsTcpBearerAuthorityResolver::from_token(
                bearer_token,
                service_actor,
            )
            .with_allowed_operations(COMPOSITE_SERVICE_OPERATIONS),
            keyring,
            max_ttl_ms: MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
            clock_skew_ms: DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS,
            replay: Arc::new(Mutex::new(DelegationReplayState::new(
                DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
            ))),
        }
    }

    pub fn with_service_claim(mut self, claim: impl Into<String>) -> Self {
        self.service_authority = self.service_authority.with_claim(claim);
        self
    }

    pub fn with_service_role(mut self, role: impl Into<String>) -> Self {
        self.service_authority = self.service_authority.with_role(role);
        self
    }

    pub fn with_max_ttl(
        mut self,
        max_ttl: Duration,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        let max_ttl_ms = duration_ms(max_ttl)
            .ok_or(CommentsTcpDelegationConfigError::InvalidTtl)?;
        if max_ttl_ms == 0 || max_ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
            return Err(CommentsTcpDelegationConfigError::InvalidTtl);
        }
        self.max_ttl_ms = max_ttl_ms;
        Ok(self)
    }

    pub fn with_replay_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        if capacity == 0 || capacity > MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY {
            return Err(CommentsTcpDelegationConfigError::InvalidReplayCapacity);
        }
        self.replay = Arc::new(Mutex::new(DelegationReplayState::new(capacity)));
        Ok(self)
    }

    fn authorize_delegated_write_at(
        &self,
        peer_addr: std::net::SocketAddr,
        operation: CommentsTcpOperation,
        credential: Option<&CommentsTcpCredential>,
        request: &CommentsThreadRequest,
        now_ms: u64,
    ) -> Result<TrustedCommentsTcpAuthority, PortError> {
        if !peer_addr.ip().is_loopback()
            || CommentsTcpOperation::for_request(request) != operation
            || operation == CommentsTcpOperation::SetCommentStatus
        {
            return Err(delegation_invalid());
        }
        let credential = credential
            .filter(|value| value.scheme() == DELEGATION_SCHEME)
            .ok_or_else(delegation_invalid)?;
        let token = credential.token();
        if token.len() > MAX_DELEGATION_TOKEN_BYTES {
            return Err(delegation_invalid());
        }
        let signed = serde_json::from_str::<SignedCommentsTcpDelegation>(token)
            .map_err(|_| delegation_invalid())?;
        if signed.payload.len() > MAX_DELEGATION_PAYLOAD_BYTES {
            return Err(delegation_invalid());
        }
        let expected = match signed.key_id.as_deref() {
            Some(raw_key_id) => {
                let key_id = CommentsTcpDelegationKeyId::new(raw_key_id)
                    .map_err(|_| delegation_invalid())?;
                let secret = self
                    .keyring
                    .verification_secret(&key_id)
                    .ok_or_else(delegation_invalid)?;
                keyed_delegation_signature(secret, raw_key_id, signed.payload.as_bytes())
            }
            None => {
                let secret = self
                    .keyring
                    .legacy_unkeyed_secret()
                    .ok_or_else(delegation_invalid)?;
                legacy_delegation_signature(secret, signed.payload.as_bytes())
            }
        };
        if !fixed_work_sha256_eq(&expected, &signed.signature) {
            return Err(delegation_invalid());
        }
        let claims = serde_json::from_str::<CommentsTcpDelegationClaims>(&signed.payload)
            .map_err(|_| delegation_invalid())?;
        let context = request.context();
        let idempotency_key = context.idempotency_key.as_deref().unwrap_or_default();
        let digest = request_digest(request).map_err(|_| delegation_invalid())?;
        let latest_issued_at = now_ms.saturating_add(self.clock_skew_ms);
        let ttl_ms = claims
            .expires_at_unix_ms
            .checked_sub(claims.issued_at_unix_ms)
            .ok_or_else(delegation_invalid)?;

        if claims.version != COMMENTS_TCP_DELEGATION_VERSION
            || claims.operation != operation.as_str()
            || !operation.is_write()
            || context.actor.kind != PortActorKind::User
            || !is_canonical_uuid(&claims.tenant_id)
            || !is_canonical_uuid(&claims.actor_id)
            || !is_canonical_uuid(&claims.nonce)
            || claims.tenant_id != context.tenant_id
            || claims.actor_id != context.actor.id
            || claims.claims != context.claims
            || claims.roles != context.roles
            || claims.correlation_id != context.correlation_id
            || claims.idempotency_key != idempotency_key
            || claims.request_digest != digest
            || claims.claims.is_empty()
            || claims.roles.len() != 1
            || ttl_ms == 0
            || ttl_ms > self.max_ttl_ms
            || claims.issued_at_unix_ms > latest_issued_at
            || claims.expires_at_unix_ms < now_ms
        {
            return Err(delegation_invalid());
        }

        self.accept_nonce(&claims.nonce, claims.expires_at_unix_ms, now_ms)?;
        Ok(TrustedCommentsTcpAuthority {
            tenant_id: claims.tenant_id,
            actor: PortActor::user(claims.actor_id),
            claims: claims.claims,
            roles: claims.roles,
        })
    }

    fn accept_nonce(
        &self,
        nonce: &str,
        expires_at_unix_ms: u64,
        now_ms: u64,
    ) -> Result<(), PortError> {
        let mut replay = self.replay.lock().map_err(|_| {
            PortError::unavailable(
                "comments.tcp_delegation_replay_unavailable",
                "Comments TCP delegation replay protection is temporarily unavailable",
            )
        })?;
        replay.entries.retain(|_, expiry| *expiry >= now_ms);
        if replay.entries.contains_key(nonce) {
            return Err(PortError::forbidden(
                "comments.tcp_delegation_replayed",
                "Comments TCP user delegation has already been used by this listener process",
            ));
        }
        if replay.entries.len() >= replay.capacity {
            return Err(PortError::unavailable(
                "comments.tcp_delegation_replay_unavailable",
                "Comments TCP delegation replay protection is temporarily unavailable",
            ));
        }
        replay
            .entries
            .insert(nonce.to_string(), expires_at_unix_ms);
        Ok(())
    }
}

impl fmt::Debug for CommentsTcpDelegatingAuthorityResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let capacity = self
            .replay
            .lock()
            .map(|replay| replay.capacity)
            .unwrap_or_default();
        formatter
            .debug_struct("CommentsTcpDelegatingAuthorityResolver")
            .field("service_authority", &self.service_authority)
            .field("keyring", &self.keyring)
            .field("max_ttl_ms", &self.max_ttl_ms)
            .field("clock_skew_ms", &self.clock_skew_ms)
            .field("replay_capacity", &capacity)
            .finish()
    }
}

#[async_trait]
impl CommentsTcpAuthorityResolver for CommentsTcpDelegatingAuthorityResolver {
    async fn authorize(
        &self,
        peer_addr: std::net::SocketAddr,
        operation: CommentsTcpOperation,
        credential: Option<&CommentsTcpCredential>,
        request: &CommentsThreadRequest,
    ) -> Result<TrustedCommentsTcpAuthority, PortError> {
        let service_moderation = operation == CommentsTcpOperation::SetCommentStatus
            && request.context().actor.kind == PortActorKind::System;
        if !operation.is_write() || service_moderation {
            self.service_authority
                .authorize(peer_addr, operation, credential, request)
                .await
        } else {
            self.authorize_delegated_write_at(
                peer_addr,
                operation,
                credential,
                request,
                current_unix_ms()?,
            )
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CommentsTcpDelegationClaims {
    version: u16,
    tenant_id: String,
    actor_id: String,
    claims: Vec<String>,
    roles: Vec<String>,
    operation: String,
    correlation_id: String,
    idempotency_key: String,
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    nonce: String,
    request_digest: [u8; SHA256_DIGEST_BYTES],
}

#[derive(Clone, Serialize, Deserialize)]
struct SignedCommentsTcpDelegation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key_id: Option<String>,
    payload: String,
    signature: [u8; SHA256_DIGEST_BYTES],
}

impl fmt::Debug for SignedCommentsTcpDelegation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignedCommentsTcpDelegation")
            .field("key_id", &self.key_id.as_ref().map(|_| "[CONFIGURED]"))
            .field("payload_bytes", &self.payload.len())
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

struct DelegationReplayState {
    capacity: usize,
    entries: HashMap<String, u64>,
}

impl DelegationReplayState {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
        }
    }
}

fn request_digest(
    request: &CommentsThreadRequest,
) -> Result<[u8; SHA256_DIGEST_BYTES], PortError> {
    let payload = serde_json::to_vec(request).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_delegation_request_encode",
            "Comments TCP request could not be bound to user delegation",
        )
    })?;
    Ok(sha256_digest(&[payload.as_slice()]))
}

fn keyed_delegation_signature(
    secret: &CommentsTcpDelegationSecret,
    key_id: &str,
    payload: &[u8],
) -> [u8; SHA256_DIGEST_BYTES] {
    hmac_sha256(
        secret.as_bytes(),
        &[
            DELEGATION_SIGNATURE_DOMAIN,
            key_id.as_bytes(),
            KEY_ID_SEPARATOR,
            payload,
        ],
    )
}

fn legacy_delegation_signature(
    secret: &CommentsTcpDelegationSecret,
    payload: &[u8],
) -> [u8; SHA256_DIGEST_BYTES] {
    hmac_sha256(secret.as_bytes(), &[DELEGATION_SIGNATURE_DOMAIN, payload])
}

fn valid_secret_text(secret: &str) -> bool {
    secret.len() >= MIN_DELEGATION_SECRET_BYTES
        && secret.len() <= MAX_DELEGATION_SECRET_BYTES
        && secret.is_ascii()
        && !secret
            .as_bytes()
            .iter()
            .any(|byte| *byte <= b' ' || *byte == 0x7f)
}

fn valid_key_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_COMMENTS_TCP_DELEGATION_KEY_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn duration_ms(duration: Duration) -> Option<u64> {
    u64::try_from(duration.as_millis()).ok()
}

fn current_unix_ms() -> Result<u64, PortError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_delegation_clock_invalid",
            "Comments TCP delegation clock is not available",
        )
    })?;
    u64::try_from(duration.as_millis()).map_err(|_| {
        PortError::invariant_violation(
            "comments.tcp_delegation_clock_invalid",
            "Comments TCP delegation clock is not available",
        )
    })
}

fn delegation_context_invalid() -> PortError {
    PortError::forbidden(
        "comments.tcp_delegation_context_invalid",
        "Comments TCP write requires trusted user delegation context",
    )
}

fn delegation_invalid() -> PortError {
    PortError::forbidden(
        "comments.tcp_delegation_invalid",
        "Comments TCP user delegation could not be verified",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortContext;

    fn secret(value: &str) -> CommentsTcpDelegationSecret {
        CommentsTcpDelegationSecret::new(value).unwrap()
    }

    fn key_id(value: &str) -> CommentsTcpDelegationKeyId {
        CommentsTcpDelegationKeyId::new(value).unwrap()
    }

    fn write_request() -> CommentsThreadRequest {
        CommentsThreadRequest::DeleteComment {
            context: PortContext::new(
                Uuid::new_v4().to_string(),
                PortActor::user(Uuid::new_v4().to_string()),
                "en",
                "corr-delegation",
            )
            .with_claim("comments:delete")
            .with_role("customer")
            .with_idempotency_key("idem-delegation")
            .with_deadline(Duration::from_secs(2)),
            comment_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn delegation_secret_and_keyring_debug_are_redacted() {
        let keyring = CommentsTcpDelegationKeyring::new(
            key_id("active-2026-08"),
            vec![(
                key_id("active-2026-08"),
                secret("0123456789abcdef0123456789abcdef"),
            )],
        )
        .unwrap();
        let debug = format!("{keyring:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("0123456789abcdef"));
        assert!(!debug.contains("active-2026-08"));
    }

    #[test]
    fn keyring_requires_unique_bounded_keys_and_present_active_key() {
        let first = secret("0123456789abcdef0123456789abcdef");
        let second = secret("abcdef0123456789abcdef0123456789");
        assert_eq!(
            CommentsTcpDelegationKeyring::new(
                key_id("missing"),
                vec![(key_id("one"), first.clone())],
            )
            .unwrap_err(),
            CommentsTcpDelegationConfigError::ActiveKeyMissing
        );
        assert_eq!(
            CommentsTcpDelegationKeyring::new(
                key_id("one"),
                vec![(key_id("one"), first), (key_id("one"), second)],
            )
            .unwrap_err(),
            CommentsTcpDelegationConfigError::DuplicateKeyId
        );
    }

    #[test]
    fn overlapping_keyring_accepts_old_and_new_keyed_tokens() {
        let old_id = key_id("old-2026-07");
        let new_id = key_id("new-2026-08");
        let old_secret = secret("0123456789abcdef0123456789abcdef");
        let new_secret = secret("abcdef0123456789abcdef0123456789");
        let old_ring = CommentsTcpDelegationKeyring::new(
            old_id.clone(),
            vec![
                (old_id.clone(), old_secret.clone()),
                (new_id.clone(), new_secret.clone()),
            ],
        )
        .unwrap();
        let new_ring = CommentsTcpDelegationKeyring::new(
            new_id.clone(),
            vec![(old_id, old_secret), (new_id, new_secret)],
        )
        .unwrap();
        let request = write_request();
        let old_credential = CommentsTcpDelegationSigner::with_keyring(old_ring)
            .credential_for_at(&request, 10_000)
            .unwrap();
        let resolver = CommentsTcpDelegatingAuthorityResolver::with_keyring(
            CommentsTcpBearerToken::new("comments-read-secret").unwrap(),
            PortActor::service(Uuid::new_v4().to_string()),
            new_ring,
        );
        resolver
            .authorize_delegated_write_at(
                "127.0.0.1:9000".parse().unwrap(),
                CommentsTcpOperation::DeleteComment,
                Some(&old_credential),
                &request,
                10_001,
            )
            .unwrap();
    }

    #[test]
    fn revoked_or_unknown_key_id_fails_with_generic_invalid_code() {
        let old_id = key_id("old");
        let old_secret = secret("0123456789abcdef0123456789abcdef");
        let request = write_request();
        let credential = CommentsTcpDelegationSigner::with_keyring(
            CommentsTcpDelegationKeyring::new(
                old_id.clone(),
                vec![(old_id, old_secret)],
            )
            .unwrap(),
        )
        .credential_for_at(&request, 20_000)
        .unwrap();
        let current_id = key_id("current");
        let resolver = CommentsTcpDelegatingAuthorityResolver::with_keyring(
            CommentsTcpBearerToken::new("comments-read-secret").unwrap(),
            PortActor::service(Uuid::new_v4().to_string()),
            CommentsTcpDelegationKeyring::new(
                current_id.clone(),
                vec![(
                    current_id,
                    secret("abcdef0123456789abcdef0123456789"),
                )],
            )
            .unwrap(),
        );
        let error = resolver
            .authorize_delegated_write_at(
                "127.0.0.1:9000".parse().unwrap(),
                CommentsTcpOperation::DeleteComment,
                Some(&credential),
                &request,
                20_001,
            )
            .unwrap_err();
        assert_eq!(error.code, "comments.tcp_delegation_invalid");
    }

    #[test]
    fn legacy_unkeyed_token_can_be_retained_during_rolling_upgrade() {
        let legacy_secret = secret("0123456789abcdef0123456789abcdef");
        let request = write_request();
        let payload = match &request {
            CommentsThreadRequest::DeleteComment { context, .. } => {
                let claims = CommentsTcpDelegationClaims {
                    version: COMMENTS_TCP_DELEGATION_VERSION,
                    tenant_id: context.tenant_id.clone(),
                    actor_id: context.actor.id.clone(),
                    claims: context.claims.clone(),
                    roles: context.roles.clone(),
                    operation: CommentsTcpOperation::DeleteComment.as_str().to_string(),
                    correlation_id: context.correlation_id.clone(),
                    idempotency_key: context.idempotency_key.clone().unwrap(),
                    issued_at_unix_ms: 30_000,
                    expires_at_unix_ms: 35_000,
                    nonce: Uuid::new_v4().to_string(),
                    request_digest: request_digest(&request).unwrap(),
                };
                serde_json::to_string(&claims).unwrap()
            }
            _ => unreachable!(),
        };
        let signature = legacy_delegation_signature(&legacy_secret, payload.as_bytes());
        let token = serde_json::to_string(&SignedCommentsTcpDelegation {
            key_id: None,
            payload,
            signature,
        })
        .unwrap();
        let credential = CommentsTcpCredential::delegated(token);
        let resolver = CommentsTcpDelegatingAuthorityResolver::new(
            CommentsTcpBearerToken::new("comments-read-secret").unwrap(),
            PortActor::service(Uuid::new_v4().to_string()),
            legacy_secret,
        );
        resolver
            .authorize_delegated_write_at(
                "127.0.0.1:9000".parse().unwrap(),
                CommentsTcpOperation::DeleteComment,
                Some(&credential),
                &request,
                30_001,
            )
            .unwrap();
    }

    #[test]
    fn replay_capacity_is_hard_bounded() {
        let resolver = CommentsTcpDelegatingAuthorityResolver::new(
            CommentsTcpBearerToken::new("comments-read-secret").unwrap(),
            PortActor::service(Uuid::new_v4().to_string()),
            secret("0123456789abcdef0123456789abcdef"),
        );
        assert!(
            resolver
                .with_replay_capacity(MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY + 1)
                .is_err()
        );
    }

    #[test]
    fn delegation_binds_request_and_rejects_process_local_replay() {
        let shared_secret = secret("0123456789abcdef0123456789abcdef");
        let signer = CommentsTcpDelegationSigner::new(shared_secret.clone());
        let request = write_request();
        let credential = signer.credential_for_at(&request, 40_000).unwrap();
        let resolver = CommentsTcpDelegatingAuthorityResolver::new(
            CommentsTcpBearerToken::new("comments-read-secret").unwrap(),
            PortActor::service(Uuid::new_v4().to_string()),
            shared_secret,
        );
        let peer = "127.0.0.1:9000".parse().unwrap();
        resolver
            .authorize_delegated_write_at(
                peer,
                CommentsTcpOperation::DeleteComment,
                Some(&credential),
                &request,
                40_001,
            )
            .unwrap();
        let replay = resolver
            .authorize_delegated_write_at(
                peer,
                CommentsTcpOperation::DeleteComment,
                Some(&credential),
                &request,
                40_002,
            )
            .unwrap_err();
        assert_eq!(replay.code, "comments.tcp_delegation_replayed");
    }
}
