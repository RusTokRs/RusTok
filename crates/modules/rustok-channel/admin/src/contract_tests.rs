use leptos::prelude::ServerFnError;
use serde_json::json;

use crate::core::reorder_policy_rule_ids;
use crate::model::{
    AvailableModuleItem, AvailableOauthAppItem, BindChannelModulePayload,
    BindChannelOauthAppPayload, ChannelAdminBootstrap, ChannelDetail, ChannelModuleBindingRecord,
    ChannelOauthAppRecord, ChannelRecord, ChannelResolutionActionRecord,
    ChannelResolutionPolicySetDetail, ChannelResolutionPolicySetRecord,
    ChannelResolutionPredicateRecord, ChannelResolutionRuleDefinitionRecord,
    ChannelResolutionRuleRecord, ChannelTargetRecord, CreateChannelPayload,
    CreateChannelTargetPayload, CreateResolutionPolicySetPayload, CreateResolutionRulePayload,
    ReorderResolutionRulesPayload, ResolvedChannelContext, UpdateResolutionRulePayload,
};
use crate::transport::ApiError;

#[test]
fn test_crate_api_compliance() {
    let api_doc = include_str!("../CRATE_API.md");
    assert!(api_doc.contains("## Platform Capability Bindings"));
    assert!(api_doc.contains("## Domain Collaborations"));
    assert!(api_doc.contains("## Minimum Contract Set"));
    assert!(api_doc.contains("### Input DTOs/Commands"));
    assert!(api_doc.contains("### Domain Invariants"));
    assert!(api_doc.contains("### Events / Outbox Side Effects"));
    assert!(api_doc.contains("### Errors / Failure Codes"));
}

#[test]
fn ffa_core_boundary_invariants() {
    let core_source = include_str!("core.rs");
    assert!(
        !core_source.contains("use leptos"),
        "core.rs must not depend on leptos"
    );
    assert!(
        !core_source.contains("use dioxus"),
        "core.rs must not depend on dioxus"
    );
    assert!(
        !core_source.contains("leptos::"),
        "core.rs must have zero leptos framework calls"
    );
}

#[test]
fn api_error_display_and_traits() {
    let rest_err = ApiError::Rest("connection refused".to_string());
    assert_eq!(rest_err.to_string(), "connection refused");
    assert_eq!(rest_err, ApiError::Rest("connection refused".to_string()));
    assert_ne!(rest_err, ApiError::Rest("timeout".to_string()));

    let srv_err = ApiError::ServerFn("unauthorized".to_string());
    assert_eq!(srv_err.to_string(), "unauthorized");
    assert_ne!(rest_err, srv_err);

    let from_server_fn: ApiError = ServerFnError::new("internal failure").into();
    assert_eq!(
        from_server_fn,
        ApiError::ServerFn("error running server function: internal failure".to_string())
    );

    let serialized = serde_json::to_string(&rest_err).expect("ApiError serializes");
    let deserialized: ApiError = serde_json::from_str(&serialized).expect("ApiError deserializes");
    assert_eq!(rest_err, deserialized);
}

#[test]
fn model_dto_serde_roundtrips() {
    let bootstrap = ChannelAdminBootstrap {
        current_channel: Some(ResolvedChannelContext {
            id: "chan-1".to_string(),
            tenant_id: "ten-1".to_string(),
            slug: "default".to_string(),
            name: "Default Channel".to_string(),
            is_active: true,
            status: "active".to_string(),
            target_type: Some("web_domain".to_string()),
            target_value: Some("example.com".to_string()),
            settings: json!({"theme": "dark"}),
            resolution_source: rustok_api::context::ChannelResolutionSource::Host,
            resolution_trace: vec![],
        }),
        channels: vec![ChannelDetail {
            channel: ChannelRecord {
                id: "chan-1".to_string(),
                tenant_id: "ten-1".to_string(),
                slug: "default".to_string(),
                name: "Default Channel".to_string(),
                is_active: true,
                is_default: true,
                status: "active".to_string(),
                settings: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            targets: vec![ChannelTargetRecord {
                id: "tgt-1".to_string(),
                channel_id: "chan-1".to_string(),
                target_type: "web_domain".to_string(),
                value: "example.com".to_string(),
                is_primary: true,
                settings: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            module_bindings: vec![ChannelModuleBindingRecord {
                id: "mod-1".to_string(),
                channel_id: "chan-1".to_string(),
                module_slug: "commerce".to_string(),
                is_enabled: true,
                settings: json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            oauth_apps: vec![ChannelOauthAppRecord {
                id: "app-1".to_string(),
                channel_id: "chan-1".to_string(),
                oauth_app_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
                role: Some("client".to_string()),
                created_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        }],
        policy_sets: vec![ChannelResolutionPolicySetDetail {
            policy_set: ChannelResolutionPolicySetRecord {
                id: "set-1".to_string(),
                tenant_id: "ten-1".to_string(),
                slug: "default-policy".to_string(),
                name: "Default Policy Set".to_string(),
                schema_version: 1,
                is_active: true,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            rules: vec![ChannelResolutionRuleRecord {
                id: "rule-1".to_string(),
                policy_set_id: "set-1".to_string(),
                priority: 10,
                is_active: true,
                action_channel_id: "chan-1".to_string(),
                definition: ChannelResolutionRuleDefinitionRecord {
                    predicates: vec![
                        ChannelResolutionPredicateRecord::HostEquals(
                            "shop.example.com".to_string(),
                        ),
                        ChannelResolutionPredicateRecord::HostSuffix(".example.com".to_string()),
                        ChannelResolutionPredicateRecord::OAuthAppEquals("app-1".to_string()),
                        ChannelResolutionPredicateRecord::SurfaceIs("http".to_string()),
                        ChannelResolutionPredicateRecord::LocaleEquals("en-US".to_string()),
                    ],
                    action: ChannelResolutionActionRecord::ResolveToChannel {
                        channel_id: "chan-1".to_string(),
                    },
                },
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        }],
        available_modules: vec![AvailableModuleItem {
            slug: "commerce".to_string(),
            name: "Commerce".to_string(),
            kind: "core".to_string(),
        }],
        oauth_apps: vec![AvailableOauthAppItem {
            id: "app-1".to_string(),
            name: "Storefront App".to_string(),
            slug: "storefront-app".to_string(),
            app_type: "spa".to_string(),
            is_active: true,
        }],
    };

    let serialized = serde_json::to_string(&bootstrap).expect("bootstrap serializes");
    let deserialized: ChannelAdminBootstrap =
        serde_json::from_str(&serialized).expect("bootstrap deserializes");
    assert_eq!(bootstrap, deserialized);
}

#[test]
fn payload_dto_serde_roundtrips() {
    let create_channel = CreateChannelPayload {
        tenant_id: Some("ten-1".to_string()),
        slug: "b2b".to_string(),
        name: "B2B Channel".to_string(),
        settings: Some(json!({"currency": "EUR"})),
    };
    let s = serde_json::to_string(&create_channel).unwrap();
    let d: CreateChannelPayload = serde_json::from_str(&s).unwrap();
    assert_eq!(create_channel, d);

    let target_payload = CreateChannelTargetPayload {
        target_type: "web_domain".to_string(),
        value: "b2b.example.com".to_string(),
        is_primary: true,
        settings: None,
    };
    let s = serde_json::to_string(&target_payload).unwrap();
    let d: CreateChannelTargetPayload = serde_json::from_str(&s).unwrap();
    assert_eq!(target_payload, d);

    let module_payload = BindChannelModulePayload {
        module_slug: "pricing".to_string(),
        is_enabled: true,
        settings: Some(json!({})),
    };
    let s = serde_json::to_string(&module_payload).unwrap();
    let d: BindChannelModulePayload = serde_json::from_str(&s).unwrap();
    assert_eq!(module_payload, d);

    let oauth_payload = BindChannelOauthAppPayload {
        oauth_app_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        role: Some("admin".to_string()),
    };
    let s = serde_json::to_string(&oauth_payload).unwrap();
    let d: BindChannelOauthAppPayload = serde_json::from_str(&s).unwrap();
    assert_eq!(oauth_payload, d);

    let policy_set_payload = CreateResolutionPolicySetPayload {
        slug: "mobile-policy".to_string(),
        name: "Mobile Policy".to_string(),
        is_active: true,
    };
    let s = serde_json::to_string(&policy_set_payload).unwrap();
    let d: CreateResolutionPolicySetPayload = serde_json::from_str(&s).unwrap();
    assert_eq!(policy_set_payload, d);

    let create_rule_payload = CreateResolutionRulePayload {
        priority: 15,
        is_active: true,
        action_channel_id: "chan-1".to_string(),
        host_equals: Some("m.example.com".to_string()),
        host_suffix: None,
        oauth_app_id: None,
        surface: Some("http".to_string()),
        locale: Some("de".to_string()),
    };
    let s = serde_json::to_string(&create_rule_payload).unwrap();
    let d: CreateResolutionRulePayload = serde_json::from_str(&s).unwrap();
    assert_eq!(create_rule_payload, d);

    let update_rule_payload = UpdateResolutionRulePayload {
        priority: Some(25),
        is_active: Some(false),
        action_channel_id: None,
        host_equals: None,
        host_suffix: Some(".mobile.com".to_string()),
        oauth_app_id: None,
        surface: None,
        locale: None,
    };
    let s = serde_json::to_string(&update_rule_payload).unwrap();
    let d: UpdateResolutionRulePayload = serde_json::from_str(&s).unwrap();
    assert_eq!(update_rule_payload, d);

    let reorder_payload = ReorderResolutionRulesPayload {
        rule_ids: vec!["r-1".to_string(), "r-2".to_string(), "r-3".to_string()],
    };
    let s = serde_json::to_string(&reorder_payload).unwrap();
    let d: ReorderResolutionRulesPayload = serde_json::from_str(&s).unwrap();
    assert_eq!(reorder_payload, d);
}

#[test]
fn reorder_policy_rule_ids_bounds_and_extremes() {
    let empty: Vec<String> = vec![];
    assert_eq!(reorder_policy_rule_ids(&empty, 0, true), None);
    assert_eq!(reorder_policy_rule_ids(&empty, 0, false), None);

    let single = vec!["rule-1".to_string()];
    assert_eq!(reorder_policy_rule_ids(&single, 0, true), None);
    assert_eq!(reorder_policy_rule_ids(&single, 0, false), None);
    assert_eq!(reorder_policy_rule_ids(&single, 5, true), None);
    assert_eq!(reorder_policy_rule_ids(&single, 5, false), None);

    let double = vec!["rule-1".to_string(), "rule-2".to_string()];
    assert_eq!(
        reorder_policy_rule_ids(&double, 0, false),
        Some(vec!["rule-2".to_string(), "rule-1".to_string()])
    );
    assert_eq!(
        reorder_policy_rule_ids(&double, 1, true),
        Some(vec!["rule-2".to_string(), "rule-1".to_string()])
    );
}
