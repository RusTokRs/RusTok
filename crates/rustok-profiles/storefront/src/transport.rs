#[path = "transport/graphql_adapter.rs"]
mod graphql_adapter;
#[path = "transport/native_server_adapter.rs"]
mod native_server_adapter;

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::ProfilesStorefrontTransportProfile;
use crate::model::{
    ProfilesStorefrontFollowState, ProfilesStorefrontPage, SetProfilesStorefrontFollowCommand,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfilesStorefrontTransportContext {
    pub profile: ProfilesStorefrontTransportProfile,
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
    pub current_user_id: Option<String>,
}

impl ProfilesStorefrontTransportContext {
    pub fn native() -> Self {
        Self {
            profile: ProfilesStorefrontTransportProfile::Native,
            access_token: None,
            tenant_slug: None,
            current_user_id: None,
        }
    }

    pub fn graphql_with_access_token(
        access_token: Option<String>,
        tenant_slug: Option<String>,
        current_user_id: Option<String>,
    ) -> Self {
        Self {
            profile: ProfilesStorefrontTransportProfile::Graphql,
            access_token,
            tenant_slug,
            current_user_id,
        }
    }

    fn path(&self) -> UiTransportPath {
        match self.profile {
            ProfilesStorefrontTransportProfile::Native => UiTransportPath::NativeServer,
            ProfilesStorefrontTransportProfile::Graphql => UiTransportPath::Graphql,
        }
    }
}

pub async fn load_profiles_storefront_page(
    context: ProfilesStorefrontTransportContext,
    handle: String,
    locale: Option<String>,
) -> UiTransportResult<ProfilesStorefrontPage> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let current_user_id = context.current_user_id.clone();
    let native_handle = handle.clone();
    let native_locale = locale.clone();
    execute_selected_transport(
        "profiles.storefront.page",
        context.path(),
        move || native_server_adapter::load_profile(native_handle, native_locale),
        move || {
            graphql_adapter::load_profile(
                token,
                tenant,
                current_user_id,
                handle,
                locale,
            )
        },
    )
    .await
}

pub async fn set_profiles_storefront_follow(
    context: ProfilesStorefrontTransportContext,
    command: SetProfilesStorefrontFollowCommand,
) -> UiTransportResult<ProfilesStorefrontFollowState> {
    let token = context.access_token.clone();
    let tenant = context.tenant_slug.clone();
    let native_command = command.clone();
    execute_selected_transport(
        "profiles.storefront.follow.set",
        context.path(),
        move || native_server_adapter::set_follow(native_command),
        move || graphql_adapter::set_follow(token, tenant, command),
    )
    .await
}

pub const PROFILES_STOREFRONT_TRANSPORT_FALLBACK_POLICY: &str = "never falls back";
