use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdminQueryKey {
    ProductId,
    CartId,
    OrderId,
    CustomerId,
    RegionId,
    ShippingProfileId,
    ShippingOptionId,
    PostId,
    PageId,
    ThreadId,
    MediaId,
    CategoryId,
    TopicId,
    SessionId,
    ProviderSlug,
    ToolProfileSlug,
    TaskProfileSlug,
    ChannelId,
    TargetId,
    TargetKind,
    ModuleSlug,
    OauthAppId,
    Tab,
    Locale,
    Currency,
    PriceListId,
    ChannelSlug,
    Quantity,
    Page,
    Query,
}

impl AdminQueryKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProductId => "product_id",
            Self::CartId => "cart_id",
            Self::OrderId => "order_id",
            Self::CustomerId => "customer_id",
            Self::RegionId => "region_id",
            Self::ShippingProfileId => "shipping_profile_id",
            Self::ShippingOptionId => "shipping_option_id",
            Self::PostId => "post_id",
            Self::PageId => "page_id",
            Self::ThreadId => "thread_id",
            Self::MediaId => "media_id",
            Self::CategoryId => "category_id",
            Self::TopicId => "topic_id",
            Self::SessionId => "session_id",
            Self::ProviderSlug => "provider_slug",
            Self::ToolProfileSlug => "tool_profile_slug",
            Self::TaskProfileSlug => "task_profile_slug",
            Self::ChannelId => "channel_id",
            Self::TargetId => "target_id",
            Self::TargetKind => "target_kind",
            Self::ModuleSlug => "module_slug",
            Self::OauthAppId => "oauth_app_id",
            Self::Tab => "tab",
            Self::Locale => "locale",
            Self::Currency => "currency",
            Self::PriceListId => "price_list_id",
            Self::ChannelSlug => "channel_slug",
            Self::Quantity => "quantity",
            Self::Page => "page",
            Self::Query => "q",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "product_id" => Some(Self::ProductId),
            "cart_id" => Some(Self::CartId),
            "order_id" => Some(Self::OrderId),
            "customer_id" => Some(Self::CustomerId),
            "region_id" => Some(Self::RegionId),
            "shipping_profile_id" => Some(Self::ShippingProfileId),
            "shipping_option_id" => Some(Self::ShippingOptionId),
            "post_id" => Some(Self::PostId),
            "page_id" => Some(Self::PageId),
            "thread_id" => Some(Self::ThreadId),
            "media_id" => Some(Self::MediaId),
            "category_id" => Some(Self::CategoryId),
            "topic_id" => Some(Self::TopicId),
            "session_id" => Some(Self::SessionId),
            "provider_slug" => Some(Self::ProviderSlug),
            "tool_profile_slug" => Some(Self::ToolProfileSlug),
            "task_profile_slug" => Some(Self::TaskProfileSlug),
            "channel_id" => Some(Self::ChannelId),
            "target_id" => Some(Self::TargetId),
            "target_kind" => Some(Self::TargetKind),
            "module_slug" => Some(Self::ModuleSlug),
            "oauth_app_id" => Some(Self::OauthAppId),
            "tab" => Some(Self::Tab),
            "locale" => Some(Self::Locale),
            "currency" => Some(Self::Currency),
            "price_list_id" => Some(Self::PriceListId),
            "channel_slug" => Some(Self::ChannelSlug),
            "quantity" => Some(Self::Quantity),
            "page" => Some(Self::Page),
            "q" => Some(Self::Query),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminQueryDependency {
    pub parent: AdminQueryKey,
    pub child: AdminQueryKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminRouteQuerySchema {
    pub route_segment: &'static str,
    pub allowed_keys: &'static [AdminQueryKey],
    pub dependencies: &'static [AdminQueryDependency],
}

const EMPTY_DEPENDENCIES: &[AdminQueryDependency] = &[];
const CHANNEL_DEPENDENCIES: &[AdminQueryDependency] = &[
    AdminQueryDependency {
        parent: AdminQueryKey::ChannelId,
        child: AdminQueryKey::TargetId,
    },
    AdminQueryDependency {
        parent: AdminQueryKey::ChannelId,
        child: AdminQueryKey::ModuleSlug,
    },
    AdminQueryDependency {
        parent: AdminQueryKey::ChannelId,
        child: AdminQueryKey::OauthAppId,
    },
];

const PRODUCT_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::ProductId];
const PRICING_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::ProductId,
    AdminQueryKey::Currency,
    AdminQueryKey::RegionId,
    AdminQueryKey::PriceListId,
    AdminQueryKey::ChannelId,
    AdminQueryKey::ChannelSlug,
    AdminQueryKey::Quantity,
];
const INVENTORY_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::ProductId];
const ORDER_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::OrderId];
const CUSTOMER_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::CustomerId];
const REGION_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::RegionId];
const COMMERCE_ROUTE_KEYS: &[AdminQueryKey] =
    &[AdminQueryKey::ShippingProfileId, AdminQueryKey::CartId];
const FULFILLMENT_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::ShippingOptionId];
const BLOG_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::PostId];
const PAGES_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::PageId];
const COMMENTS_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::ThreadId,
    AdminQueryKey::Locale,
    AdminQueryKey::Page,
];
const MEDIA_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::MediaId,
    AdminQueryKey::Locale,
    AdminQueryKey::Page,
];
const FORUM_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::CategoryId,
    AdminQueryKey::TopicId,
    AdminQueryKey::Locale,
];
const AI_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::Tab,
    AdminQueryKey::SessionId,
    AdminQueryKey::ProviderSlug,
    AdminQueryKey::ToolProfileSlug,
    AdminQueryKey::TaskProfileSlug,
];
const CHANNEL_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::ChannelId,
    AdminQueryKey::TargetId,
    AdminQueryKey::ModuleSlug,
    AdminQueryKey::OauthAppId,
];
const SEO_ROUTE_KEYS: &[AdminQueryKey] = &[
    AdminQueryKey::TargetKind,
    AdminQueryKey::TargetId,
    AdminQueryKey::Locale,
    AdminQueryKey::Tab,
];
const SEARCH_ROUTE_KEYS: &[AdminQueryKey] = &[AdminQueryKey::Query, AdminQueryKey::Page];

const ROUTE_SCHEMAS: &[AdminRouteQuerySchema] = &[
    AdminRouteQuerySchema {
        route_segment: "product",
        allowed_keys: PRODUCT_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "pricing",
        allowed_keys: PRICING_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "inventory",
        allowed_keys: INVENTORY_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "orders",
        allowed_keys: ORDER_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "customers",
        allowed_keys: CUSTOMER_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "regions",
        allowed_keys: REGION_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "commerce",
        allowed_keys: COMMERCE_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "fulfillment",
        allowed_keys: FULFILLMENT_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "blog",
        allowed_keys: BLOG_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "pages",
        allowed_keys: PAGES_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "comments",
        allowed_keys: COMMENTS_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "media",
        allowed_keys: MEDIA_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "forum",
        allowed_keys: FORUM_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "ai",
        allowed_keys: AI_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "channels",
        allowed_keys: CHANNEL_ROUTE_KEYS,
        dependencies: CHANNEL_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "seo",
        allowed_keys: SEO_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
    AdminRouteQuerySchema {
        route_segment: "search",
        allowed_keys: SEARCH_ROUTE_KEYS,
        dependencies: EMPTY_DEPENDENCIES,
    },
];

const LEGACY_QUERY_KEYS: &[&str] = &["id", "pageId", "topicId", "module"];

pub fn admin_route_query_schema(
    route_segment: Option<&str>,
    _subpath: Option<&str>,
) -> Option<&'static AdminRouteQuerySchema> {
    let route_segment = route_segment?.trim();
    if route_segment.is_empty() {
        return None;
    }

    ROUTE_SCHEMAS
        .iter()
        .find(|schema| schema.route_segment == route_segment)
}

pub fn sanitize_admin_route_query(
    route_segment: Option<&str>,
    subpath: Option<&str>,
    query: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let schema = admin_route_query_schema(route_segment, subpath);
    let mut sanitized = BTreeMap::new();

    for (key, value) in query {
        let trimmed = value.trim();
        if trimmed.is_empty() || LEGACY_QUERY_KEYS.contains(&key.as_str()) {
            continue;
        }

        match (schema, AdminQueryKey::parse(key.as_str())) {
            (Some(schema), Some(parsed_key)) if !schema.allowed_keys.contains(&parsed_key) => {}
            _ => {
                sanitized.insert(key.clone(), trimmed.to_string());
            }
        }
    }

    if let Some(schema) = schema {
        for dependency in schema.dependencies {
            if !sanitized.contains_key(dependency.parent.as_str()) {
                sanitized.remove(dependency.child.as_str());
            }
        }
    }

    sanitized
}

pub fn is_legacy_admin_query_key(key: &str) -> bool {
    LEGACY_QUERY_KEYS.contains(&key)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{sanitize_admin_route_query, AdminQueryKey};

    fn query(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn product_route_drops_legacy_and_foreign_selection_keys() {
        let sanitized = sanitize_admin_route_query(
            Some("product"),
            None,
            &query(&[
                ("id", "legacy"),
                ("order_id", "ord_01"),
                ("product_id", "prod_01"),
                ("locale", "ru"),
            ]),
        );

        assert_eq!(
            sanitized
                .get(AdminQueryKey::ProductId.as_str())
                .map(String::as_str),
            Some("prod_01")
        );
        assert!(!sanitized.contains_key("id"));
        assert!(!sanitized.contains_key(AdminQueryKey::OrderId.as_str()));
        assert!(!sanitized.contains_key(AdminQueryKey::Locale.as_str()));
    }

    #[test]
    fn pricing_route_keeps_resolution_context_keys() {
        let sanitized = sanitize_admin_route_query(
            Some("pricing"),
            None,
            &query(&[
                ("product_id", "prod_01"),
                ("currency", "USD"),
                ("region_id", "reg_01"),
                ("price_list_id", "pl_01"),
                ("channel_id", "chn_01"),
                ("channel_slug", "retail"),
                ("quantity", "4"),
            ]),
        );

        for key in [
            AdminQueryKey::ProductId,
            AdminQueryKey::Currency,
            AdminQueryKey::RegionId,
            AdminQueryKey::PriceListId,
            AdminQueryKey::ChannelId,
            AdminQueryKey::ChannelSlug,
            AdminQueryKey::Quantity,
        ] {
            assert!(sanitized.contains_key(key.as_str()));
        }
    }

    #[test]
    fn commerce_route_keeps_shipping_profile_and_cart_selection_keys() {
        let sanitized = sanitize_admin_route_query(
            Some("commerce"),
            None,
            &query(&[
                ("shipping_profile_id", "sp_01"),
                ("cart_id", "cart_01"),
                ("order_id", "ord_01"),
            ]),
        );

        assert_eq!(
            sanitized
                .get(AdminQueryKey::ShippingProfileId.as_str())
                .map(String::as_str),
            Some("sp_01")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::CartId.as_str())
                .map(String::as_str),
            Some("cart_01")
        );
        assert!(!sanitized.contains_key(AdminQueryKey::OrderId.as_str()));
    }

    #[test]
    fn channel_route_requires_parent_channel_for_nested_keys() {
        let sanitized = sanitize_admin_route_query(
            Some("channels"),
            None,
            &query(&[
                ("target_id", "target_01"),
                ("module_slug", "blog"),
                ("oauth_app_id", "oauth_01"),
            ]),
        );

        assert!(sanitized.is_empty());
    }

    #[test]
    fn channel_route_keeps_nested_keys_when_parent_is_present() {
        let sanitized = sanitize_admin_route_query(
            Some("channels"),
            None,
            &query(&[
                ("channel_id", "ch_01"),
                ("target_id", "target_01"),
                ("module_slug", "blog"),
                ("oauth_app_id", "oauth_01"),
            ]),
        );

        assert_eq!(
            sanitized
                .get(AdminQueryKey::ChannelId.as_str())
                .map(String::as_str),
            Some("ch_01")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::TargetId.as_str())
                .map(String::as_str),
            Some("target_01")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::ModuleSlug.as_str())
                .map(String::as_str),
            Some("blog")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::OauthAppId.as_str())
                .map(String::as_str),
            Some("oauth_01")
        );
    }

    #[test]
    fn seo_route_keeps_typed_selection_keys_only() {
        let sanitized = sanitize_admin_route_query(
            Some("seo"),
            None,
            &query(&[
                ("target_kind", "product"),
                ("target_id", "550e8400-e29b-41d4-a716-446655440000"),
                ("locale", "en-US"),
                ("tab", "redirects"),
                ("product_id", "prod_01"),
                ("id", "legacy"),
            ]),
        );

        assert_eq!(
            sanitized
                .get(AdminQueryKey::TargetKind.as_str())
                .map(String::as_str),
            Some("product")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::TargetId.as_str())
                .map(String::as_str),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::Locale.as_str())
                .map(String::as_str),
            Some("en-US")
        );
        assert_eq!(
            sanitized
                .get(AdminQueryKey::Tab.as_str())
                .map(String::as_str),
            Some("redirects")
        );
        assert!(!sanitized.contains_key(AdminQueryKey::ProductId.as_str()));
        assert!(!sanitized.contains_key("id"));
    }
}
