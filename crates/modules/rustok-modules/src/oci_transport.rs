//! Strict HTTP transport for the OCI Distribution API.
//!
//! `oci-distribution` owns the OCI data-model types used by the control plane,
//! but its HTTP client does not expose the egress controls required by the
//! platform. This adapter owns the small Distribution API subset used for
//! immutable module publication and admission.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use futures_util::{StreamExt, future};
use oci_distribution::{
    manifest::{OCI_IMAGE_MEDIA_TYPE, OciDescriptor, OciImageManifest},
    secrets::RegistryAuth,
};
use reqwest::{
    Client, Method, StatusCode, Url,
    header::{ACCEPT, AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
    redirect::Policy as RedirectPolicy,
};
use serde::Deserialize;
use tokio::sync::{OwnedSemaphorePermit, RwLock, Semaphore};

use crate::oci::OciRegistryTransportPolicy;

const OCI_MANIFEST_ACCEPT: &str = "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
const OCI_BLOB_MEDIA_TYPE: &str = "application/octet-stream";
const TOKEN_RESPONSE_MAXIMUM_BYTES: u64 = 1024 * 1024;
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(100);

/// A registry/repository/reference tuple safe to place in an OCI Distribution
/// API path. It stays crate-private so callers cannot bypass owner validation.
#[derive(Clone, Debug)]
pub(crate) struct RegistryReference {
    registry: String,
    repository: String,
    reference: String,
}

impl RegistryReference {
    pub(crate) fn new(
        registry: impl Into<String>,
        repository: impl Into<String>,
        reference: impl Into<String>,
    ) -> Result<Self, String> {
        let reference = Self {
            registry: registry.into(),
            repository: repository.into(),
            reference: reference.into(),
        };
        reference.base_url()?;
        if reference.repository.is_empty()
            || reference.repository.starts_with('/')
            || reference.repository.ends_with('/')
            || reference
                .repository
                .split('/')
                .any(|segment| segment.is_empty() || !valid_path_segment(segment))
            || !valid_reference(&reference.reference)
        {
            return Err(
                "OCI registry reference contains an invalid repository or reference".to_string(),
            );
        }
        Ok(reference)
    }

    #[cfg(test)]
    pub(crate) fn canonical(&self) -> String {
        format!("{}/{}@{}", self.registry, self.repository, self.reference)
    }

    fn base_url(&self) -> Result<Url, String> {
        let url = Url::parse(&format!("https://{}/", self.registry))
            .map_err(|_| "OCI registry host is invalid".to_string())?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("OCI registry host is invalid".to_string());
        }
        Ok(url)
    }

    fn manifest_url(&self) -> Result<Url, String> {
        self.api_url(&format!("manifests/{}", self.reference))
    }

    fn blob_url(&self, digest: &str) -> Result<Url, String> {
        if !valid_digest(digest) {
            return Err("OCI blob digest is invalid".to_string());
        }
        self.api_url(&format!("blobs/{digest}"))
    }

    fn upload_url(&self) -> Result<Url, String> {
        self.api_url("blobs/uploads/")
    }

    fn api_url(&self, suffix: &str) -> Result<Url, String> {
        let mut url = self.base_url()?;
        url.set_path(&format!("/v2/{}/{suffix}", self.repository));
        Ok(url)
    }

    fn upload_location(&self, location: &str) -> Result<Url, String> {
        let base = self.base_url()?;
        let url = match Url::parse(location) {
            Ok(url) => url,
            Err(url::ParseError::RelativeUrlWithoutBase) => base
                .join(location)
                .map_err(|_| "OCI upload location is invalid".to_string())?,
            Err(_) => return Err("OCI upload location is invalid".to_string()),
        };
        if url.scheme() != "https"
            || !same_origin(&base, &url)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !url.path().starts_with("/v2/")
        {
            return Err("OCI registry returned an upload location outside its origin".to_string());
        }
        Ok(url)
    }
}

/// A digest-verified blob supplied by the publication owner.
#[derive(Clone, Copy)]
pub(crate) struct Blob<'a> {
    pub(crate) media_type: &'a str,
    pub(crate) digest: &'a str,
    pub(crate) bytes: &'a [u8],
}

#[derive(Clone)]
pub(crate) struct OciRegistryTransport {
    client: Client,
    policy: OciRegistryTransportPolicy,
    permits: Arc<Semaphore>,
    tokens: Arc<RwLock<HashMap<TokenKey, CachedToken>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TokenKey {
    registry: String,
    repository: String,
    operation: Operation,
}

#[derive(Clone)]
struct CachedToken {
    value: String,
    expires_at: Instant,
}

struct BearerToken {
    value: String,
    lifetime: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Operation {
    Pull,
    Push,
}

impl Operation {
    fn scope(self, reference: &RegistryReference) -> String {
        let actions = match self {
            Self::Pull => "pull",
            Self::Push => "pull,push",
        };
        format!("repository:{}:{actions}", reference.repository)
    }
}

enum Authorization {
    None,
    Basic(String, String),
    Bearer(String),
}

struct OciRequest<'a> {
    reference: &'a RegistryReference,
    method: Method,
    url: Url,
    auth: &'a RegistryAuth,
    operation: Operation,
    content_type: Option<&'a str>,
    accept: Option<&'a str>,
    body: Option<Bytes>,
    retryable: bool,
}

struct OciSendRequest<'a> {
    method: Method,
    url: Url,
    authorization: Authorization,
    content_type: Option<&'a str>,
    accept: Option<&'a str>,
    body: Option<Bytes>,
    retryable: bool,
}

impl Authorization {
    fn from_registry_auth(auth: &RegistryAuth) -> Self {
        match auth {
            RegistryAuth::Anonymous => Self::None,
            RegistryAuth::Basic(username, password) => {
                Self::Basic(username.clone(), password.clone())
            }
        }
    }
}

struct OciResponse {
    response: reqwest::Response,
    _permit: OwnedSemaphorePermit,
}

#[derive(Deserialize)]
struct TokenResponse {
    token: Option<String>,
    access_token: Option<String>,
    expires_in: Option<u64>,
}

struct BearerChallenge {
    realm: Url,
    service: Option<String>,
}

impl OciRegistryTransport {
    pub(crate) fn with_policy(policy: OciRegistryTransportPolicy) -> Result<Self, String> {
        policy.validate()?;
        let timeout = Duration::from_millis(policy.request_timeout_ms);
        let client = Client::builder()
            .https_only(true)
            .redirect(RedirectPolicy::none())
            .no_proxy()
            .timeout(timeout)
            .connect_timeout(timeout)
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| "strict OCI registry transport could not be constructed".to_string())?;
        Ok(Self {
            client,
            permits: Arc::new(Semaphore::new(policy.max_concurrent_requests)),
            policy,
            tokens: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub(crate) async fn pull_manifest(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
    ) -> Result<(Vec<u8>, String), String> {
        let response = self
            .request(OciRequest {
                reference,
                method: Method::GET,
                url: reference.manifest_url()?,
                auth,
                operation: Operation::Pull,
                content_type: None,
                accept: Some(OCI_MANIFEST_ACCEPT),
                body: None,
                retryable: true,
            })
            .await?;
        let response = expect_status(response, &[StatusCode::OK], "read an OCI manifest")?;
        let declared_digest = response
            .response
            .headers()
            .get("docker-content-digest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = self.read_bytes(response, self.response_limit()).await?;
        let actual_digest = digest_bytes(&body);
        if let Some(declared_digest) = declared_digest
            && declared_digest != actual_digest
        {
            return Err(
                "OCI registry returned a manifest with a mismatched digest header".to_string(),
            );
        }
        Ok((body, actual_digest))
    }

    pub(crate) async fn pull_image_manifest(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
    ) -> Result<(OciImageManifest, String), String> {
        let (body, digest) = self.pull_manifest(reference, auth).await?;
        let manifest: OciImageManifest = serde_json::from_slice(&body)
            .map_err(|_| "OCI registry returned an invalid image manifest".to_string())?;
        if manifest.schema_version != 2
            || manifest.media_type.as_deref() != Some(OCI_IMAGE_MEDIA_TYPE)
        {
            return Err("OCI registry returned an unsupported image manifest".to_string());
        }
        Ok((manifest, digest))
    }

    pub(crate) async fn pull_blob_stream(
        &self,
        reference: &RegistryReference,
        digest: &str,
        auth: &RegistryAuth,
    ) -> Result<futures_util::stream::BoxStream<'static, Result<Bytes, String>>, String> {
        let response = self
            .request(OciRequest {
                reference,
                method: Method::GET,
                url: reference.blob_url(digest)?,
                auth,
                operation: Operation::Pull,
                content_type: None,
                accept: Some(OCI_BLOB_MEDIA_TYPE),
                body: None,
                retryable: true,
            })
            .await?;
        let response = expect_status(response, &[StatusCode::OK], "read an OCI blob")?;
        self.ensure_transfer_headers(&response, self.response_limit())?;
        let OciResponse { response, _permit } = response;
        let maximum = self.response_limit();
        Ok(response
            .bytes_stream()
            .scan((_permit, 0_u64), move |state, next| {
                future::ready(Some(match next {
                    Ok(chunk) => {
                        state.1 = state.1.saturating_add(chunk.len() as u64);
                        if state.1 > maximum {
                            Err("OCI registry response exceeds the transfer limit".to_string())
                        } else {
                            Ok(chunk)
                        }
                    }
                    Err(_) => Err("OCI registry blob transfer failed".to_string()),
                }))
            })
            .boxed())
    }

    pub(crate) async fn push_artifact(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
        config: Blob<'_>,
        layers: &[Blob<'_>],
        artifact_type: &str,
    ) -> Result<(), String> {
        validate_blob(config)?;
        if layers.is_empty() || !valid_media_type(artifact_type) {
            return Err("OCI artifact manifest is invalid".to_string());
        }
        for layer in layers {
            validate_blob(*layer)?;
        }
        self.push_blob(reference, auth, config.bytes, config.digest)
            .await?;
        for layer in layers {
            self.push_blob(reference, auth, layer.bytes, layer.digest)
                .await?;
        }
        let manifest = OciImageManifest {
            schema_version: 2,
            media_type: Some(OCI_IMAGE_MEDIA_TYPE.to_string()),
            config: descriptor(config)?,
            layers: layers
                .iter()
                .copied()
                .map(descriptor)
                .collect::<Result<Vec<_>, _>>()?,
            artifact_type: Some(artifact_type.to_string()),
            annotations: None,
        };
        let body = serde_json::to_vec(&manifest)
            .map_err(|_| "OCI artifact manifest could not be encoded".to_string())?;
        self.push_manifest(reference, auth, body, OCI_IMAGE_MEDIA_TYPE)
            .await
    }

    pub(crate) async fn push_blob(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
        bytes: &[u8],
        digest: &str,
    ) -> Result<(), String> {
        validate_blob(Blob {
            media_type: OCI_BLOB_MEDIA_TYPE,
            digest,
            bytes,
        })?;
        if bytes.len() as u64 > self.policy.max_transfer_bytes {
            return Err("OCI blob publication exceeds the transfer limit".to_string());
        }
        let response = self
            .request(OciRequest {
                reference,
                method: Method::HEAD,
                url: reference.blob_url(digest)?,
                auth,
                operation: Operation::Push,
                content_type: None,
                accept: None,
                body: None,
                retryable: true,
            })
            .await?;
        match response.response.status() {
            StatusCode::OK => return Ok(()),
            StatusCode::NOT_FOUND => {}
            _ => return Err(status_error(response, "check an OCI blob")),
        }
        let response = self
            .request(OciRequest {
                reference,
                method: Method::POST,
                url: reference.upload_url()?,
                auth,
                operation: Operation::Push,
                content_type: None,
                accept: None,
                body: Some(Bytes::new()),
                retryable: false,
            })
            .await?;
        let response = expect_status(
            response,
            &[StatusCode::ACCEPTED],
            "begin an OCI blob upload",
        )?;
        let location = response
            .response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "OCI registry did not return an upload location".to_string())?;
        let upload_url = reference.upload_location(location)?;
        drop(response);

        let mut upload_url = upload_url;
        upload_url.query_pairs_mut().append_pair("digest", digest);
        let response = self
            .request(OciRequest {
                reference,
                method: Method::PUT,
                url: upload_url,
                auth,
                operation: Operation::Push,
                content_type: Some(OCI_BLOB_MEDIA_TYPE),
                accept: None,
                body: Some(Bytes::copy_from_slice(bytes)),
                retryable: true,
            })
            .await?;
        expect_status(
            response,
            &[StatusCode::CREATED],
            "complete an OCI blob upload",
        )?;
        Ok(())
    }

    pub(crate) async fn push_manifest(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
        body: Vec<u8>,
        media_type: &str,
    ) -> Result<(), String> {
        if body.is_empty()
            || body.len() as u64 > self.policy.max_transfer_bytes
            || !valid_media_type(media_type)
        {
            return Err("OCI manifest publication input is invalid".to_string());
        }
        let response = self
            .request(OciRequest {
                reference,
                method: Method::PUT,
                url: reference.manifest_url()?,
                auth,
                operation: Operation::Push,
                content_type: Some(media_type),
                accept: None,
                body: Some(Bytes::from(body)),
                retryable: true,
            })
            .await?;
        expect_status(response, &[StatusCode::CREATED], "publish an OCI manifest")?;
        Ok(())
    }

    async fn request(&self, request: OciRequest<'_>) -> Result<OciResponse, String> {
        let OciRequest {
            reference,
            method,
            url,
            auth,
            operation,
            content_type,
            accept,
            body,
            retryable,
        } = request;
        let token_key = TokenKey {
            registry: reference.registry.clone(),
            repository: reference.repository.clone(),
            operation,
        };
        let authorization = self
            .cached_token(&token_key)
            .await
            .map(Authorization::Bearer)
            .unwrap_or_else(|| Authorization::from_registry_auth(auth));
        let response = self
            .send_with_retries(OciSendRequest {
                method: method.clone(),
                url: url.clone(),
                authorization,
                content_type,
                accept,
                body: body.clone(),
                retryable,
            })
            .await?;
        if response.response.status() != StatusCode::UNAUTHORIZED {
            return Ok(response);
        }
        let challenge = response
            .response
            .headers()
            .get("www-authenticate")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                "OCI registry rejected credentials without a supported challenge".to_string()
            })?;
        let challenge = parse_bearer_challenge(challenge)?;
        drop(response);
        let token = self
            .acquire_bearer_token(reference, auth, operation, &challenge)
            .await?;
        self.store_token(token_key, &token).await;
        let response = self
            .send_with_retries(OciSendRequest {
                method,
                url,
                authorization: Authorization::Bearer(token.value),
                content_type,
                accept,
                body,
                retryable,
            })
            .await?;
        if response.response.status() == StatusCode::UNAUTHORIZED {
            return Err("OCI registry rejected the bearer credential".to_string());
        }
        Ok(response)
    }

    async fn send_with_retries(&self, request: OciSendRequest<'_>) -> Result<OciResponse, String> {
        let OciSendRequest {
            method,
            url,
            authorization,
            content_type,
            accept,
            body,
            retryable,
        } = request;
        let maximum_attempts = if retryable {
            u32::from(self.policy.max_retries) + 1
        } else {
            1
        };
        let mut attempt = 0_u32;
        loop {
            attempt += 1;
            match self
                .send_once(
                    method.clone(),
                    url.clone(),
                    &authorization,
                    content_type,
                    accept,
                    body.clone(),
                )
                .await
            {
                Ok(response)
                    if attempt < maximum_attempts
                        && retryable_status(response.response.status()) =>
                {
                    drop(response);
                    tokio::time::sleep(retry_backoff(attempt)).await;
                }
                Ok(response) => return Ok(response),
                Err(error) if attempt < maximum_attempts && retryable => {
                    tokio::time::sleep(retry_backoff(attempt)).await;
                    let _ = error;
                }
                Err(_) => return Err("OCI registry request failed".to_string()),
            }
        }
    }

    async fn send_once(
        &self,
        method: Method,
        url: Url,
        authorization: &Authorization,
        content_type: Option<&str>,
        accept: Option<&str>,
        body: Option<Bytes>,
    ) -> Result<OciResponse, String> {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "OCI registry transport is unavailable".to_string())?;
        let mut request = self.client.request(method, url);
        if let Some(value) = content_type {
            request = request.header(CONTENT_TYPE, value);
        }
        if let Some(value) = accept {
            request = request.header(ACCEPT, value);
        }
        request = match authorization {
            Authorization::None => request,
            Authorization::Basic(username, password) => {
                request.basic_auth(username, Some(password))
            }
            Authorization::Bearer(token) => {
                request.header(AUTHORIZATION, format!("Bearer {token}"))
            }
        };
        if let Some(body) = body {
            request = request.body(body);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "OCI registry request failed".to_string())?;
        Ok(OciResponse {
            response,
            _permit: permit,
        })
    }

    async fn acquire_bearer_token(
        &self,
        reference: &RegistryReference,
        auth: &RegistryAuth,
        operation: Operation,
        challenge: &BearerChallenge,
    ) -> Result<BearerToken, String> {
        let registry_origin = reference.base_url()?;
        let authorization = if same_origin(&registry_origin, &challenge.realm) {
            Authorization::from_registry_auth(auth)
        } else {
            Authorization::None
        };
        let mut realm = challenge.realm.clone();
        let preserved_query = realm
            .query_pairs()
            .filter(|(key, _)| key != "service" && key != "scope")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        realm.set_query(None);
        {
            let mut query = realm.query_pairs_mut();
            for (key, value) in preserved_query {
                query.append_pair(&key, &value);
            }
            if let Some(service) = challenge.service.as_deref() {
                query.append_pair("service", service);
            }
            query.append_pair("scope", &operation.scope(reference));
        }
        let response = self
            .send_with_retries(OciSendRequest {
                method: Method::GET,
                url: realm,
                authorization,
                content_type: None,
                accept: Some("application/json"),
                body: None,
                retryable: true,
            })
            .await?;
        let response = expect_status(
            response,
            &[StatusCode::OK],
            "obtain an OCI bearer credential",
        )?;
        let maximum = self.response_limit().min(TOKEN_RESPONSE_MAXIMUM_BYTES);
        let bytes = self.read_bytes(response, maximum).await?;
        let token: TokenResponse = serde_json::from_slice(&bytes)
            .map_err(|_| "OCI token service returned an invalid response".to_string())?;
        let value = token
            .token
            .or(token.access_token)
            .filter(|value| !value.is_empty() && HeaderValue::from_str(value).is_ok())
            .ok_or_else(|| "OCI token service did not return a usable credential".to_string())?;
        let lifetime = Duration::from_secs(token.expires_in.unwrap_or(60).clamp(10, 3_600));
        Ok(BearerToken { value, lifetime })
    }

    async fn cached_token(&self, key: &TokenKey) -> Option<String> {
        let now = Instant::now();
        self.tokens
            .read()
            .await
            .get(key)
            .filter(|token| token.expires_at > now + Duration::from_secs(5))
            .map(|token| token.value.clone())
    }

    async fn store_token(&self, key: TokenKey, token: &BearerToken) {
        let lifetime = token.lifetime.saturating_sub(Duration::from_secs(5));
        let expires_at = Instant::now() + lifetime;
        self.tokens.write().await.insert(
            key,
            CachedToken {
                value: token.value.clone(),
                expires_at,
            },
        );
    }

    async fn read_bytes(&self, response: OciResponse, maximum: u64) -> Result<Vec<u8>, String> {
        self.ensure_transfer_headers(&response, maximum)?;
        let OciResponse { response, _permit } = response;
        let mut received = 0_u64;
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "OCI registry response transfer failed".to_string())?;
            received = received
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| "OCI registry response exceeds the transfer limit".to_string())?;
            if received > maximum {
                return Err("OCI registry response exceeds the transfer limit".to_string());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    fn ensure_transfer_headers(&self, response: &OciResponse, maximum: u64) -> Result<(), String> {
        if let Some(encoding) = response.response.headers().get(CONTENT_ENCODING)
            && encoding != "identity"
        {
            return Err("OCI registry response uses prohibited content encoding".to_string());
        }
        if let Some(length) = response
            .response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
        {
            let length = length
                .parse::<u64>()
                .map_err(|_| "OCI registry response has an invalid content length".to_string())?;
            if length > maximum {
                return Err("OCI registry response exceeds the transfer limit".to_string());
            }
        }
        Ok(())
    }

    fn response_limit(&self) -> u64 {
        self.policy
            .max_transfer_bytes
            .min(self.policy.max_decompressed_bytes)
    }
}

fn expect_status(
    response: OciResponse,
    expected: &[StatusCode],
    operation: &str,
) -> Result<OciResponse, String> {
    if expected.contains(&response.response.status()) {
        Ok(response)
    } else {
        Err(status_error(response, operation))
    }
}

fn status_error(response: OciResponse, operation: &str) -> String {
    let status = response.response.status().as_u16();
    drop(response);
    format!("OCI registry rejected the request to {operation} with HTTP {status}")
}

fn retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn retry_backoff(attempt: u32) -> Duration {
    RETRY_BACKOFF_BASE.saturating_mul(1_u32 << attempt.saturating_sub(1).min(3))
}

fn descriptor(blob: Blob<'_>) -> Result<OciDescriptor, String> {
    Ok(OciDescriptor {
        media_type: blob.media_type.to_string(),
        digest: blob.digest.to_string(),
        size: i64::try_from(blob.bytes.len())
            .map_err(|_| "OCI blob is too large to describe".to_string())?,
        urls: None,
        annotations: None,
    })
}

fn validate_blob(blob: Blob<'_>) -> Result<(), String> {
    if blob.bytes.is_empty()
        || !valid_digest(blob.digest)
        || !valid_media_type(blob.media_type)
        || digest_bytes(blob.bytes) != blob.digest
    {
        return Err("OCI blob does not match its immutable descriptor".to_string());
    }
    Ok(())
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.contains('/')
        && !value.chars().any(char::is_control)
}

fn valid_path_segment(value: &str) -> bool {
    value != "."
        && value != ".."
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn valid_reference(value: &str) -> bool {
    valid_digest(value)
        || (!value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_uppercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            }))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn parse_bearer_challenge(value: &str) -> Result<BearerChallenge, String> {
    let (scheme, parameters) = value
        .split_once(char::is_whitespace)
        .ok_or_else(|| "OCI registry returned an invalid authentication challenge".to_string())?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err("OCI registry returned an unsupported authentication challenge".to_string());
    }
    let parameters = parse_challenge_parameters(parameters.trim())?;
    let realm = parameters
        .get("realm")
        .ok_or_else(|| "OCI bearer challenge has no token realm".to_string())?;
    let realm = Url::parse(realm).map_err(|_| "OCI bearer token realm is invalid".to_string())?;
    if realm.scheme() != "https"
        || realm.host_str().is_none()
        || !realm.username().is_empty()
        || realm.password().is_some()
        || realm.fragment().is_some()
    {
        return Err("OCI bearer token realm is not an HTTPS origin".to_string());
    }
    Ok(BearerChallenge {
        realm,
        service: parameters.get("service").cloned(),
    })
}

fn parse_challenge_parameters(value: &str) -> Result<HashMap<String, String>, String> {
    let mut values = HashMap::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        let equals = rest
            .find('=')
            .ok_or_else(|| "OCI bearer challenge has malformed parameters".to_string())?;
        let name = rest[..equals].trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("OCI bearer challenge has malformed parameters".to_string());
        }
        rest = rest[equals + 1..].trim_start();
        let (parameter, remainder) = parse_challenge_value(rest)?;
        if values
            .insert(name.to_ascii_lowercase(), parameter)
            .is_some()
        {
            return Err("OCI bearer challenge repeats a parameter".to_string());
        }
        rest = remainder.trim_start();
        if rest.is_empty() {
            break;
        }
        rest = rest
            .strip_prefix(',')
            .ok_or_else(|| "OCI bearer challenge has malformed parameters".to_string())?
            .trim_start();
    }
    Ok(values)
}

fn parse_challenge_value(value: &str) -> Result<(String, &str), String> {
    if let Some(mut rest) = value.strip_prefix('"') {
        let mut result = String::new();
        let mut escaped = false;
        while let Some(character) = rest.chars().next() {
            rest = &rest[character.len_utf8()..];
            if escaped {
                result.push(character);
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                return Ok((result, rest));
            } else {
                result.push(character);
            }
        }
        return Err("OCI bearer challenge has an unterminated quoted value".to_string());
    }
    let end = value.find(',').unwrap_or(value.len());
    let parameter = value[..end].trim();
    if parameter.is_empty() {
        return Err("OCI bearer challenge has an empty parameter".to_string());
    }
    Ok((parameter.to_string(), &value[end..]))
}

#[cfg(test)]
mod tests {
    use super::{OciRegistryTransport, RegistryReference, parse_bearer_challenge};
    use crate::oci::OciRegistryTransportPolicy;

    #[test]
    fn strict_transport_constructs_with_a_complete_policy() {
        OciRegistryTransport::with_policy(OciRegistryTransportPolicy::strict())
            .expect("strict transport");
    }

    #[test]
    fn registry_reference_rejects_unsafe_path_data() {
        assert!(RegistryReference::new("registry.example", "modules/../unsafe", "latest").is_err());
        assert!(RegistryReference::new("registry.example", "modules/sample", "tag?query").is_err());
    }

    #[test]
    fn upload_location_cannot_cross_the_registry_origin() {
        let reference = RegistryReference::new("registry.example", "modules/sample", "latest")
            .expect("reference");

        assert!(
            reference
                .upload_location("https://attacker.example/v2/modules/sample/blobs/uploads/session")
                .is_err()
        );
        assert!(
            reference
                .upload_location("/v2/modules/sample/blobs/uploads/session")
                .is_ok()
        );
    }

    #[test]
    fn bearer_challenge_requires_an_https_realm() {
        assert!(
            parse_bearer_challenge(
                "Bearer realm=\"https://auth.example/token\",service=\"registry.example\""
            )
            .is_ok()
        );
        assert!(parse_bearer_challenge("Bearer realm=\"http://auth.example/token\"").is_err());
    }
}
