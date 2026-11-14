use serde_json::Value;
use uuid::Uuid;

pub(crate) fn channel_tax_provider_id(
    metadata: &Value,
    channel_id: Option<Uuid>,
) -> Option<String> {
    let channel_id = channel_id?;
    if channel_id.is_nil() {
        return None;
    }
    let channel_key = channel_id.to_string();
    let value = metadata
        .get("channel_tax_provider_ids")
        .and_then(Value::as_object)
        .and_then(|mapping| mapping.get(&channel_key))?;

    match value {
        Value::String(value) => Some(value.as_str()),
        Value::Object(value) => value
            .get("provider_id")
            .and_then(Value::as_str)
            .or_else(|| value.get("provider").and_then(Value::as_str)),
        _ => None,
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn channel_tax_provider_id_reads_string_mapping_for_channel() {
        let channel_id = Uuid::new_v4();
        let metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "provider_alpha"
            }
        });

        let resolved = channel_tax_provider_id(&metadata, Some(channel_id));
        assert_eq!(resolved.as_deref(), Some("provider_alpha"));
    }

    #[test]
    fn channel_tax_provider_id_ignores_blank_or_malformed_values() {
        let channel_id = Uuid::new_v4();

        let blank_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "   "
            }
        });
        assert_eq!(
            channel_tax_provider_id(&blank_metadata, Some(channel_id)),
            None
        );

        let typed_legacy_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&typed_legacy_metadata, Some(channel_id)).as_deref(),
            Some("external_tax")
        );

        let malformed_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"unknown_key": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&malformed_metadata, Some(channel_id)),
            None
        );

        let typed_object_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider_id": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&typed_object_metadata, Some(channel_id)).as_deref(),
            Some("external_tax")
        );
    }

    #[test]
    fn channel_tax_provider_id_prefers_provider_id_over_provider_alias() {
        let channel_id = Uuid::new_v4();
        let metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {
                    "provider_id": "region_default",
                    "provider": "external_tax"
                }
            }
        });

        assert_eq!(
            channel_tax_provider_id(&metadata, Some(channel_id)).as_deref(),
            Some("region_default")
        );
    }

    #[test]
    fn channel_tax_provider_id_returns_none_without_channel_context() {
        let metadata = json!({
            "channel_tax_provider_ids": {
                Uuid::new_v4().to_string(): "provider_alpha"
            }
        });

        assert_eq!(channel_tax_provider_id(&metadata, None), None);
    }

    #[test]
    fn channel_tax_provider_id_returns_none_for_nil_channel_id() {
        let metadata = json!({
            "channel_tax_provider_ids": {
                Uuid::nil().to_string(): "provider_nil"
            }
        });

        assert_eq!(channel_tax_provider_id(&metadata, Some(Uuid::nil())), None);
    }
}
