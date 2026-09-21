use sea_orm::Value;
use serde_json::Value as JsonValue;

use crate::TrustedStorefrontChannel;

const PRODUCT_ALLOWED_CHANNEL_SLUGS_PATH: &str = "{channel_visibility,allowed_channel_slugs}";

pub(crate) fn product_channel_visibility_sql(
    entity_type_column: &str,
    payload_column: &str,
    channel: &TrustedStorefrontChannel,
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> String {
    let allowed_slugs = format!("{payload_column} #> '{PRODUCT_ALLOWED_CHANNEL_SLUGS_PATH}'");
    let channel_match = normalized_trusted_channel_slug(channel)
        .map(|slug| {
            let placeholder = format!("${}", *next_param);
            bound_values.push(slug.into());
            *next_param += 1;
            format!("({allowed_slugs}) ? {placeholder}")
        })
        .unwrap_or_else(|| "FALSE".to_string());

    format!(
        "(
            {entity_type_column} <> 'product'
            OR CASE
                WHEN jsonb_typeof({allowed_slugs}) IS DISTINCT FROM 'array' THEN FALSE
                WHEN jsonb_array_length({allowed_slugs}) = 0 THEN TRUE
                ELSE {channel_match}
            END
        )"
    )
}

pub(crate) fn product_payload_visible_for_storefront(
    payload: &JsonValue,
    channel: &TrustedStorefrontChannel,
) -> bool {
    let Some(allowed_slugs) = payload
        .get("channel_visibility")
        .and_then(|value| value.get("allowed_channel_slugs"))
        .and_then(JsonValue::as_array)
    else {
        return false;
    };

    if allowed_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = normalized_trusted_channel_slug(channel) else {
        return false;
    };

    allowed_slugs.iter().any(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some_and(|value| value.eq_ignore_ascii_case(&channel_slug))
    })
}

fn normalized_trusted_channel_slug(channel: &TrustedStorefrontChannel) -> Option<String> {
    channel
        .channel_slug
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}


pub(crate) fn blog_channel_visibility_sql(
    entity_type_column: &str,
    source_module_column: &str,
    payload_column: &str,
    channel: &TrustedStorefrontChannel,
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> String {
    let channel_slugs = format!("{payload_column} -> 'channel_slugs'");
    let channel_match = normalized_trusted_channel_slug(channel)
        .map(|slug| {
            let placeholder = format("${}", *next_param);
            bound_values.push(slug.into());
            *next_param += 1;
            format!("({channel_slugs}) ? {placeholder}")
        })
        .unwrap_or_else(|| "FALSE".to_string());

    format!(
        "(
            NOT ({entity_type_column} = 'blog_post' AND {source_module_column} = 'blog')
            OR CASE
                WHEN jsonb_typeof({channel_slugs}) IS DISTINCT FROM 'array' THEN FALSE
                WHEN jsonb_array_length({channel_slugs}) = 0 THEN TRUE
                ELSE {channel_match}
            END
        )"
    )
}

pub(crate) fn blog_payload_visible_for_storefront(
    payload: &JsonValue,
    channel: &TrustedStorefrontChannel,
) -> bool {
    let Some(channel_slugs) = payload
        .get("channel_slugs")
        .and_then(JsonValue::as_array)
    else {
        return false;
    };

    if channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = normalized_trusted_channel_slug(channel) else {
        return false;
    };

    channel_slugs.iter().any(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some_and(|value| value.eq_ignore_ascii_case(&channel_slug))
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::Value;
    use uuid::Uuid;

    use super::{product_channel_visibility_sql, product_payload_visible_for_storefront};
    use crate::TrustedStorefrontChannel;

    fn channel(slug: Option<&str>) -> TrustedStorefrontChannel {
        TrustedStorefrontChannel {
            channel_id: slug.map(|_| Uuid::new_v4()),
            channel_slug: slug.map(ToOwned::to_owned),
        }
    }

    #[test]
    fn empty_allowlist_is_visible_in_scoped_and_unscoped_storefronts() {
        let payload = serde_json::json!({
            "channel_visibility": { "allowed_channel_slugs": [] }
        });

        assert!(product_payload_visible_for_storefront(
            &payload,
            &channel(Some("web"))
        ));
        assert!(product_payload_visible_for_storefront(
            &payload,
            &channel(None)
        ));
    }

    #[test]
    fn restricted_product_requires_matching_normalized_slug() {
        let payload = serde_json::json!({
            "channel_visibility": { "allowed_channel_slugs": ["web"] }
        });

        assert!(product_payload_visible_for_storefront(
            &payload,
            &channel(Some(" Web "))
        ));
        assert!(!product_payload_visible_for_storefront(
            &payload,
            &channel(Some("mobile"))
        ));
        assert!(!product_payload_visible_for_storefront(
            &payload,
            &channel(None)
        ));
    }

    #[test]
    fn missing_or_malformed_projection_fails_closed() {
        assert!(!product_payload_visible_for_storefront(
            &serde_json::json!({}),
            &channel(Some("web"))
        ));
        assert!(!product_payload_visible_for_storefront(
            &serde_json::json!({
                "channel_visibility": { "allowed_channel_slugs": "web" }
            }),
            &channel(Some("web"))
        ));
    }

    #[test]
    fn sql_scope_guards_array_length_with_case() {
        let mut values = Vec::<Value>::new();
        let mut next_param = 4;
        let sql = product_channel_visibility_sql(
            "entity_type",
            "payload",
            &channel(Some("Web")),
            &mut values,
            &mut next_param,
        );

        assert!(sql.contains("entity_type <> 'product'"));
        assert!(sql.contains("OR CASE"));
        assert!(sql.contains("IS DISTINCT FROM 'array' THEN FALSE"));
        assert!(sql.contains("WHEN jsonb_array_length"));
        assert!(sql.contains("? $4"));
        assert_eq!(values.len(), 1);
        assert_eq!(next_param, 5);

        let mut unscoped_values = Vec::<Value>::new();
        let mut unscoped_next_param = 4;
        let unscoped_sql = product_channel_visibility_sql(
            "entity_type",
            "payload",
            &channel(None),
            &mut unscoped_values,
            &mut unscoped_next_param,
        );
        assert!(unscoped_sql.contains("ELSE FALSE"));
        assert!(unscoped_values.is_empty());
        assert_eq!(unscoped_next_param, 4);
    }

    #[test]
    fn blog_projection_visibility_requires_matching_channel_for_restricted_posts() {
        let restricted = serde_json::json!({ "channel_slugs": ["mobile"] });
        let unrestricted = serde_json::json!({ "channel_slugs": [] });

        assert!(blog_payload_visible_for_storefront(&restricted, &channel(Some("Mobile"))));
        assert!(!blog_payload_visible_for_storefront(&restricted, &channel(Some("web"))));
        assert!(blog_payload_visible_for_storefront(&unrestricted, &channel(Some("web"))));
        assert!(!blog_payload_visible_for_storefront(&restricted, &channel(None)));
    }

    #[test]
    fn malformed_blog_projection_fails_closed() {
        assert!(!blog_payload_visible_for_storefront(
            &serde_json::json!({}),
            &channel(Some("web"))
        ));
        assert!(!blog_payload_visible_for_storefront(
            &serde_json::json!({ "channel_slugs": "web" }),
            &channel(Some("web"))
        ));
    }

    #[test]
    fn blog_sql_scope_is_limited_to_blog_documents() {
        let mut values = Vec::<Value>::new();
        let mut next_param = 4;
        let sql = blog_channel_visibility_sql(
            "entity_type",
            "source_module",
            "payload",
            &channel(Some("Web")),
            &mut values,
            &mut next_param,
        );

        assert!(sql.contains("NOT (entity_type = 'blog_post' AND source_module = 'blog')"));
        assert!(sql.contains("jsonb_typeof(payload -> 'channel_slugs')"));
        assert!(sql.contains(
            "WHEN jsonb_array_length(payload -> 'channel_slugs') = 0 THEN TRUE"
        ));
        assert!(sql.contains("? $4"));
        assert_eq!(values.len(), 1);
        assert_eq!(next_param, 5);
    }

}
