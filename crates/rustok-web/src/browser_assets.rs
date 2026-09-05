//! Release-qualified and content-addressed browser asset registry and resolution service.
//!
//! Enforces:
//! - Every browser asset is release-qualified and content-addressed with immutable cache headers.
//! - Dual N and N+1 retention for the measured client/cache lifetime across rollouts and rollbacks.
//! - Strict HTTP 404 Not Found for any missing immutable asset (never fall back to HTML or default routes).

use std::collections::HashMap;
use std::fmt::Write as _;

use axum::{
    body::Body,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::Response,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{content_etag, if_none_match_matches};

pub const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
pub const MISSING_ASSET_CACHE_CONTROL: &str = "no-cache, no-store, must-revalidate";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseQualifiedAsset {
    pub logical_path: String,
    pub content_digest: String,
    pub content_type: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub etag: String,
}

impl ReleaseQualifiedAsset {
    pub fn new(
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Self {
        let hash = Sha256::digest(&bytes);
        let mut digest = String::with_capacity(7 + hash.len() * 2);
        digest.push_str("sha256:");
        for byte in hash {
            write!(&mut digest, "{byte:02x}").expect("writing to String cannot fail");
        }
        let etag = content_etag(&bytes);
        Self {
            logical_path: logical_path.into(),
            content_digest: digest,
            content_type: content_type.into(),
            bytes,
            etag,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReleaseAssetSet {
    pub release_id: String,
    pub registered_at: DateTime<Utc>,
    pub retained_until: DateTime<Utc>,
    pub assets: HashMap<String, ReleaseQualifiedAsset>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BrowserAssetError {
    #[error("Release `{0}` not found in asset registry")]
    ReleaseNotFound(String),
    #[error("Asset `{0}` in release `{1}` is missing or expired")]
    AssetNotFound(String, String),
}

/// Registry managing content-addressed browser assets for active (N) and candidate/predecessor (N+1 / N-1) releases.
#[derive(Clone, Debug)]
pub struct BrowserAssetRegistry {
    active_release_id: Option<String>,
    retained_releases: HashMap<String, ReleaseAssetSet>,
    client_cache_lifetime: Duration,
}

impl BrowserAssetRegistry {
    pub fn new(client_cache_lifetime: Duration) -> Self {
        Self {
            active_release_id: None,
            retained_releases: HashMap::new(),
            client_cache_lifetime,
        }
    }

    pub fn active_release_id(&self) -> Option<&str> {
        self.active_release_id.as_deref()
    }

    pub fn client_cache_lifetime(&self) -> Duration {
        self.client_cache_lifetime
    }

    /// Registers a release-qualified set of browser assets.
    /// Retention window is set to `now + client_cache_lifetime`.
    pub fn register_release(
        &mut self,
        release_id: impl Into<String>,
        assets: Vec<ReleaseQualifiedAsset>,
        now: DateTime<Utc>,
    ) {
        let id = release_id.into();
        let mut map = HashMap::with_capacity(assets.len());
        for asset in assets {
            map.insert(asset.logical_path.clone(), asset);
        }

        let retained_until = now + self.client_cache_lifetime;
        self.retained_releases.insert(
            id.clone(),
            ReleaseAssetSet {
                release_id: id,
                registered_at: now,
                retained_until,
                assets: map,
            },
        );
    }

    /// Activates a release (e.g. promoting N+1 to N).
    /// Predecessor release (N) remains in `retained_releases` with retention extended
    /// for `client_cache_lifetime` from now.
    pub fn activate_release(&mut self, release_id: &str, now: DateTime<Utc>) -> Result<(), BrowserAssetError> {
        if !self.retained_releases.contains_key(release_id) {
            return Err(BrowserAssetError::ReleaseNotFound(release_id.to_string()));
        }

        // Extend retention for previous active release
        if let Some(prev_set) = self
            .active_release_id
            .as_deref()
            .and_then(|prev_active| self.retained_releases.get_mut(prev_active))
        {
            prev_set.retained_until = now + self.client_cache_lifetime;
        }

        // Active release must stay retained indefinitely while active
        if let Some(new_set) = self.retained_releases.get_mut(release_id) {
            new_set.retained_until = now + self.client_cache_lifetime;
        }

        self.active_release_id = Some(release_id.to_string());
        Ok(())
    }

    /// Checks whether a release is currently retained and valid.
    pub fn is_release_retained(&self, release_id: &str, now: DateTime<Utc>) -> bool {
        if self.active_release_id.as_deref() == Some(release_id) {
            return true;
        }
        self.retained_releases
            .get(release_id)
            .is_some_and(|set| now <= set.retained_until)
    }

    /// Resolves an immutable release-qualified asset.
    ///
    /// Contracts:
    /// - If asset exists in active (N) or retained (N+1 / N-1) release:
    ///   - Supports `If-None-Match` returning HTTP 304 Not Modified.
    ///   - Otherwise returns HTTP 200 OK with `Cache-Control: public, max-age=31536000, immutable`.
    /// - If asset is missing or release is not retained:
    ///   - STRICT NOT-FOUND: Returns HTTP 404 NOT_FOUND with `Cache-Control: no-cache, no-store`.
    ///   - Never falls back to HTML or default routes!
    pub fn resolve_asset(
        &self,
        headers: &HeaderMap,
        release_id: &str,
        logical_path: &str,
        now: DateTime<Utc>,
    ) -> Response {
        let release_set = match self.retained_releases.get(release_id) {
            Some(set) => {
                let is_active = self.active_release_id.as_deref() == Some(release_id);
                if !is_active && now > set.retained_until {
                    return self.strict_not_found_response("release asset expired");
                }
                set
            }
            None => return self.strict_not_found_response("release not found in registry"),
        };

        let asset = match release_set.assets.get(logical_path) {
            Some(a) => a,
            None => return self.strict_not_found_response("asset not found in release"),
        };

        // Check conditional If-None-Match
        let not_modified = headers
            .get(IF_NONE_MATCH)
            .and_then(|val| val.to_str().ok())
            .is_some_and(|val| if_none_match_matches(val, asset.etag.as_str()));

        if not_modified {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header(CACHE_CONTROL, IMMUTABLE_ASSET_CACHE_CONTROL)
                .header(ETAG, &asset.etag)
                .header("cross-origin-resource-policy", "same-origin")
                .body(Body::empty())
                .unwrap_or_else(|_| Response::new(Body::empty()));
        }

        let content_type = HeaderValue::from_str(&asset.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));

        Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type)
            .header(CACHE_CONTROL, IMMUTABLE_ASSET_CACHE_CONTROL)
            .header(ETAG, &asset.etag)
            .header("cross-origin-resource-policy", "same-origin")
            .body(Body::from(asset.bytes.clone()))
            .unwrap_or_else(|_| Response::new(Body::empty()))
    }

    /// Strict HTTP 404 response for missing or expired immutable assets.
    /// Never returns HTML or allows client caching of broken assets.
    fn strict_not_found_response(&self, detail: &str) -> Response {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(CACHE_CONTROL, MISSING_ASSET_CACHE_CONTROL)
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .header("X-Content-Type-Options", "nosniff")
            .body(Body::from(format!("404 Not Found: {detail}")))
            .unwrap_or_else(|_| Response::new(Body::empty()))
    }

    /// Prunes releases that have expired beyond their retention window and are not active.
    pub fn prune_expired_releases(&mut self, now: DateTime<Utc>) -> usize {
        let active = self.active_release_id.clone();
        let mut expired = Vec::new();

        for (id, set) in &self.retained_releases {
            if active.as_deref() != Some(id) && now > set.retained_until {
                expired.push(id.clone());
            }
        }

        for id in &expired {
            self.retained_releases.remove(id);
        }

        expired.len()
    }
}
