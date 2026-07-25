use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::model::{
    ProfilesStorefrontFollowState, ProfilesStorefrontImage, ProfilesStorefrontPage,
    ProfilesStorefrontProfile, SetProfilesStorefrontFollowCommand,
};

#[derive(Debug, Clone)]
pub struct NativeProfilesStorefrontError(pub String);

impl Display for NativeProfilesStorefrontError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeProfilesStorefrontError {}

impl From<ServerFnError> for NativeProfilesStorefrontError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_profile(
    handle: String,
    locale: Option<String>,
) -> Result<ProfilesStorefrontPage, NativeProfilesStorefrontError> {
    profiles_storefront_profile_native(handle, locale)
        .await
        .map_err(Into::into)
}

pub async fn set_follow(
    command: SetProfilesStorefrontFollowCommand,
) -> Result<ProfilesStorefrontFollowState, NativeProfilesStorefrontError> {
    profiles_storefront_follow_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "profiles/storefront/profile")]
async fn profiles_storefront_profile_native(
    handle: String,
    locale: Option<String>,
) -> Result<ProfilesStorefrontPage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            HostRuntimeContext, OptionalAuthContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_media::{MediaPublicImageReadPort, MediaPublicImageService};
        use rustok_profiles::{
            ProfileAccessAudience, ProfileError, ProfileMediaPublicImageProvider,
            ProfileMediaSlot, ProfilePresentationService,
        };
        use rustok_social_graph::{
            SocialGraphFollowReadPort, SocialGraphPairRequest, SocialGraphService,
        };
        use rustok_storage::StorageRuntime;
        use std::{sync::Arc, time::Duration};
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let auth = leptos_axum::extract::<OptionalAuthContext>()
            .await
            .map_err(ServerFnError::new)?
            .0;
        if auth.as_ref().is_some_and(|auth| auth.tenant_id != tenant.id) {
            return Err(ServerFnError::new("profiles tenant mismatch"));
        }
        let human_auth = auth.filter(|auth| !auth.is_service_principal());
        let viewer_authenticated = human_auth.is_some();
        let audience = human_auth
            .as_ref()
            .map(|auth| ProfileAccessAudience::Authenticated {
                actor_id: auth.user_id,
            })
            .unwrap_or(ProfileAccessAudience::Anonymous);
        let effective_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(request.locale.as_str())
            .to_string();
        let db = runtime.db_clone();
        let profile = match ProfilePresentationService::for_audience(db.clone(), audience)
            .get_profile_by_handle(
                tenant.id,
                handle.trim(),
                Some(effective_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
        {
            Ok(profile) => profile,
            Err(ProfileError::ProfileByHandleNotFound(_)) => {
                return Ok(ProfilesStorefrontPage {
                    profile: None,
                    viewer_authenticated,
                    is_self: false,
                    follow_state: None,
                });
            }
            Err(error) => return Err(ServerFnError::new(error.to_string())),
        };

        let media_public_images = runtime
            .shared_get::<ProfileMediaPublicImageProvider>()
            .map(|provider| provider.port())
            .or_else(|| {
                runtime.shared_get::<StorageRuntime>().map(|storage| {
                    Arc::new(MediaPublicImageService::new(db.clone(), storage))
                        as Arc<dyn MediaPublicImageReadPort>
                })
            });
        let (avatar_image, banner_image) = if let Some(media) = media_public_images {
            let alt = Some(profile.display_name.clone());
            (
                load_public_profile_image(
                    media.as_ref(),
                    tenant.id,
                    profile.user_id,
                    profile.avatar_media_id,
                    alt.clone(),
                    ProfileMediaSlot::Avatar,
                    effective_locale.as_str(),
                    request.channel_slug.as_deref(),
                )
                .await,
                load_public_profile_image(
                    media.as_ref(),
                    tenant.id,
                    profile.user_id,
                    profile.banner_media_id,
                    alt,
                    ProfileMediaSlot::Banner,
                    effective_locale.as_str(),
                    request.channel_slug.as_deref(),
                )
                .await,
            )
        } else {
            (None, None)
        };

        let is_self = human_auth
            .as_ref()
            .is_some_and(|auth| auth.user_id == profile.user_id);
        let follow_state = if let Some(auth) = human_auth.as_ref().filter(|_| !is_self) {
            let mut context = PortContext::new(
                tenant.id.to_string(),
                PortActor::user(auth.user_id.to_string()),
                effective_locale.clone(),
                format!("profiles-storefront-native-read-{}", Uuid::new_v4()),
            )
            .with_deadline(Duration::from_secs(5));
            for permission in &auth.permissions {
                context = context.with_claim(permission.to_string());
            }
            if let Some(channel) = request.channel_slug.as_deref() {
                context = context.with_channel(channel.to_string());
            }
            SocialGraphFollowReadPort::source_follow_state(
                &SocialGraphService::new(db),
                context,
                SocialGraphPairRequest {
                    source_user_id: auth.user_id,
                    target_user_id: profile.user_id,
                },
            )
            .await
            .ok()
            .map(|state| ProfilesStorefrontFollowState {
                user_id: state.target_user_id.to_string(),
                following: state.following,
                revision: state.revision.map(|revision| revision.to_string()),
            })
        } else {
            None
        };

        Ok(ProfilesStorefrontPage {
            profile: Some(map_profile(profile, avatar_image, banner_image)),
            viewer_authenticated,
            is_self,
            follow_state,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (handle, locale);
        Err(ServerFnError::new(
            "profiles storefront native transport requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "profiles/storefront/follow")]
async fn profiles_storefront_follow_native(
    command: SetProfilesStorefrontFollowCommand,
) -> Result<ProfilesStorefrontFollowState, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_social_graph::{
            SetSocialRelationCommand, SocialGraphCommandPort, SocialGraphService,
            SocialRelationKind,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.is_service_principal() {
            return Err(ServerFnError::new(
                "follow operations require human-user credentials",
            ));
        }
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("profiles tenant mismatch"));
        }
        let target_user_id = Uuid::parse_str(command.user_id.trim())
            .map_err(|_| ServerFnError::new("user_id must be a UUID"))?;
        let expected_revision = command
            .expected_revision
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                value
                    .parse::<i64>()
                    .ok()
                    .filter(|revision| *revision > 0)
                    .ok_or_else(|| ServerFnError::new("expected revision must be positive"))
            })
            .transpose()?;
        let idempotency_key = command.idempotency_key.trim();
        if idempotency_key.is_empty() {
            return Err(ServerFnError::new("idempotency key must not be empty"));
        }
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("profiles-storefront-native-write-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(idempotency_key.to_string());
        for permission in &auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        if let Some(channel) = request.channel_slug.as_deref() {
            context = context.with_channel(channel.to_string());
        }
        let relation = SocialGraphCommandPort::set_relation(
            &SocialGraphService::new(runtime.db_clone()),
            context,
            SetSocialRelationCommand {
                source_user_id: auth.user_id,
                target_user_id,
                relation_kind: SocialRelationKind::Follow,
                active: command.following,
                expected_revision,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;

        Ok(ProfilesStorefrontFollowState {
            user_id: relation.target_user_id.to_string(),
            following: relation.active,
            revision: Some(relation.revision.to_string()),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "profiles storefront native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn load_public_profile_image(
    media: &dyn rustok_media::MediaPublicImageReadPort,
    tenant_id: uuid::Uuid,
    profile_user_id: uuid::Uuid,
    media_id: Option<uuid::Uuid>,
    alt: Option<String>,
    slot: rustok_profiles::ProfileMediaSlot,
    locale: &str,
    channel: Option<&str>,
) -> Option<ProfilesStorefrontImage> {
    use rustok_api::{PortActor, PortContext};
    use std::time::Duration;

    let media_id = media_id?;
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("profiles-storefront-presentation"),
        locale,
        format!(
            "profiles-storefront-native-{}-image-{}",
            slot.as_str(),
            uuid::Uuid::new_v4()
        ),
    )
    .with_deadline(Duration::from_secs(2));
    if let Some(channel) = channel {
        context = context.with_channel(channel.to_string());
    }

    match media.get_public_image_asset(context, media_id, alt).await {
        Ok(public_asset) => {
            if let Err(error) = rustok_profiles::validate_profile_media_asset(
                tenant_id,
                profile_user_id,
                slot,
                &public_asset.asset,
            ) {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    profile_user_id = %profile_user_id,
                    profile_media_slot = slot.as_str(),
                    media_id = %media_id,
                    error = %error,
                    "Storefront profile image reference failed owner validation"
                );
                return None;
            }
            public_asset
                .descriptor
                .and_then(rustok_profiles::profile_image_presentation)
                .map(map_profile_image)
        }
        Err(error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                profile_user_id = %profile_user_id,
                profile_media_slot = slot.as_str(),
                media_id = %media_id,
                error_code = %error.code,
                "Storefront profile image presentation is unavailable"
            );
            None
        }
    }
}

#[cfg(feature = "ssr")]
fn map_profile_image(
    value: rustok_profiles::ProfileImagePresentation,
) -> ProfilesStorefrontImage {
    ProfilesStorefrontImage {
        url: value.url,
        alt: value.alt,
        width: value.width,
        height: value.height,
        mime_type: value.mime_type,
    }
}

#[cfg(feature = "ssr")]
fn map_profile(
    value: rustok_profiles::ProfileRecord,
    avatar_image: Option<ProfilesStorefrontImage>,
    banner_image: Option<ProfilesStorefrontImage>,
) -> ProfilesStorefrontProfile {
    ProfilesStorefrontProfile {
        user_id: value.user_id.to_string(),
        handle: value.handle,
        display_name: value.display_name,
        bio: value.bio,
        tags: value.tags,
        avatar_media_id: value.avatar_media_id.map(|id| id.to_string()),
        banner_media_id: value.banner_media_id.map(|id| id.to_string()),
        avatar_image,
        banner_image,
        preferred_locale: value.preferred_locale,
        visibility: value.visibility.as_str().to_string(),
    }
}
