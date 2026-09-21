use sea_orm::Value;
use serde_json::Value as JsonValue;

use crate::TrustedStorefrontChannel;

const PRODUCT_ALLOWED_CHANNEL_SLUGS_PATH: &str = "{channel_visibility,allowed_channel_slugs}";
const BLOG_ALLOWED_CHANNEL_SLUGS_PATH: &str = "{channel_slugs}";

pub(crate) fn storefront_channel_visibility_sql(
    entity_type_column: &str,
    payload_column: &str,
    channel: &TrustedStorefrontChannel,
    bound_values: &mut Vec<Value>,
    next_param: &mut usize,
) -> String {
    let product_allowed_slugs = format!("{payload_column} #> '{PRODUCT_ALLOWED_CHANNEL_SLUGS_PATH}'");
    let blog_allowed_slugs = format!("{payload_column} #> '{BLOG_ALLOWED_CHANNEL_SLUGS_PATH}'");
    let channel_placeholder = normalized_trusted_channel_slug(channel).map(|slug| {
        let placeholder = format!("${}", *next_param);
        bound_values.push(slug.into());
        *next_param += 1;
        placeholder
    });

    let product_channel_match = channel_placeholder
        .as_deref()
        .map(|placeholder| format!("({product_allowed_slugs}) ? {placeholder}"))
        .unwrap_or_else(|| "FALSE".to_string());
    let blog_channel_match = channel_placeholder
        .as_deref()
        .map(|placeholder| format!("({blog_allowed_slugs}) ? {placeholder}"))
        .unwrap_or_else(|| "FALSE".to_string());

    format!(
        "(CASE\n            WHEN {entity_type_column} = 'product' THEN\n                CASE\n                    WHEN jsonb_typeof({product_allowed_slugs}) IS DISTINCT FROM 'array' THEN FALSE\n                    WHEN jsonb_array_length({product_allowed_slugs}) = 0 THEN TRUE\n                    ELSE {product_channel_match}\n                END\n            WHEN {entity_type_column} = 'blog_post' THEN\n                CASE\n                    WHEN jsonb_typeof({blog_allowed_slugs}) IS DISTINCT FROM 'array' THEN FALSE\n                    WHEN jsonb_array_length({blog_allowed_slugs}) = 0 THEN TRUE\n                    ELSE {blog_channel_match}\n                END\n            ELSE TRUE\n        END)"
    )
}

pub(crate) fn storefront_payload_visible_for_channel(
    payload: &JsonValue,
    entity_type: &str,
    channel: &TrustedStorefrontChannel,
) -> bool {
    let Some(allowed_slugs) = match entity_type {
        "product" => payload
            .get("channel_visibility")
            .and_then(|value| value.get("allowed_channel_slugs"))
            .and_then(JsonValue::as_array),
        "blog_post" => payload.get("channel_slugs").and_then(JsonValue::as_array),
        _ => return true,
    } else {
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

#[cfg(test)]
mod tests {
    use sea_orm::Value;
    use uuid::Uuid;

    use super::{storefront_channel_visibility_sql, storefront_payload_visible_for_channel};
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

        assert!(storefront_payload_visible_for_channel(
            &payload,
            "product",
            &channel(Some("web"))
        ));
        assert!(storefront_payload_visible_for_channel(
            &payload,
            "product",
            &channel(None)
        ));
    }

    #[test]
    fn restricted_product_requires_matching_normalized_slug() {
        let payload = serde_json::json!({
            "channel_visibility": { "allowed_channel_slugs": ["web"] }
        });

        assert!(storefront_payload_visible_for_channel(
            &payload,
            "product",
            &channel(Some(" Web "))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &payload,
            "product",
            &channel(Some("mobile"))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &payload,
            "product",
            &channel(None)
        ));
    }

    #[test]
    fn missing_or_malformed_projection_fails_closed() {
        assert!(!storefront_payload_visible_for_channel(
            &serde_json::json!({}),
            "product",
            &channel(Some("web"))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &serde_json::json!({
                "channel_visibility": { "allowed_channel_slugs": "web" }
            }),
            "product",
            &channel(Some("web"))
        ));
    }

    #[test]
    fn sql_scope_guards_array_length_with_case() {
        let mut values = Vec::<Value>::new();
        let mut next_param = 4;
        let sql = storefront_channel_visibility_sql(
            "entity_type",
            "payload",
            &channel(Some("Web")),
            &mut values,
            &mut next_param,
        );

        assert!(sql.contains("entity_type = 'product'"));
        assert!(sql.contains("entity_type = 'blog_post'"));
        assert!(sql.contains("IS DISTINCT FROM 'array' THEN FALSE"));
        assert!(sql.contains("WHEN jsonb_array_length"));
        assert!(sql.contains("? $4"));
        assert_eq!(values.len(), 1);
        assert_eq!(next_param, 5);

        let mut unscoped_values = Vec::<Value>::new();
        let mut unscoped_next_param = 4;
        let unscoped_sql = storefront_channel_visibility_sql(
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
    fn blog_post_visibility_uses_channel_slugs_and_fails_closed() {
        let visible = serde_json::json!({
            "channel_slugs": ["web"]
        });
        assert!(storefront_payload_visible_for_channel(
            &visible,
            "blog_post",
            &channel(Some("WEB"))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &visible,
            "blog_post",
            &channel(Some("mobile"))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &serde_json::json!({}),
            "blog_post",
            &channel(Some("web"))
        ));
        assert!(!storefront_payload_visible_for_channel(
            &serde_json::json!({"channel_slugs": "web"}),
            "blog_post",
            &channel(Some("web"))
        ));
    }

    #[test]
    fn unrelated_documents_remain_visible() {
        assert!(storefront_payload_visible_for_channel(
            &serde_json::json!({}),
            "forum_topic",
            &channel(Some("web"))
        ));
    }
}
