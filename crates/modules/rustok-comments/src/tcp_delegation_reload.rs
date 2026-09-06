use std::{
    collections::HashMap,
    fmt,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use rustok_api::{PortActor, PortActorKind, PortError, SHA256_DIGEST_BYTES, sha256_digest};
use serde::Deserialize;

use crate::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerAuthorityResolver, CommentsTcpBearerToken,
    CommentsTcpCredential, CommentsTcpDelegatingAuthorityResolver,
    CommentsTcpDelegationConfigError, CommentsTcpDelegationKeyring, CommentsTcpDelegationSigner,
    CommentsTcpOperation, CommentsThreadRequest, DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS,
    DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY, DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS,
    MAX_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY, MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
    TrustedCommentsTcpAuthority,
};

const RELOADABLE_SERVICE_OPERATIONS: [CommentsTcpOperation; 4] = [
    CommentsTcpOperation::GetComment,
    CommentsTcpOperation::ListCommentsForTarget,
    CommentsTcpOperation::ListPublicCommentsForTarget,
    CommentsTcpOperation::SetCommentStatus,
];

/// Supplies one immutable delegation keyring snapshot to a new signing or
/// authorization operation.
///
/// Implementations must return a complete validated keyring. A caller obtains
/// the keyring once at the operation boundary, so an in-flight operation never
/// observes a partial or mixed rotation.
pub trait CommentsTcpDelegationKeyringProvider: Send + Sync {
    fn current_keyring(&self) -> Result<CommentsTcpDelegationKeyring, PortError>;
}

impl CommentsTcpDelegationKeyringProvider for CommentsTcpDelegationKeyring {
    fn current_keyring(&self) -> Result<CommentsTcpDelegationKeyring, PortError> {
        Ok(self.clone())
    }
}

/// Delegation signer that selects the current immutable keyring once for every
/// new user-write operation.
#[derive(Clone)]
pub struct ReloadableCommentsTcpDelegationSigner {
    keyring_provider: Arc<dyn CommentsTcpDelegationKeyringProvider>,
    ttl: Duration,
}

impl ReloadableCommentsTcpDelegationSigner {
    pub fn new(keyring_provider: Arc<dyn CommentsTcpDelegationKeyringProvider>) -> Self {
        Self {
            keyring_provider,
            ttl: Duration::from_millis(DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS),
        }
    }

    pub fn with_ttl(
        keyring_provider: Arc<dyn CommentsTcpDelegationKeyringProvider>,
        ttl: Duration,
    ) -> Result<Self, CommentsTcpDelegationConfigError> {
        let ttl_ms = u64::try_from(ttl.as_millis())
            .map_err(|_| CommentsTcpDelegationConfigError::InvalidTtl)?;
        if ttl_ms == 0 || ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
            return Err(CommentsTcpDelegationConfigError::InvalidTtl);
        }
        Ok(Self {
            keyring_provider,
            ttl,
        })
    }

    pub fn ttl_ms(&self) -> u64 {
        u64::try_from(self.ttl.as_millis()).unwrap_or_default()
    }

    pub fn credential_for(
        &self,
        request: &CommentsThreadRequest,
    ) -> Result<CommentsTcpCredential, PortError> {
        let keyring = self.keyring_provider.current_keyring()?;
        let signer =
            CommentsTcpDelegationSigner::with_keyring_and_ttl(keyring, self.ttl).map_err(|_| {
                PortError::invariant_violation(
                    "comments.tcp_delegation_reload_signer_invalid",
                    "Comments TCP reloadable delegation signer configuration is invalid",
                )
            })?;
        signer.credential_for(request)
    }
}

impl fmt::Debug for ReloadableCommentsTcpDelegationSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReloadableCommentsTcpDelegationSigner")
            .field("keyring_provider", &"[RELOADABLE]")
            .field("ttl_ms", &self.ttl_ms())
            .finish()
    }
}

/// Authority resolver that selects one immutable keyring for every new
/// delegated write while retaining one process-local replay gate across all
/// accepted generations.
#[derive(Clone)]
pub struct ReloadableCommentsTcpDelegatingAuthorityResolver {
    bearer_token: CommentsTcpBearerToken,
    service_actor: PortActor,
    service_authority: CommentsTcpBearerAuthorityResolver,
    keyring_provider: Arc<dyn CommentsTcpDelegationKeyringProvider>,
    max_ttl_ms: u64,
    clock_skew_ms: u64,
    replay: Arc<Mutex<ReloadableDelegationReplayState>>,
}

impl ReloadableCommentsTcpDelegatingAuthorityResolver {
    pub fn new(
        bearer_token: CommentsTcpBearerToken,
        service_actor: PortActor,
        keyring_provider: Arc<dyn CommentsTcpDelegationKeyringProvider>,
    ) -> Self {
        let service_authority = CommentsTcpBearerAuthorityResolver::from_token(
            bearer_token.clone(),
            service_actor.clone(),
        )
        .with_allowed_operations(RELOADABLE_SERVICE_OPERATIONS);
        Self {
            bearer_token,
            service_actor,
            service_authority,
            keyring_provider,
            max_ttl_ms: MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
            clock_skew_ms: DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS,
            replay: Arc::new(Mutex::new(ReloadableDelegationReplayState::new(
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
        let max_ttl_ms = u64::try_from(max_ttl.as_millis())
            .map_err(|_| CommentsTcpDelegationConfigError::InvalidTtl)?;
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
        self.replay = Arc::new(Mutex::new(ReloadableDelegationReplayState::new(capacity)));
        Ok(self)
    }

    async fn authorize_delegated_write(
        &self,
        peer_addr: SocketAddr,
        operation: CommentsTcpOperation,
        credential: Option<&CommentsTcpCredential>,
        request: &CommentsThreadRequest,
    ) -> Result<TrustedCommentsTcpAuthority, PortError> {
        let keyring = self.keyring_provider.current_keyring()?;
        let resolver = CommentsTcpDelegatingAuthorityResolver::with_keyring(
            self.bearer_token.clone(),
            self.service_actor.clone(),
            keyring,
        )
        .with_max_ttl(Duration::from_millis(self.max_ttl_ms))
        .map_err(|_| {
            PortError::invariant_violation(
                "comments.tcp_delegation_reload_resolver_invalid",
                "Comments TCP reloadable delegation resolver configuration is invalid",
            )
        })?;
        let authority = resolver
            .authorize(peer_addr, operation, credential, request)
            .await?;
        self.accept_verified_nonce_once(credential)?;
        Ok(authority)
    }

    fn accept_verified_nonce_once(
        &self,
        credential: Option<&CommentsTcpCredential>,
    ) -> Result<(), PortError> {
        let credential = credential.ok_or_else(reload_delegation_invalid)?;
        let signed = serde_json::from_str::<ReloadableSignedDelegation>(credential.token())
            .map_err(|_| reload_delegation_invalid())?;
        let claims = serde_json::from_str::<ReloadableDelegationClaims>(&signed.payload)
            .map_err(|_| reload_delegation_invalid())?;
        let nonce_digest = sha256_digest(&[claims.nonce.as_bytes()]);
        let now_ms = reload_current_unix_ms()?;
        let expires_at_ms = claims.expires_at_unix_ms.saturating_add(self.clock_skew_ms);
        let mut replay = self.replay.lock().map_err(|_| {
            PortError::unavailable(
                "comments.tcp_delegation_replay_unavailable",
                "Comments TCP delegation replay protection is temporarily unavailable",
            )
        })?;
        replay.entries.retain(|_, expiry| *expiry >= now_ms);
        if replay.entries.contains_key(&nonce_digest) {
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
        replay.entries.insert(nonce_digest, expires_at_ms);
        Ok(())
    }
}

impl fmt::Debug for ReloadableCommentsTcpDelegatingAuthorityResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let capacity = self
            .replay
            .lock()
            .map(|replay| replay.capacity)
            .unwrap_or_default();
        formatter
            .debug_struct("ReloadableCommentsTcpDelegatingAuthorityResolver")
            .field("bearer_token", &"[REDACTED]")
            .field("service_actor", &self.service_actor)
            .field("service_authority", &self.service_authority)
            .field("keyring_provider", &"[RELOADABLE]")
            .field("max_ttl_ms", &self.max_ttl_ms)
            .field("clock_skew_ms", &self.clock_skew_ms)
            .field("replay_capacity", &capacity)
            .finish()
    }
}

#[async_trait]
impl CommentsTcpAuthorityResolver for ReloadableCommentsTcpDelegatingAuthorityResolver {
    async fn authorize(
        &self,
        peer_addr: SocketAddr,
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
            self.authorize_delegated_write(peer_addr, operation, credential, request)
                .await
        }
    }
}

#[derive(Deserialize)]
struct ReloadableSignedDelegation {
    payload: String,
}

#[derive(Deserialize)]
struct ReloadableDelegationClaims {
    expires_at_unix_ms: u64,
    nonce: String,
}

struct ReloadableDelegationReplayState {
    capacity: usize,
    entries: HashMap<[u8; SHA256_DIGEST_BYTES], u64>,
}

impl ReloadableDelegationReplayState {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
        }
    }
}

fn reload_current_unix_ms() -> Result<u64, PortError> {
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

fn reload_delegation_invalid() -> PortError {
    PortError::forbidden(
        "comments.tcp_delegation_invalid",
        "Comments TCP user delegation could not be verified",
    )
}
