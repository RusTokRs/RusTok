use std::{sync::Arc, time::Duration};

use async_graphql::{ComplexObject, Context, Enum, InputObject, SimpleObject};
use rustok_api::{ChannelContext, PortActor, PortContext, RequestContext};
use rustok_core::ModuleRuntimeExtensions;
use rustok_media::{MediaPublicImageReadPort, MediaPublicImageService};
use rustok_storage::StorageRuntime;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    ProfileImagePresentation, ProfileMediaPublicImageProvider, ProfileMediaSlot, ProfileRecord,
    ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput,
    profile_image_presentation, validate_profile_media_asset,
};

const PROFILE_MEDIA_PRESENTATION_DEADLINE: Duration = Duration::from_secs(2);

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlProfileVisibility {
    Public,
    Authenticated,
    FollowersOnly,
    Private,
}

impl From<ProfileVisibility> for GqlProfileVisibility {
    fn from(value: ProfileVisibility) -> Self {
        match value {
            ProfileVisibility::Public => Self::Public,
            ProfileVisibility::Authenticated => Self::Authenticated,
            ProfileVisibility::FollowersOnly => Self::FollowersOnly,
            ProfileVisibility::Private => Self::Private,
        }
    }
}

impl From<GqlProfileVisibility> for ProfileVisibility {
    fn from(value: GqlProfileVisibility) -> Self {
        match value {
            GqlProfileVisibility::Public => Self::Public,
            GqlProfileVisibility::Authenticated => Self::Authenticated,
            GqlProfileVisibility::FollowersOnly => Self::FollowersOnly,
            GqlProfileVisibility::Private => Self::Private,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlProfileStatus {
    Active,
    Hidden,
    Blocked,
}

impl From<ProfileStatus> for GqlProfileStatus {
    fn from(value: ProfileStatus) -> Self {
        match value {
            ProfileStatus::Active => Self::Active,
            ProfileStatus::Hidden => Self::Hidden,
            ProfileStatus::Blocked => Self::Blocked,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GqlProfileImage {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

impl From<ProfileImagePresentation> for GqlProfileImage {
    fn from(value: ProfileImagePresentation) -> Self {
        Self {
            url: value.url,
            alt: value.alt,
            width: value.width,
            height: value.height,
            mime_type: value.mime_type,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(complex)]
pub struct GqlProfile {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
    pub status: GqlProfileStatus,
}

#[ComplexObject]
impl GqlProfile {
    async fn avatar_image(&self, ctx: &Context<'_>) -> Option<GqlProfileImage> {
        resolve_public_profile_image(
            ctx,
            self.tenant_id,
            self.user_id,
            self.avatar_media_id,
            Some(self.display_name.clone()),
            ProfileMediaSlot::Avatar,
        )
        .await
    }

    async fn banner_image(&self, ctx: &Context<'_>) -> Option<GqlProfileImage> {
        resolve_public_profile_image(
            ctx,
            self.tenant_id,
            self.user_id,
            self.banner_media_id,
            Some(self.display_name.clone()),
            ProfileMediaSlot::Banner,
        )
        .await
    }
}

impl From<ProfileRecord> for GqlProfile {
    fn from(value: ProfileRecord) -> Self {
        Self {
            tenant_id: value.tenant_id,
            user_id: value.user_id,
            handle: value.handle,
            display_name: value.display_name,
            bio: value.bio,
            tags: value.tags,
            avatar_media_id: value.avatar_media_id,
            banner_media_id: value.banner_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
            status: value.status.into(),
        }
    }
}

fn public_image_provider(ctx: &Context<'_>) -> Option<Arc<dyn MediaPublicImageReadPort>> {
    if let Some(provider) = ctx.data_opt::<ProfileMediaPublicImageProvider>() {
        return Some(provider.port());
    }
    if let Some(provider) = ctx
        .data_opt::<Arc<ModuleRuntimeExtensions>>()
        .and_then(|extensions| extensions.get::<ProfileMediaPublicImageProvider>())
    {
        return Some(provider.port());
    }

    let db = ctx.data_opt::<DatabaseConnection>()?;
    let storage = ctx.data_opt::<StorageRuntime>()?;
    Some(Arc::new(MediaPublicImageService::new(
        db.clone(),
        storage.clone(),
    )))
}

async fn resolve_public_profile_image(
    ctx: &Context<'_>,
    tenant_id: Uuid,
    profile_user_id: Uuid,
    media_id: Option<Uuid>,
    alt: Option<String>,
    slot: ProfileMediaSlot,
) -> Option<GqlProfileImage> {
    let media_id = media_id?;
    let provider = public_image_provider(ctx)?;
    let request = ctx.data_opt::<RequestContext>();
    let locale = request
        .map(|request| request.locale.as_str())
        .unwrap_or("und");
    let mut port_context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("profiles-graphql-presentation"),
        locale,
        format!(
            "profiles-graphql-{}-image-{}",
            slot.as_str(),
            Uuid::new_v4()
        ),
    )
    .with_deadline(PROFILE_MEDIA_PRESENTATION_DEADLINE);
    if let Some(channel) = ctx.data_opt::<ChannelContext>() {
        port_context = port_context.with_channel(channel.slug.clone());
    }

    match provider
        .get_public_image_asset(port_context, media_id, alt)
        .await
    {
        Ok(public_asset) => {
            if let Err(error) =
                validate_profile_media_asset(tenant_id, profile_user_id, slot, &public_asset.asset)
            {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    profile_user_id = %profile_user_id,
                    profile_media_slot = slot.as_str(),
                    media_id = %media_id,
                    error = %error,
                    "Profile image reference failed owner validation"
                );
                return None;
            }
            public_asset
                .descriptor
                .and_then(profile_image_presentation)
                .map(Into::into)
        }
        Err(error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                profile_user_id = %profile_user_id,
                profile_media_slot = slot.as_str(),
                media_id = %media_id,
                error_code = %error.code,
                "Profile image presentation is unavailable"
            );
            None
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GqlProfileSummary {
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
}

impl From<ProfileSummary> for GqlProfileSummary {
    fn from(value: ProfileSummary) -> Self {
        Self {
            user_id: value.user_id,
            handle: value.handle,
            display_name: value.display_name,
            tags: value.tags,
            avatar_media_id: value.avatar_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpsertProfileInput {
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Option<Vec<String>>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
}

impl From<GqlUpsertProfileInput> for UpsertProfileInput {
    fn from(value: GqlUpsertProfileInput) -> Self {
        Self {
            handle: value.handle,
            display_name: value.display_name,
            bio: value.bio,
            tags: value.tags.unwrap_or_default(),
            avatar_media_id: value.avatar_media_id,
            banner_media_id: value.banner_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpdateMyProfileContentInput {
    pub display_name: String,
    pub bio: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpdateMyProfileMediaInput {
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
}
