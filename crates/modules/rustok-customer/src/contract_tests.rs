use chrono::Utc;
use rustok_api::Permission;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm::DbErr;
use uuid::Uuid;

use crate::dto::{CustomerResponse, CustomerWithProfileResponse};
use crate::error::CustomerError;
use crate::ports::{
    CustomerListProjectionRequest, CustomerListProjectionResponse, CustomerProfileEnrichment,
    CustomerProfileEnrichmentRequest, CustomerProjectionRequest, CustomerUserProjectionRequest,
};
use crate::{CustomerModule, CustomerService, InProcessCustomerReadPort};

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public Modules",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[tokio::test]
async fn module_contract_metadata_matches() {
    let module = CustomerModule::new();
    assert_eq!(module.slug(), "customer");
    assert_eq!(module.name(), "Customer");
    assert_eq!(
        module.description(),
        "Default storefront customer submodule in the ecommerce family"
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(module.kind(), ModuleKind::Optional);
    assert_eq!(module.health().await, HealthStatus::Healthy);
    assert!(module.dependencies().is_empty());
    assert_eq!(module.permissions().len(), 6);
    assert!(!module.migrations().is_empty());
}

#[test]
fn module_permissions_cover_customer_domain() {
    let module = CustomerModule::new();
    let perms = module.permissions();
    for expected in [
        Permission::CUSTOMERS_CREATE,
        Permission::CUSTOMERS_READ,
        Permission::CUSTOMERS_UPDATE,
        Permission::CUSTOMERS_DELETE,
        Permission::CUSTOMERS_LIST,
        Permission::CUSTOMERS_MANAGE,
    ] {
        assert!(
            perms.contains(&expected),
            "permissions must include {expected:?}"
        );
    }
}

#[test]
fn customer_error_code_mapping_and_display() {
    let err_val = CustomerError::validation("email is empty");
    assert_eq!(err_val.code(), "customer.validation_failed");
    assert!(err_val.to_string().contains("email is empty"));

    let cid = Uuid::new_v4();
    let err_not_found = CustomerError::CustomerNotFound(cid);
    assert_eq!(err_not_found.code(), "customer.not_found");
    assert!(err_not_found.to_string().contains(&cid.to_string()));

    let uid = Uuid::new_v4();
    let err_user_not_found = CustomerError::CustomerByUserNotFound(uid);
    assert_eq!(err_user_not_found.code(), "customer.user_not_found");
    assert!(err_user_not_found.to_string().contains(&uid.to_string()));

    let err_dup_email = CustomerError::DuplicateEmail("dup@example.com".to_string());
    assert_eq!(err_dup_email.code(), "customer.duplicate_email");
    assert!(err_dup_email.to_string().contains("dup@example.com"));

    let err_dup_user = CustomerError::DuplicateUserLink(uid);
    assert_eq!(err_dup_user.code(), "customer.duplicate_user_link");
    assert!(err_dup_user.to_string().contains(&uid.to_string()));

    let err_db = CustomerError::Database(DbErr::Custom("db down".to_string()));
    assert_eq!(err_db.code(), "customer.database_error");
    assert!(err_db.to_string().contains("db down"));
}

#[test]
fn customer_dto_partial_eq_and_debug() {
    let now = Utc::now();
    let id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let c1 = CustomerResponse {
        id,
        tenant_id,
        user_id: None,
        email: "c@example.com".to_string(),
        first_name: Some("Ada".to_string()),
        last_name: Some("Lovelace".to_string()),
        phone: None,
        locale: Some("en".to_string()),
        metadata: serde_json::json!({ "tag": "test" }),
        created_at: now,
        updated_at: now,
    };
    let c2 = c1.clone();
    assert_eq!(c1, c2);

    let dbg = format!("{c1:?}");
    assert!(dbg.contains("CustomerResponse"));
    assert!(dbg.contains("c@example.com"));

    let with_profile1 = CustomerWithProfileResponse {
        customer: c1.clone(),
        profile: None,
    };
    let with_profile2 = with_profile1.clone();
    assert_eq!(with_profile1, with_profile2);

    let list_resp = CustomerListProjectionResponse {
        items: vec![c1],
        total: 1,
    };
    assert_eq!(list_resp.items.len(), 1);
    assert_eq!(list_resp.total, 1);
    let list_resp2 = list_resp.clone();
    assert_eq!(list_resp, list_resp2);
}

#[test]
fn port_requests_serde_roundtrip() {
    let cid = Uuid::new_v4();
    let req = CustomerProjectionRequest { customer_id: cid };
    let json = serde_json::to_string(&req).unwrap();
    let deserialized: CustomerProjectionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(req, deserialized);

    let uid = Uuid::new_v4();
    let user_req = CustomerUserProjectionRequest { user_id: uid };
    let json = serde_json::to_string(&user_req).unwrap();
    let deserialized: CustomerUserProjectionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(user_req, deserialized);

    let list_req = CustomerListProjectionRequest {
        search: Some("alpha".to_string()),
        page: 2,
        per_page: 25,
    };
    let json = serde_json::to_string(&list_req).unwrap();
    let deserialized: CustomerListProjectionRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(list_req, deserialized);

    let enrich_req = CustomerProfileEnrichmentRequest {
        user_ids: vec![uid],
    };
    let json = serde_json::to_string(&enrich_req).unwrap();
    let deserialized: CustomerProfileEnrichmentRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(enrich_req, deserialized);

    let enrichment = CustomerProfileEnrichment {
        user_id: uid,
        first_name: Some("Ada".to_string()),
        last_name: Some("Lovelace".to_string()),
        preferred_locale: Some("en".to_string()),
    };
    let json = serde_json::to_string(&enrichment).unwrap();
    let deserialized: CustomerProfileEnrichment = serde_json::from_str(&json).unwrap();
    assert_eq!(enrichment, deserialized);
}

#[tokio::test]
async fn customer_service_and_in_process_port_derive_debug_and_clone() {
    let db = rustok_test_utils::db::setup_test_db().await;
    let service = CustomerService::new(db.clone());
    let service_clone = service.clone();
    let service_debug = format!("{service:?}");
    assert!(service_debug.contains("CustomerService"));
    drop(service_clone);

    let port = InProcessCustomerReadPort::new(db);
    let port_clone = port.clone();
    let port_debug = format!("{port:?}");
    assert!(port_debug.contains("InProcessCustomerReadPort"));
    drop(port_clone);
}

