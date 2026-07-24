use super::submission_aggregation::{
    SITEMAP_SUBMIT_MAX_ERRORS, SITEMAP_SUBMIT_MAX_ERROR_LEN, SITEMAP_SUBMIT_MAX_TIMEOUT_DETAILS,
};
use super::{
    normalize_sitemap_submission_endpoints, record_invalid_endpoint, record_submission_failure,
    record_submission_success, render_robots_body, resolve_public_origin_from_values,
    sitemap_event_key, sitemap_file_count, SitemapSubmissionAdapter, SitemapSubmissionSummary,
    SitemapSubmitEndpoint,
};
use crate::services::SeoService;
use rustok_api::TenantContext;
use rustok_tenant::entities::tenant_module;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, DbBackend, Statement,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

struct TestSitemapSubmissionAdapter {
    outcomes: Arc<Mutex<HashMap<String, Result<(), String>>>>,
    submitted_endpoints: Arc<Mutex<Vec<String>>>,
    submitted_request_urls: Arc<Mutex<Vec<String>>>,
}

impl TestSitemapSubmissionAdapter {
    fn new(outcomes: HashMap<String, Result<(), String>>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(outcomes)),
            submitted_endpoints: Arc::new(Mutex::new(Vec::new())),
            submitted_request_urls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn submitted_endpoints(&self) -> Vec<String> {
        self.submitted_endpoints.lock().await.clone()
    }

    async fn submitted_request_urls(&self) -> Vec<String> {
        self.submitted_request_urls.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl SitemapSubmissionAdapter for TestSitemapSubmissionAdapter {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String> {
        self.submitted_endpoints
            .lock()
            .await
            .push(endpoint.endpoint.clone());
        self.submitted_request_urls
            .lock()
            .await
            .push(endpoint.request_url.clone());
        let outcomes = self.outcomes.lock().await;
        outcomes
            .get(endpoint.endpoint.as_str())
            .cloned()
            .unwrap_or(Ok(()))
    }
}

async fn test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:seo_service_sitemaps_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(opts)
        .await
        .expect("failed to connect seo sqlite db")
}

async fn seed_tenant_modules_table(db: &DatabaseConnection) {
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"
        .to_string(),
    ))
    .await
    .expect("create tenant_modules table");
}

async fn insert_seo_settings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    settings: serde_json::Value,
) {
    let now = chrono::Utc::now();
    tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        module_slug: Set("seo".to_string()),
        enabled: Set(true),
        settings: Set(settings),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("insert seo module settings");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Tenant".to_string(),
        slug: "tenant".to_string(),
        domain: Some("store.example.com".to_string()),
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

#[test]
fn render_robots_body_omits_sitemap_when_disabled() {
    assert_eq!(
        render_robots_body("https://example.com", false),
        "User-agent: *\nAllow: /\n"
    );
}

#[test]
fn render_robots_body_includes_sitemap_when_enabled() {
    assert_eq!(
        render_robots_body("https://example.com", true),
        "User-agent: *\nAllow: /\nSitemap: https://example.com/sitemap.xml\n"
    );
}

#[test]
fn public_origin_prefers_tenant_domain_and_defaults_to_https() {
    let origin = resolve_public_origin_from_values(
        Some(" Store.Example.com. "),
        Some("https://fallback.example.com"),
        Some("https://api.example.com"),
    )
    .expect("tenant domain should resolve");

    assert_eq!(origin.as_str(), "https://store.example.com");
}

#[test]
fn public_origin_uses_environment_fallback_order() {
    let public_origin = resolve_public_origin_from_values(
        None,
        Some("https://public.example.com/"),
        Some("https://api.example.com"),
    )
    .expect("public URL should resolve");
    assert_eq!(public_origin.as_str(), "https://public.example.com");

    let api_origin = resolve_public_origin_from_values(None, Some("   "), Some("api.example.com"))
        .expect("API URL should resolve");
    assert_eq!(api_origin.as_str(), "https://api.example.com");
}

#[test]
fn public_origin_requires_explicit_configuration() {
    let error = resolve_public_origin_from_values(None, None, None)
        .expect_err("missing public origin must fail closed");

    assert!(error
        .to_string()
        .contains("SEO public origin is not configured"));
    assert!(error.to_string().contains("RUSTOK_PUBLIC_URL"));
}

#[test]
fn public_origin_rejects_non_origin_url_components() {
    for value in [
        "https://user:secret@example.com",
        "https://example.com/store",
        "https://example.com?tenant=1",
        "https://example.com#fragment",
    ] {
        assert!(
            resolve_public_origin_from_values(Some(value), None, None).is_err(),
            "{value} must be rejected"
        );
    }
}

#[test]
fn public_origin_rejects_local_internal_and_private_hosts() {
    for value in [
        "http://localhost:5150",
        "https://service.internal",
        "https://service.local",
        "http://127.0.0.1:5150",
        "http://10.0.0.1",
        "http://169.254.169.254",
    ] {
        assert!(
            resolve_public_origin_from_values(Some(value), None, None).is_err(),
            "{value} must be rejected"
        );
    }
}

#[test]
fn sitemap_file_count_always_includes_the_index() {
    assert_eq!(sitemap_file_count(0), 1);
    assert_eq!(sitemap_file_count(1), 2);
    assert_eq!(sitemap_file_count(super::SITEMAP_CHUNK_SIZE), 2);
    assert_eq!(sitemap_file_count(super::SITEMAP_CHUNK_SIZE + 1), 3);
}

#[test]
fn sitemap_event_keys_are_deterministic_and_outcome_sensitive() {
    let tenant_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let generated = sitemap_event_key(
        "seo.sitemap.generated",
        tenant_id,
        &[job_id.to_string(), "3".to_string()],
    );
    let repeated = sitemap_event_key(
        "seo.sitemap.generated",
        tenant_id,
        &[job_id.to_string(), "3".to_string()],
    );
    let submitted = sitemap_event_key(
        "seo.sitemap.submitted",
        tenant_id,
        &[job_id.to_string(), "2".to_string(), "true".to_string()],
    );

    assert_eq!(generated, repeated);
    assert_ne!(generated, submitted);
    assert!(generated.starts_with("seo.sitemap.generated:"));
    assert!(submitted.starts_with("seo.sitemap.submitted:"));
}

#[tokio::test]
async fn load_settings_returns_defaults_when_no_tenant_override_exists() {
    let db = test_db().await;
    seed_tenant_modules_table(&db).await;
    let tenant_id = Uuid::new_v4();
    let service = SeoService::new_memory(db);

    let settings = service
        .load_settings(tenant_id)
        .await
        .expect("load default settings");

    assert_eq!(settings.default_robots, vec!["index", "follow"]);
    assert!(settings.sitemap_enabled);
    assert!(settings.allowed_redirect_hosts.is_empty());
    assert!(settings.allowed_canonical_hosts.is_empty());
    assert_eq!(settings.x_default_locale, None);
    assert!(settings.sitemap_submission_endpoints.is_empty());
}

#[tokio::test]
async fn load_settings_normalizes_hosts_robots_and_locale() {
    let db = test_db().await;
    seed_tenant_modules_table(&db).await;
    let tenant_id = Uuid::new_v4();
    insert_seo_settings(
        &db,
        tenant_id,
        json!({
            "default_robots": [" Index ", "FOLLOW", "noarchive", "index"],
            "sitemap_enabled": true,
            "allowed_redirect_hosts": [" Example.com ", "cdn.example.com", "example.com"],
            "allowed_canonical_hosts": [" Blog.Example.com "],
            "x_default_locale": " EN-us ",
            "sitemap_submission_endpoints": [
                "https://www.google.com/ping?sitemap=https://store.example.com/sitemap.xml",
                "http://localhost:8080/seo/ping#ignored-fragment",
                "invalid://endpoint",
                "https://www.google.com/ping?sitemap=https://store.example.com/sitemap.xml"
            ]
        }),
    )
    .await;

    let service = SeoService::new_memory(db);
    let settings = service
        .load_settings(tenant_id)
        .await
        .expect("load normalized settings");

    assert_eq!(
        settings.default_robots,
        vec!["index", "follow", "noarchive"]
    );
    assert_eq!(
        settings.allowed_redirect_hosts,
        vec!["example.com", "cdn.example.com"]
    );
    assert_eq!(settings.allowed_canonical_hosts, vec!["blog.example.com"]);
    assert_eq!(settings.x_default_locale.as_deref(), Some("en-US"));
    assert_eq!(
        settings.sitemap_submission_endpoints,
        vec![
            "http://localhost:8080/seo/ping".to_string(),
            "https://www.google.com/ping?sitemap=https://store.example.com/sitemap.xml".to_string()
        ]
    );
}

#[test]
fn normalize_sitemap_submission_endpoints_filters_invalid_and_deduplicates() {
    let normalized = normalize_sitemap_submission_endpoints(&[
        " https://example.com/ping?sitemap=https://store/sitemap.xml ".to_string(),
        "ftp://example.com/not-supported".to_string(),
        "not a url".to_string(),
        "https://example.com/ping?sitemap=https://store/sitemap.xml#fragment".to_string(),
    ]);

    assert_eq!(
        normalized,
        vec!["https://example.com/ping?sitemap=https://store/sitemap.xml".to_string()]
    );
}

#[test]
fn build_sitemap_submission_url_supports_placeholder_and_query_append() {
    let placeholder = super::build_sitemap_submission_url(
        "https://example.com/ping?source=rustok&sitemap={sitemap_url}",
        "https://store.example.com/sitemap.xml",
    )
    .expect("placeholder url");
    assert_eq!(
        placeholder,
        "https://example.com/ping?source=rustok&sitemap=https%3A%2F%2Fstore.example.com%2Fsitemap.xml"
    );

    let appended = super::build_sitemap_submission_url(
        "https://example.com/ping?source=rustok",
        "https://store.example.com/sitemap.xml",
    )
    .expect("query append url");
    assert_eq!(
        appended,
        "https://example.com/ping?source=rustok&sitemap=https%3A%2F%2Fstore.example.com%2Fsitemap.xml"
    );
}

#[test]
fn build_sitemap_submission_url_rejects_non_http_and_keeps_existing_sitemap() {
    let keeps_existing = super::build_sitemap_submission_url(
        "https://example.com/ping?sitemap=https://preset.example.com/sitemap.xml",
        "https://store.example.com/sitemap.xml",
    )
    .expect("existing sitemap");
    assert_eq!(
        keeps_existing,
        "https://example.com/ping?sitemap=https://preset.example.com/sitemap.xml"
    );

    let invalid_scheme = super::build_sitemap_submission_url(
        "ftp://example.com/ping",
        "https://store.example.com/sitemap.xml",
    );
    assert!(invalid_scheme.is_none());
}

#[tokio::test]
async fn robots_preview_uses_tenant_domain_and_omits_sitemap_when_disabled() {
    let db = test_db().await;
    seed_tenant_modules_table(&db).await;
    let tenant_id = Uuid::new_v4();
    insert_seo_settings(
        &db,
        tenant_id,
        json!({
            "default_robots": ["index", "follow"],
            "sitemap_enabled": false
        }),
    )
    .await;

    let service = SeoService::new_memory(db);
    let preview = service
        .robots_preview(&tenant_context(tenant_id))
        .await
        .expect("load robots preview");

    assert_eq!(preview.public_url, "https://store.example.com/robots.txt");
    assert_eq!(preview.sitemap_index_url, None);
    assert_eq!(preview.body, "User-agent: *\nAllow: /\n");
}

#[tokio::test]
async fn sitemap_status_returns_disabled_snapshot_without_jobs() {
    let db = test_db().await;
    seed_tenant_modules_table(&db).await;
    let tenant_id = Uuid::new_v4();
    insert_seo_settings(
        &db,
        tenant_id,
        json!({
            "default_robots": ["index", "follow"],
            "sitemap_enabled": false
        }),
    )
    .await;

    let service = SeoService::new_memory(db);
    let status = service
        .sitemap_status(&tenant_context(tenant_id))
        .await
        .expect("load sitemap status");

    assert!(!status.enabled);
    assert_eq!(status.latest_job_id, None);
    assert_eq!(status.status, None);
    assert_eq!(status.file_count, 0);
    assert!(status.files.is_empty());
}

#[tokio::test]
async fn submit_sitemap_endpoints_empty_input_short_circuits_without_submissions() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::new());

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_ok());
    assert!(adapter.submitted_endpoints().await.is_empty());
    assert!(adapter.submitted_request_urls().await.is_empty());
}

#[tokio::test]
async fn submit_sitemap_endpoints_all_success_returns_ok() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([
        ("https://ok-1.example.com/ping".to_string(), Ok(())),
        ("https://ok-2.example.com/ping".to_string(), Ok(())),
    ]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[
                "https://ok-1.example.com/ping".to_string(),
                "https://ok-2.example.com/ping".to_string(),
            ],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn submit_sitemap_endpoints_reports_success_failure_and_invalid() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([
        ("https://ok.example.com/ping".to_string(), Ok(())),
        (
            "https://fail.example.com/ping".to_string(),
            Err("endpoint `https://fail.example.com/ping` responded with status 500 Internal Server Error".to_string()),
        ),
    ]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[
                "https://ok.example.com/ping".to_string(),
                "https://fail.example.com/ping".to_string(),
                "invalid endpoint".to_string(),
            ],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    let message = result.expect_err("must return aggregate error");
    assert!(message.contains("1 success(es) and 2 failure(s)"));
    assert!(message.contains("endpoint `https://fail.example.com/ping` responded with status 500"));
    assert!(message.contains("invalid endpoint: invalid endpoint"));
    let submitted = adapter.submitted_endpoints().await;
    assert_eq!(
        submitted,
        vec![
            "https://ok.example.com/ping".to_string(),
            "https://fail.example.com/ping".to_string(),
        ]
    );
}

#[tokio::test]
async fn submit_sitemap_endpoints_passes_normalized_request_urls_to_adapter() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([
        ("https://example.com/ping?source=rustok".to_string(), Ok(())),
        (
            "https://example.com/ping?sitemap={sitemap_url}".to_string(),
            Ok(()),
        ),
    ]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[
                "https://example.com/ping?source=rustok".to_string(),
                "https://example.com/ping?sitemap={sitemap_url}".to_string(),
            ],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_ok());
    let urls = adapter.submitted_request_urls().await;
    assert_eq!(
        urls,
        vec![
            "https://example.com/ping?source=rustok&sitemap=https%3A%2F%2Fstore.example.com%2Fsitemap.xml".to_string(),
            "https://example.com/ping?sitemap=https%3A%2F%2Fstore.example.com%2Fsitemap.xml"
                .to_string(),
        ]
    );
}

#[tokio::test]
async fn submit_sitemap_endpoints_preserves_valid_endpoint_order() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([
        ("https://first.example.com/ping".to_string(), Ok(())),
        ("https://second.example.com/ping".to_string(), Ok(())),
        ("https://third.example.com/ping".to_string(), Ok(())),
    ]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[
                "https://first.example.com/ping".to_string(),
                "invalid endpoint".to_string(),
                "https://second.example.com/ping".to_string(),
                "https://third.example.com/ping".to_string(),
            ],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_err());
    let submitted = adapter.submitted_endpoints().await;
    assert_eq!(
        submitted,
        vec![
            "https://first.example.com/ping".to_string(),
            "https://second.example.com/ping".to_string(),
            "https://third.example.com/ping".to_string(),
        ]
    );
}

#[tokio::test]
async fn submit_sitemap_endpoints_whitespace_only_endpoint_is_counted_as_invalid() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::new());

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &["         ".to_string()],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    let message = result.expect_err("whitespace endpoint must be invalid");
    assert!(message.contains("0 success(es) and 1 failure(s)"));
    assert!(message.contains("invalid endpoint:"));
    assert!(adapter.submitted_endpoints().await.is_empty());
    assert!(adapter.submitted_request_urls().await.is_empty());
}

#[tokio::test]
async fn submit_sitemap_endpoints_invalid_endpoint_is_not_submitted() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::new());

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &["not a valid url".to_string()],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    let message = result.expect_err("invalid endpoint should fail");
    assert!(message.contains("0 success(es) and 1 failure(s)"));
    assert!(message.contains("invalid endpoint: not a valid url"));
    let submitted = adapter.submitted_endpoints().await;
    assert!(submitted.is_empty());
}

#[tokio::test]
async fn submit_sitemap_endpoints_keeps_existing_sitemap_query_in_adapter_payload() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let endpoint = "https://example.com/ping?sitemap=https://preset.example.com/sitemap.xml";
    let adapter =
        TestSitemapSubmissionAdapter::new(HashMap::from([(endpoint.to_string(), Ok(()))]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[endpoint.to_string()],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_ok());
    let urls = adapter.submitted_request_urls().await;
    assert_eq!(
        urls,
        vec!["https://example.com/ping?sitemap=https://preset.example.com/sitemap.xml".to_string()]
    );
}

#[tokio::test]
async fn submit_sitemap_endpoints_preserves_case_insensitive_sitemap_query_key() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let endpoint = "https://example.com/ping?SITEMAP=https://preset.example.com/sitemap.xml";
    let adapter =
        TestSitemapSubmissionAdapter::new(HashMap::from([(endpoint.to_string(), Ok(()))]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[endpoint.to_string()],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    assert!(result.is_ok());
    let urls = adapter.submitted_request_urls().await;
    assert_eq!(
        urls,
        vec!["https://example.com/ping?SITEMAP=https://preset.example.com/sitemap.xml".to_string()]
    );
}

#[tokio::test]
async fn submit_sitemap_endpoints_timeout_and_failure_messages_are_bounded() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);
    let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([
        (
            "https://timeout.example.com/ping".to_string(),
            Err(format!(
                "request failed for endpoint `https://timeout.example.com/ping` with error: {}",
                "operation timed out ".repeat(400)
            )),
        ),
        (
            "https://failure.example.com/ping".to_string(),
            Err(format!(
                "endpoint `https://failure.example.com/ping` responded with status 503 and body: {}",
                "service unavailable ".repeat(400)
            )),
        ),
    ]));

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            &[
                "https://timeout.example.com/ping".to_string(),
                "https://failure.example.com/ping".to_string(),
            ],
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    let message = result.expect_err("aggregated bounded error expected");
    assert!(message.contains("0 success(es) and 2 failure(s)"));
    assert!(message.contains("errors:"));
    assert!(message.contains("timeouts:"));
    assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
}

#[tokio::test]
async fn submit_sitemap_endpoints_truncates_timeout_and_failure_details_deterministically() {
    let db = test_db().await;
    let service = SeoService::new_memory(db);

    let mut outcomes = HashMap::new();
    for index in 0..16 {
        let endpoint = format!("https://failure-{index:02}.example.com/ping");
        outcomes.insert(
            endpoint.clone(),
            Err(format!("failure detail {index:02}: {}", "x".repeat(500))),
        );
    }
    for index in 0..12 {
        let endpoint = format!("https://timeout-{index:02}.example.com/ping");
        outcomes.insert(
            endpoint.clone(),
            Err(format!(
                "request failed for endpoint `{endpoint}` with error: operation timed out {}",
                "y".repeat(500)
            )),
        );
    }
    let adapter = TestSitemapSubmissionAdapter::new(outcomes);

    let mut endpoints = (0..16)
        .rev()
        .map(|index| format!("https://failure-{index:02}.example.com/ping"))
        .collect::<Vec<_>>();
    endpoints.extend(
        (0..12)
            .rev()
            .map(|index| format!("https://timeout-{index:02}.example.com/ping")),
    );

    let result = service
        .submit_sitemap_endpoints_with_adapter(
            endpoints.as_slice(),
            "https://store.example.com/sitemap.xml",
            &adapter,
        )
        .await;

    let message = result.expect_err("expected aggregate error");
    assert!(message.contains("0 success(es) and 28 failure(s)"));
    assert!(message.contains(&format!(
        "errors omitted: {}",
        16 - SITEMAP_SUBMIT_MAX_ERRORS
    )));
    assert!(message.contains(&format!(
        "timeout details omitted: {}",
        12 - SITEMAP_SUBMIT_MAX_TIMEOUT_DETAILS
    )));

    let failure_00 = message
        .find("failure detail 00")
        .expect("deterministic failure ordering should keep failure-00");
    let failure_01 = message
        .find("failure detail 01")
        .expect("deterministic failure ordering should keep failure-01");
    assert!(failure_00 < failure_01);

    let timeout_00 = message
        .find("timeout-00")
        .expect("deterministic timeout ordering should keep timeout-00");
    let timeout_01 = message
        .find("timeout-01")
        .expect("deterministic timeout ordering should keep timeout-01");
    assert!(timeout_00 < timeout_01);

    assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
}

#[test]
fn submission_summary_without_failures_returns_none() {
    let summary = SitemapSubmissionSummary {
        success_count: 3,
        ..Default::default()
    };
    assert_eq!(summary.into_error(), None);
}

#[test]
fn submission_summary_with_failure_count_but_empty_details_still_returns_error() {
    let summary = SitemapSubmissionSummary {
        success_count: 2,
        failure_count: 1,
        ..Default::default()
    };

    let message = summary.into_error().expect("error summary expected");
    assert_eq!(
        message,
        "sitemap submission finished with 2 success(es) and 1 failure(s)"
    );
}

#[test]
fn submission_summary_truncates_bounded_error_message() {
    let mut summary = SitemapSubmissionSummary::default();
    record_submission_failure(
        &mut summary,
        "https://failure.example.com/ping",
        format!(
            "failure: {}",
            "x".repeat(SITEMAP_SUBMIT_MAX_ERROR_LEN + 200)
        ),
    );
    let message = summary.into_error().expect("error expected");
    assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
    assert!(message.ends_with("..."));
}

#[test]
fn submission_summary_truncation_respects_length_budget_with_unicode() {
    let mut summary = SitemapSubmissionSummary::default();
    record_submission_failure(
        &mut summary,
        "https://пример.рф/ping",
        format!("деталь: {}", "Ж".repeat(10_000)),
    );
    let message = summary.into_error().expect("error expected");

    assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
    assert!(message.ends_with("..."));
    assert!(std::str::from_utf8(message.as_bytes()).is_ok());
}

#[test]
fn submission_summary_limits_error_and_timeout_details() {
    let mut summary = SitemapSubmissionSummary::default();
    for index in 0..(SITEMAP_SUBMIT_MAX_ERRORS + 2) {
        record_submission_failure(
            &mut summary,
            format!("https://failure-{index:02}.example.com/ping").as_str(),
            format!("failure detail {index:02}"),
        );
    }
    for index in 0..(SITEMAP_SUBMIT_MAX_TIMEOUT_DETAILS + 2) {
        let endpoint = format!("https://timeout-{index:02}.example.com/ping");
        record_submission_failure(
            &mut summary,
            endpoint.as_str(),
            format!("request failed for endpoint `{endpoint}` with error: operation timed out"),
        );
    }

    let message = summary.into_error().expect("error expected");
    assert!(message.contains("errors omitted: 2"));
    assert!(message.contains("timeout details omitted: 2"));
}

#[test]
fn submission_summary_telemetry_snapshot_is_sorted_and_bounded() {
    let mut summary = SitemapSubmissionSummary::default();
    for index in (0..40).rev() {
        record_submission_success(
            &mut summary,
            format!("https://endpoint-{index:02}.example.com/ping").as_str(),
        );
    }
    record_invalid_endpoint(&mut summary, "not-a-valid-endpoint");

    let snapshot = summary.telemetry_snapshot();
    assert_eq!(snapshot.endpoint_statuses.len(), 24);
    assert_eq!(snapshot.omitted_endpoint_status_count, 17);
    assert_eq!(
        snapshot
            .endpoint_statuses
            .first()
            .map(|status| status.endpoint.as_str()),
        Some("https://endpoint-00.example.com/ping")
    );
}
