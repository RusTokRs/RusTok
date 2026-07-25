use std::sync::Arc;

use rustok_media::{MediaImageDescriptor, MediaItem, MediaPublicImageReadPort};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ProfileError, ProfileResult};

/// Host-selected Media public-image provider used by profile presentation adapters.
///
/// The wrapper is intentionally transport-neutral. Embedded deployments may wrap
/// `MediaPublicImageService`; extracted deployments may wrap `GrpcMediaProvider`.
/// Profiles receives only the owner port and never sees endpoints, storage handles,
/// or Media route construction.
#[derive(Clone)]
pub struct ProfileMediaPublicImageProvider {
    port: Arc<dyn MediaPublicImageReadPort>,
}

impl ProfileMediaPublicImageProvider {
    pub fn new(port: Arc<dyn MediaPublicImageReadPort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> Arc<dyn MediaPublicImageReadPort> {
        Arc::clone(&self.port)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileMediaSlot {
    Avatar,
    Banner,
}

impl ProfileMediaSlot {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Avatar => "avatar",
            Self::Banner => "banner",
        }
    }
}

/// Public-safe image presentation produced from a Media owner descriptor.
///
/// Profiles never rewrites storage paths or invents proxy URLs. Only descriptors
/// that Media classifies as directly public and that still describe an image are
/// exposed to profile transports.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileImagePresentation {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

pub fn profile_image_presentation(
    descriptor: MediaImageDescriptor,
) -> Option<ProfileImagePresentation> {
    let mime_type = descriptor
        .mime_type
        .as_deref()?
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !mime_type.starts_with("image/") {
        return None;
    }

    let url = descriptor.normalized_public_url()?.to_string();
    Some(ProfileImagePresentation {
        url,
        alt: descriptor.alt,
        width: descriptor.width,
        height: descriptor.height,
        mime_type: Some(mime_type),
    })
}

/// Validate a media-owner result before a profile persists an avatar/banner reference.
///
/// Existence and tenant-scoped lookup are performed through `MediaAssetReadPort`.
/// This final owner-side guard keeps profiles from accepting an adapter result for
/// another tenant, another uploader, or a non-image asset.
pub fn validate_profile_media_asset(
    tenant_id: Uuid,
    profile_user_id: Uuid,
    slot: ProfileMediaSlot,
    asset: &MediaItem,
) -> ProfileResult<()> {
    if asset.tenant_id != tenant_id {
        return Err(ProfileError::Validation(format!(
            "profile {} media must belong to the current tenant",
            slot.as_str()
        )));
    }

    if asset.uploaded_by != Some(profile_user_id) {
        return Err(ProfileError::Validation(format!(
            "profile {} media must be uploaded by the profile owner",
            slot.as_str()
        )));
    }

    let mime_type = asset
        .mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if !mime_type.starts_with("image/") {
        return Err(ProfileError::Validation(format!(
            "profile {} media must be an image",
            slot.as_str()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn asset(tenant_id: Uuid, uploaded_by: Option<Uuid>, mime_type: &str) -> MediaItem {
        MediaItem {
            id: Uuid::new_v4(),
            tenant_id,
            uploaded_by,
            filename: "profile-image.png".to_string(),
            original_name: "profile-image.png".to_string(),
            mime_type: mime_type.to_string(),
            size: 128,
            storage_path: "profiles/profile-image.png".to_string(),
            storage_driver: "memory".to_string(),
            public_url: "/media/profile-image.png".to_string(),
            width: Some(256),
            height: Some(256),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_owner_image_from_current_tenant() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let asset = asset(tenant_id, Some(user_id), "image/png");

        validate_profile_media_asset(tenant_id, user_id, ProfileMediaSlot::Avatar, &asset)
            .unwrap();
    }

    #[test]
    fn rejects_cross_tenant_asset() {
        let user_id = Uuid::new_v4();
        let asset = asset(Uuid::new_v4(), Some(user_id), "image/png");

        let error = validate_profile_media_asset(
            Uuid::new_v4(),
            user_id,
            ProfileMediaSlot::Banner,
            &asset,
        )
        .unwrap_err();
        assert!(error.to_string().contains("current tenant"));
    }

    #[test]
    fn rejects_asset_uploaded_by_another_user() {
        let tenant_id = Uuid::new_v4();
        let asset = asset(tenant_id, Some(Uuid::new_v4()), "image/jpeg");

        let error = validate_profile_media_asset(
            tenant_id,
            Uuid::new_v4(),
            ProfileMediaSlot::Avatar,
            &asset,
        )
        .unwrap_err();
        assert!(error.to_string().contains("profile owner"));
    }

    #[test]
    fn rejects_non_image_asset() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let asset = asset(tenant_id, Some(user_id), "video/mp4");

        let error = validate_profile_media_asset(
            tenant_id,
            user_id,
            ProfileMediaSlot::Banner,
            &asset,
        )
        .unwrap_err();
        assert!(error.to_string().contains("must be an image"));
    }

    #[test]
    fn presentation_exposes_only_direct_public_image_descriptors() {
        let absolute = MediaImageDescriptor::from_parts(
            "https://cdn.example.test/avatar.webp",
            Some("Avatar".to_string()),
            Some(128),
            Some(128),
            Some("image/webp".to_string()),
        )
        .unwrap();
        let root_relative = MediaImageDescriptor::from_parts(
            "/media/banner.png",
            None,
            Some(1200),
            Some(300),
            Some("image/png".to_string()),
        )
        .unwrap();
        let storage_relative = MediaImageDescriptor::from_parts(
            "tenant-a/profile/avatar.webp",
            None,
            Some(128),
            Some(128),
            Some("image/webp".to_string()),
        )
        .unwrap();
        let non_image = MediaImageDescriptor::from_parts(
            "https://cdn.example.test/document.pdf",
            None,
            None,
            None,
            Some("application/pdf".to_string()),
        )
        .unwrap();

        assert_eq!(
            profile_image_presentation(absolute)
                .as_ref()
                .map(|image| image.url.as_str()),
            Some("https://cdn.example.test/avatar.webp")
        );
        assert_eq!(
            profile_image_presentation(root_relative)
                .as_ref()
                .map(|image| image.url.as_str()),
            Some("/media/banner.png")
        );
        assert!(profile_image_presentation(storage_relative).is_none());
        assert!(profile_image_presentation(non_image).is_none());
    }
}
