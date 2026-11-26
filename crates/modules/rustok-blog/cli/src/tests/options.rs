use rustok_cli_core::CommandRequest;
use uuid::Uuid;

use crate::commands::options::{parse_limit, parse_optional_tenant_id};
use crate::parse_publish_scheduled_options;

#[test]
fn parse_limit_handles_defaults_strings_and_numbers() {
    let empty = serde_json::Map::new();
    assert_eq!(parse_limit(&empty, 100, 200).unwrap(), 100);

    let mut string_opt = serde_json::Map::new();
    string_opt.insert("limit".to_string(), serde_json::json!("25"));
    assert_eq!(parse_limit(&string_opt, 100, 200).unwrap(), 25);

    let mut ws_opt = serde_json::Map::new();
    ws_opt.insert("limit".to_string(), serde_json::json!("  50  \n"));
    assert_eq!(parse_limit(&ws_opt, 100, 200).unwrap(), 50);

    let mut num_opt = serde_json::Map::new();
    num_opt.insert("limit".to_string(), serde_json::json!(150));
    assert_eq!(parse_limit(&num_opt, 100, 200).unwrap(), 150);
}

#[test]
fn parse_limit_rejects_invalid_values() {
    let mut zero_str = serde_json::Map::new();
    zero_str.insert("limit".to_string(), serde_json::json!("0"));
    assert!(parse_limit(&zero_str, 100, 200).is_err());

    let mut zero_num = serde_json::Map::new();
    zero_num.insert("limit".to_string(), serde_json::json!(0));
    assert!(parse_limit(&zero_num, 100, 200).is_err());

    let mut neg_num = serde_json::Map::new();
    neg_num.insert("limit".to_string(), serde_json::json!(-5));
    assert!(parse_limit(&neg_num, 100, 200).is_err());

    let mut non_num = serde_json::Map::new();
    non_num.insert("limit".to_string(), serde_json::json!("not_a_number"));
    assert!(parse_limit(&non_num, 100, 200).is_err());

    let mut bool_val = serde_json::Map::new();
    bool_val.insert("limit".to_string(), serde_json::json!(true));
    assert!(parse_limit(&bool_val, 100, 200).is_err());

    let mut exceeded = serde_json::Map::new();
    exceeded.insert("limit".to_string(), serde_json::json!(500));
    let err = parse_limit(&exceeded, 100, 200).unwrap_err();
    assert!(err.to_string().contains("cannot exceed 200"));
}

#[test]
fn parse_optional_tenant_id_parses_and_validates() {
    let empty = serde_json::Map::new();
    assert_eq!(parse_optional_tenant_id(&empty).unwrap(), None);

    let valid_uuid = "11111111-2222-3333-4444-555555555555";
    let mut snake_opt = serde_json::Map::new();
    snake_opt.insert("tenant_id".to_string(), serde_json::json!(valid_uuid));
    assert_eq!(
        parse_optional_tenant_id(&snake_opt).unwrap(),
        Some(Uuid::parse_str(valid_uuid).unwrap())
    );

    let mut kebab_opt = serde_json::Map::new();
    kebab_opt.insert(
        "tenant-id".to_string(),
        serde_json::json!(format!("  {valid_uuid}  ")),
    );
    assert_eq!(
        parse_optional_tenant_id(&kebab_opt).unwrap(),
        Some(Uuid::parse_str(valid_uuid).unwrap())
    );

    let mut empty_str = serde_json::Map::new();
    empty_str.insert("tenant_id".to_string(), serde_json::json!("   "));
    assert!(parse_optional_tenant_id(&empty_str).is_err());

    let mut invalid_str = serde_json::Map::new();
    invalid_str.insert("tenant_id".to_string(), serde_json::json!("not-a-uuid"));
    assert!(parse_optional_tenant_id(&invalid_str).is_err());

    let mut nil_str = serde_json::Map::new();
    nil_str.insert(
        "tenant_id".to_string(),
        serde_json::json!(Uuid::nil().to_string()),
    );
    assert!(parse_optional_tenant_id(&nil_str).is_err());
}

#[test]
fn request_options_extraction() {
    let request = CommandRequest {
        namespace: "blog".to_string(),
        name: "publish-scheduled".to_string(),
        args: serde_json::json!({
            "options": {
                "tenant_id": "11111111-2222-3333-4444-555555555555",
                "limit": "42"
            }
        }),
        dry_run: true,
    };

    let opts = parse_publish_scheduled_options(&request).unwrap();
    assert_eq!(
        opts.tenant_id,
        Some(Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap())
    );
    assert_eq!(opts.limit, 42);
}
