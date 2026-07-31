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
            OR (
                jsonb_typeof({allowed_slugs}) = 'array'
                AND (
                    jsonb_array_length({allowed_slugs}) = 0
                    OR {channel_match}
                )
            )
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

        assert!(product_payload_visible_for_storefront(&payload, &channel(Some("web"))));
        assert!(product_payload_visible_for_storefront(&payload, &channel(None)));
    }

    #[test]
    fn restricted_product_requires_matching_normalized_slug() {
        let payload = serde_json::json!({
            "channel_visibility": { "allowed_channel_slugs": ["web"] }
        });

        assert!(product_payload_visible_for_storefront(&payload, &channel(Some(" Web "))));
        assert!(!product_payload_visible_for_storefront(&payload, &channel(Some("mobile"))));
        assert!(!product_payload_visible_for_storefront(&payload, &channel(None)));
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
    fn sql_scope_binds_only_a_present_channel_slug() {
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
        assert!(sql.contains("jsonb_array_length"));
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
        assert!(unscoped_sql.contains("OR FALSE"));
        assert!(unscoped_values.is_empty());
        assert_eq!(unscoped_next_param, 4);
    }
}
