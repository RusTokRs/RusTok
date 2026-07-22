use async_graphql::{Enum, InputObject, SimpleObject};
use uuid::Uuid;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Enum)]
pub enum GqlMenuLocation { Header, Footer, Sidebar, Mobile }

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlMenu {
    pub id: Uuid,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub location: GqlMenuLocation,
    pub items: Vec<GqlMenuItem>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlActiveMenuBinding {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Uuid,
    pub location: GqlMenuLocation,
    pub menu_id: Uuid,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlMenuItem {
    pub id: Uuid,
    pub title: String,
    pub url: String,
    pub icon: Option<String>,
    pub children: Vec<GqlMenuItem>,
}

#[derive(InputObject)]
pub struct CreateGqlMenuInput {
    pub translations: Vec<GqlMenuTranslationInput>,
    pub location: GqlMenuLocation,
    pub items: Vec<GqlMenuItemInput>,
}

#[derive(InputObject)]
pub struct BindGqlActiveMenuInput { pub location: GqlMenuLocation, pub menu_id: Uuid }
#[derive(InputObject)]
pub struct GqlMenuTranslationInput { pub locale: String, pub name: String }
#[derive(InputObject)]
pub struct GqlMenuItemTranslationInput { pub locale: String, pub title: String }
#[derive(InputObject)]
pub struct GqlMenuItemInput {
    pub translations: Vec<GqlMenuItemTranslationInput>,
    pub url: Option<String>,
    pub icon: Option<String>,
    pub position: i32,
    pub children: Option<Vec<GqlMenuItemInput>>,
}

impl From<crate::MenuResponse> for GqlMenu {
    fn from(menu: crate::MenuResponse) -> Self {
        Self { id: menu.id, effective_locale: menu.effective_locale, available_locales: menu.available_locales,
            name: menu.name, location: menu.location.into(), items: menu.items.into_iter().map(Into::into).collect() }
    }
}
impl From<crate::ActiveMenuBindingResponse> for GqlActiveMenuBinding {
    fn from(binding: crate::ActiveMenuBindingResponse) -> Self {
        Self { id: binding.id, tenant_id: binding.tenant_id, channel_id: binding.channel_id,
            location: binding.location.into(), menu_id: binding.menu_id }
    }
}
impl From<crate::MenuItemResponse> for GqlMenuItem {
    fn from(item: crate::MenuItemResponse) -> Self {
        Self { id: item.id, title: item.title, url: item.url, icon: item.icon,
            children: item.children.into_iter().map(Into::into).collect() }
    }
}
impl From<GqlMenuLocation> for crate::MenuLocation {
    fn from(location: GqlMenuLocation) -> Self { match location {
        GqlMenuLocation::Header => Self::Header, GqlMenuLocation::Footer => Self::Footer,
        GqlMenuLocation::Sidebar => Self::Sidebar, GqlMenuLocation::Mobile => Self::Mobile } }
}
impl From<crate::MenuLocation> for GqlMenuLocation {
    fn from(location: crate::MenuLocation) -> Self { match location {
        crate::MenuLocation::Header => Self::Header, crate::MenuLocation::Footer => Self::Footer,
        crate::MenuLocation::Sidebar => Self::Sidebar, crate::MenuLocation::Mobile => Self::Mobile } }
}
