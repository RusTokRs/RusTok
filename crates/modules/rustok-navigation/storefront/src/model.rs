use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontMenu {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    pub name: String,
    pub location: StorefrontMenuLocation,
    pub items: Vec<StorefrontMenuItem>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorefrontMenuLocation {
    Header,
    Footer,
    Sidebar,
    Mobile,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontMenuItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub children: Vec<StorefrontMenuItem>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorefrontNavigationSnapshot {
    pub header: Option<StorefrontMenu>,
    pub footer: Option<StorefrontMenu>,
}

impl StorefrontNavigationSnapshot {
    pub fn menu(&self, location: StorefrontMenuLocation) -> Option<&StorefrontMenu> {
        match location {
            StorefrontMenuLocation::Header => self.header.as_ref(),
            StorefrontMenuLocation::Footer => self.footer.as_ref(),
            StorefrontMenuLocation::Sidebar | StorefrontMenuLocation::Mobile => None,
        }
    }
}
