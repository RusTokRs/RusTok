use rustok_core::types::{UserRole, UserStatus};
use rustok_core::{generate_id, parse_id};
use std::str::FromStr;

// ---------------------------------------------------------------------------
// UserRole
// ---------------------------------------------------------------------------

#[test]
fn user_role_display_roundtrip() {
    assert_eq!(UserRole::SuperAdmin.to_string(), "super_admin");
    assert_eq!(UserRole::Admin.to_string(), "admin");
    assert_eq!(UserRole::Manager.to_string(), "manager");
    assert_eq!(UserRole::Customer.to_string(), "customer");
}

#[test]
fn user_role_parse_roundtrip() {
    assert_eq!(UserRole::from_str("super_admin").unwrap(), UserRole::SuperAdmin);
    assert_eq!(UserRole::from_str("admin").unwrap(), UserRole::Admin);
    assert_eq!(UserRole::from_str("manager").unwrap(), UserRole::Manager);
    assert_eq!(UserRole::from_str("customer").unwrap(), UserRole::Customer);
}

#[test]
fn user_role_parse_rejects_invalid() {
    assert!(UserRole::from_str("unknown").is_err());
    assert!(UserRole::from_str("").is_err());
}

#[test]
fn user_role_default_is_customer() {
    assert_eq!(UserRole::default(), UserRole::Customer);
}

#[test]
fn user_role_serde_roundtrip() {
    let role = UserRole::Admin;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"admin\"");
    let decoded: UserRole = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, role);
}

// ---------------------------------------------------------------------------
// UserStatus
// ---------------------------------------------------------------------------

#[test]
fn user_status_display_roundtrip() {
    assert_eq!(UserStatus::Active.to_string(), "active");
    assert_eq!(UserStatus::Inactive.to_string(), "inactive");
    assert_eq!(UserStatus::Banned.to_string(), "banned");
}

#[test]
fn user_status_default_is_active() {
    assert_eq!(UserStatus::default(), UserStatus::Active);
}

#[test]
fn user_status_serde_roundtrip() {
    let status = UserStatus::Inactive;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"inactive\"");
    let decoded: UserStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, status);
}

// ---------------------------------------------------------------------------
// ID generation
// ---------------------------------------------------------------------------

#[test]
fn generate_id_produces_valid_uuid() {
    let id = generate_id();
    assert_eq!(id.get_version_num(), 4);
}

#[test]
fn parse_id_accepts_uuid_string() {
    let raw = "550e8400-e29b-41d4-a716-446655440000";
    let id = parse_id(raw).unwrap();
    assert_eq!(id.to_string(), raw);
}

#[test]
fn parse_id_rejects_invalid() {
    assert!(parse_id("not-an-id").is_err());
    assert!(parse_id("").is_err());
}

// ---------------------------------------------------------------------------
// Locale normalization
// ---------------------------------------------------------------------------

#[test]
fn normalize_locale_tag_basic_cases() {
    use rustok_core::locale::normalize_locale_tag;

    assert_eq!(normalize_locale_tag("en"), Some("en".to_string()));
    assert_eq!(normalize_locale_tag("ru-RU"), Some("ru-RU".to_string()));
    assert_eq!(normalize_locale_tag("en_US"), Some("en-US".to_string()));
}

#[test]
fn normalize_locale_tag_rejects_invalid() {
    use rustok_core::locale::normalize_locale_tag;

    assert_eq!(normalize_locale_tag(""), None);
    assert_eq!(normalize_locale_tag("!!!"), None);
    assert_eq!(normalize_locale_tag(&"x".repeat(33)), None);
}

// ---------------------------------------------------------------------------
// Field schema guardrails
// ---------------------------------------------------------------------------

#[test]
fn is_valid_field_key_accepts_snake_case() {
    use rustok_core::field_schema::is_valid_field_key;

    assert!(is_valid_field_key("phone"));
    assert!(is_valid_field_key("first_name"));
    assert!(is_valid_field_key("a"));
}

#[test]
fn is_valid_field_key_rejects_invalid() {
    use rustok_core::field_schema::is_valid_field_key;

    assert!(!is_valid_field_key(""));
    assert!(!is_valid_field_key("1phone"));
    assert!(!is_valid_field_key("Phone"));
    assert!(!is_valid_field_key("phone-number"));
}
