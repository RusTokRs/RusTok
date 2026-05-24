use rustok_seo_targets::{SeoTargetCapabilityKind, SeoTargetSitemapRequest};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;

use crate::dto::{SeoRobotsPreviewRecord, SeoSitemapFileRecord, SeoSitemapStatusRecord};
use crate::entities::{seo_sitemap_file, seo_sitemap_job};
use crate::{SeoError, SeoResult};

use super::routing::locale_prefixed_path;
use super::{normalize_effective_locale, SeoService, SITEMAP_CHUNK_SIZE};

const SITEMAP_SUBMIT_TIMEOUT_SECS: u64 = 5;
const SITEMAP_SUBMIT_MAX_ERROR_LEN: usize = 4000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SitemapSubmitEndpoint {
    endpoint: String,
    request_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SitemapSubmissionSummary {
    success_count: usize,
    failure_count: usize,
    failures: Vec<String>,
}

impl SitemapSubmissionSummary {
    fn into_error(self) -> Option<String> {
        if self.failure_count == 0 {
            return None;
        }
        let mut parts = vec![format!(
            "sitemap submission finished with {} success(es) and {} failure(s)",
            self.success_count, self.failure_count
        )];
        parts.extend(self.failures);
        let mut message = parts.join("; ");
        if message.len() > SITEMAP_SUBMIT_MAX_ERROR_LEN {
            message.truncate(SITEMAP_SUBMIT_MAX_ERROR_LEN);
            message.push_str("...");
        }
        Some(message)
    }
}

#[async_trait::async_trait]
trait SitemapSubmissionAdapter: Send + Sync {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String>;
}

struct HttpSitemapSubmissionAdapter {
    client: reqwest::Client,
}

#[async_trait::async_trait]
impl SitemapSubmissionAdapter for HttpSitemapSubmissionAdapter {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String> {
        let response = self
            .client
            .get(endpoint.request_url)
            .send()
            .await
            .map_err(|error| {
                format!(
                    "request failed for endpoint `{}` with error: {error}",
                    endpoint.endpoint
                )
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "endpoint `{}` responded with status {}",
                endpoint.endpoint,
                response.status()
            ))
        }
    }
}

impl SeoService {
    pub async fn generate_sitemaps(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoSitemapStatusRecord> {
        let settings = self.load_settings(tenant.id).await?;
        if !settings.sitemap_enabled {
            return Ok(disabled_sitemap_status());
        }

        let now = chrono::Utc::now().fixed_offset();
        let job = seo_sitemap_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            status: Set("running".to_string()),
            file_count: Set(0),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        let urls = self.collect_sitemap_urls(tenant).await?;
        let file_models = self.persist_sitemap_files(tenant, &job, &urls, now).await?;
        let mut active_job: seo_sitemap_job::ActiveModel = job.into();
        active_job.status = Set("completed".to_string());
        active_job.file_count = Set(file_models.len() as i32);
        active_job.completed_at = Set(Some(now));
        active_job.last_error = match self.submit_sitemap_endpoints(tenant, &settings).await {
            Ok(()) => Set(None),
            Err(error) => {
                tracing::warn!(tenant_id = %tenant.id, error = %error, "SEO sitemap submission failed");
                Set(Some(error))
            }
        };
        active_job.updated_at = Set(now);
        active_job.update(&self.db).await?;

        self.sitemap_status(tenant).await
    }

    pub async fn sitemap_status(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoSitemapStatusRecord> {
        let settings = self.load_settings(tenant.id).await?;
        if !settings.sitemap_enabled {
            return Ok(disabled_sitemap_status());
        }

        let latest_job = seo_sitemap_job::Entity::find()
            .filter(seo_sitemap_job::Column::TenantId.eq(tenant.id))
            .order_by_desc(seo_sitemap_job::Column::CreatedAt)
            .one(&self.db)
            .await?;
        let Some(latest_job) = latest_job else {
            return Ok(SeoSitemapStatusRecord {
                enabled: true,
                latest_job_id: None,
                status: None,
                file_count: 0,
                generated_at: None,
                files: Vec::new(),
            });
        };

        let files = seo_sitemap_file::Entity::find()
            .filter(seo_sitemap_file::Column::TenantId.eq(tenant.id))
            .filter(seo_sitemap_file::Column::JobId.eq(latest_job.id))
            .order_by(seo_sitemap_file::Column::Path, Order::Asc)
            .all(&self.db)
            .await?;

        Ok(SeoSitemapStatusRecord {
            enabled: true,
            latest_job_id: Some(latest_job.id),
            status: Some(latest_job.status),
            file_count: latest_job.file_count,
            generated_at: latest_job.completed_at.map(Into::into),
            files: files
                .into_iter()
                .map(|file| SeoSitemapFileRecord {
                    id: file.id,
                    path: file.path,
                    url_count: file.url_count,
                    created_at: file.created_at.into(),
                })
                .collect(),
        })
    }

    pub async fn render_robots(&self, tenant: &TenantContext) -> SeoResult<String> {
        let settings = self.load_settings(tenant.id).await?;
        Ok(render_robots_body(
            public_base_url(tenant).as_str(),
            settings.sitemap_enabled,
        ))
    }

    pub async fn robots_preview(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoRobotsPreviewRecord> {
        let settings = self.load_settings(tenant.id).await?;
        let base_url = public_base_url(tenant);

        Ok(SeoRobotsPreviewRecord {
            body: render_robots_body(base_url.as_str(), settings.sitemap_enabled),
            public_url: format!("{base_url}/robots.txt"),
            sitemap_index_url: settings
                .sitemap_enabled
                .then(|| format!("{base_url}/sitemap.xml")),
        })
    }

    pub async fn latest_sitemap_index(
        &self,
        tenant_id: Uuid,
    ) -> SeoResult<Option<seo_sitemap_file::Model>> {
        let latest_job = seo_sitemap_job::Entity::find()
            .filter(seo_sitemap_job::Column::TenantId.eq(tenant_id))
            .order_by_desc(seo_sitemap_job::Column::CreatedAt)
            .one(&self.db)
            .await?;
        let Some(latest_job) = latest_job else {
            return Ok(None);
        };
        seo_sitemap_file::Entity::find()
            .filter(seo_sitemap_file::Column::TenantId.eq(tenant_id))
            .filter(seo_sitemap_file::Column::JobId.eq(latest_job.id))
            .filter(seo_sitemap_file::Column::Path.eq("sitemap.xml"))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn sitemap_file(
        &self,
        tenant_id: Uuid,
        path: &str,
    ) -> SeoResult<Option<seo_sitemap_file::Model>> {
        seo_sitemap_file::Entity::find()
            .filter(seo_sitemap_file::Column::TenantId.eq(tenant_id))
            .filter(seo_sitemap_file::Column::Path.eq(path))
            .order_by_desc(seo_sitemap_file::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    async fn persist_sitemap_files(
        &self,
        tenant: &TenantContext,
        job: &seo_sitemap_job::Model,
        urls: &[String],
        now: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<Vec<seo_sitemap_file::Model>> {
        let chunks = urls.chunks(SITEMAP_CHUNK_SIZE).collect::<Vec<_>>();
        let mut files = Vec::new();
        for (index, chunk) in chunks.iter().enumerate() {
            files.push(
                seo_sitemap_file::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant.id),
                    job_id: Set(job.id),
                    path: Set(format!("sitemap-{}.xml", index + 1)),
                    url_count: Set(chunk.len() as i32),
                    content: Set(render_sitemap_file(chunk)),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&self.db)
                .await?,
            );
        }

        let index_urls = files
            .iter()
            .map(|file| format!("{}/sitemaps/{}", public_base_url(tenant), file.path))
            .collect::<Vec<_>>();
        files.insert(
            0,
            seo_sitemap_file::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                job_id: Set(job.id),
                path: Set("sitemap.xml".to_string()),
                url_count: Set(urls.len() as i32),
                content: Set(render_sitemap_index(index_urls.as_slice())),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&self.db)
            .await?,
        );

        Ok(files)
    }

    async fn collect_sitemap_urls(&self, tenant: &TenantContext) -> SeoResult<Vec<String>> {
        let base_url = public_base_url(tenant);
        let mut urls = Vec::new();
        for provider in self
            .registry
            .providers_with_capability(SeoTargetCapabilityKind::Sitemaps)
        {
            let candidates = provider
                .sitemap_candidates(
                    &self.target_runtime(),
                    SeoTargetSitemapRequest {
                        tenant_id: tenant.id,
                        default_locale: tenant.default_locale.as_str(),
                    },
                )
                .await
                .map_err(|error| {
                    SeoError::validation(format!(
                        "SEO target provider `{}` failed to collect sitemap candidates: {error}",
                        provider.slug().as_str()
                    ))
                })?;
            for candidate in candidates {
                let locale = normalize_effective_locale(
                    candidate.locale.as_str(),
                    tenant.default_locale.as_str(),
                )?;
                urls.push(format!(
                    "{base_url}{}",
                    locale_prefixed_path(locale.as_str(), candidate.route.as_str())
                ));
            }
        }

        urls.sort();
        urls.dedup();
        Ok(urls)
    }
}

impl SeoService {
    async fn submit_sitemap_endpoints(
        &self,
        tenant: &TenantContext,
        settings: &crate::dto::SeoModuleSettings,
    ) -> Result<(), String> {
        if settings.sitemap_submission_endpoints.is_empty() {
            return Ok(());
        }
        let sitemap_index_url = format!("{}/sitemap.xml", public_base_url(tenant));
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(SITEMAP_SUBMIT_TIMEOUT_SECS))
            .build()
            .map_err(|error| format!("failed to create sitemap submission client: {error}"))?;
        let adapter = HttpSitemapSubmissionAdapter { client };
        self.submit_sitemap_endpoints_with_adapter(
            settings.sitemap_submission_endpoints.as_slice(),
            sitemap_index_url.as_str(),
            &adapter,
        )
        .await
    }

    async fn submit_sitemap_endpoints_with_adapter(
        &self,
        endpoints: &[String],
        sitemap_index_url: &str,
        adapter: &dyn SitemapSubmissionAdapter,
    ) -> Result<(), String> {
        let summary = self
            .collect_submission_summary(endpoints, sitemap_index_url, adapter)
            .await;
        tracing::debug!(
            success_count = summary.success_count,
            failure_count = summary.failure_count,
            "SEO sitemap endpoint submission finished"
        );
        match summary.into_error() {
            Some(message) => Err(message),
            None => Ok(()),
        }
    }

    async fn collect_submission_summary(
        &self,
        endpoints: &[String],
        sitemap_index_url: &str,
        adapter: &dyn SitemapSubmissionAdapter,
    ) -> SitemapSubmissionSummary {
        let mut summary = SitemapSubmissionSummary {
            success_count: 0,
            failure_count: 0,
            failures: Vec::new(),
        };
        for endpoint in endpoints {
            let Some(url) = build_sitemap_submission_url(endpoint.as_str(), sitemap_index_url)
            else {
                summary.failure_count += 1;
                summary.failures.push(format!("invalid endpoint: {endpoint}"));
                continue;
            };
            let request = SitemapSubmitEndpoint {
                endpoint: endpoint.clone(),
                request_url: url,
            };
            match adapter.submit_sitemap_index(request).await {
                Ok(()) => summary.success_count += 1,
                Err(message) => {
                    summary.failure_count += 1;
                    summary.failures.push(message);
                }
            }
        }
        summary
    }
}

fn disabled_sitemap_status() -> SeoSitemapStatusRecord {
    SeoSitemapStatusRecord {
        enabled: false,
        latest_job_id: None,
        status: None,
        file_count: 0,
        generated_at: None,
        files: Vec::new(),
    }
}

fn public_base_url(tenant: &TenantContext) -> String {
    if let Some(domain) = tenant
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if domain.starts_with("http://") || domain.starts_with("https://") {
            return domain.trim_end_matches('/').to_string();
        }
        return format!("https://{}", domain.trim_end_matches('/'));
    }
    std::env::var("RUSTOK_PUBLIC_URL")
        .or_else(|_| std::env::var("RUSTOK_API_URL"))
        .unwrap_or_else(|_| "http://localhost:5150".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn render_robots_body(base_url: &str, sitemap_enabled: bool) -> String {
    if sitemap_enabled {
        format!("User-agent: *\nAllow: /\nSitemap: {base_url}/sitemap.xml\n")
    } else {
        "User-agent: *\nAllow: /\n".to_string()
    }
}

fn render_sitemap_file(urls: &[String]) -> String {
    let body = urls
        .iter()
        .map(|url| format!("<url><loc>{}</loc></url>", xml_escape(url)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{body}</urlset>"#
    )
}

fn render_sitemap_index(urls: &[String]) -> String {
    let body = urls
        .iter()
        .map(|url| format!("<sitemap><loc>{}</loc></sitemap>", xml_escape(url)))
        .collect::<Vec<_>>()
        .join("");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{body}</sitemapindex>"#
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_sitemap_submission_url(endpoint: &str, sitemap_index_url: &str) -> Option<String> {
    let normalized = endpoint.trim();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("{sitemap_url}") {
        let encoded: String =
            url::form_urlencoded::byte_serialize(sitemap_index_url.as_bytes()).collect();
        let replaced = normalized.replace("{sitemap_url}", encoded.as_str());
        let parsed = Url::parse(replaced.as_str()).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        return Some(parsed.to_string());
    }
    let mut parsed = Url::parse(normalized).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if !parsed
        .query_pairs()
        .any(|(name, _)| name.eq_ignore_ascii_case("sitemap"))
    {
        parsed
            .query_pairs_mut()
            .append_pair("sitemap", sitemap_index_url);
    }
    Some(parsed.to_string())
}

pub(super) fn normalize_sitemap_submission_endpoints(values: &[String]) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut unique = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(parsed) = url::Url::parse(trimmed) else {
            continue;
        };
        if !matches!(parsed.scheme(), "http" | "https") {
            continue;
        }
        let mut normalized = parsed;
        normalized.set_fragment(None);
        unique.insert(normalized.to_string());
    }
    unique.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_sitemap_submission_endpoints, render_robots_body, SitemapSubmissionAdapter,
        SitemapSubmissionSummary, SitemapSubmitEndpoint,
    };
    use crate::SeoService;
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
        async fn submit_sitemap_index(
            &self,
            endpoint: SitemapSubmitEndpoint,
        ) -> Result<(), String> {
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
                "https://www.google.com/ping?sitemap=https://store.example.com/sitemap.xml"
                    .to_string()
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
            (
                "https://ok.example.com/ping".to_string(),
                Ok(()),
            ),
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
        assert!(
            message.contains("endpoint `https://fail.example.com/ping` responded with status 500")
        );
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
            (
                "https://example.com/ping?source=rustok".to_string(),
                Ok(()),
            ),
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
                &["   	  ".to_string()],
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
        let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([(
            endpoint.to_string(),
            Ok(()),
        )]));

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
            vec!["https://example.com/ping?sitemap=https://preset.example.com/sitemap.xml"
                .to_string()]
        );
    }


    #[tokio::test]
    async fn submit_sitemap_endpoints_preserves_case_insensitive_sitemap_query_key() {
        let db = test_db().await;
        let service = SeoService::new_memory(db);
        let endpoint = "https://example.com/ping?SITEMAP=https://preset.example.com/sitemap.xml";
        let adapter = TestSitemapSubmissionAdapter::new(HashMap::from([(
            endpoint.to_string(),
            Ok(()),
        )]));

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
            vec!["https://example.com/ping?SITEMAP=https://preset.example.com/sitemap.xml"
                .to_string()]
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
        assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
        assert!(message.ends_with("..."));
    }

    #[test]
    fn submission_summary_without_failures_returns_none() {
        let summary = SitemapSubmissionSummary {
            success_count: 3,
            failure_count: 0,
            failures: Vec::new(),
        };
        assert_eq!(summary.into_error(), None);
    }

    #[test]
    fn submission_summary_with_failure_count_but_empty_details_still_returns_error() {
        let summary = SitemapSubmissionSummary {
            success_count: 2,
            failure_count: 1,
            failures: Vec::new(),
        };

        let message = summary.into_error().expect("error summary expected");
        assert_eq!(
            message,
            "sitemap submission finished with 2 success(es) and 1 failure(s)"
        );
    }

    #[test]
    fn submission_summary_truncates_bounded_error_message() {
        let summary = SitemapSubmissionSummary {
            success_count: 0,
            failure_count: 1,
            failures: vec!["x".repeat(SITEMAP_SUBMIT_MAX_ERROR_LEN + 200)],
        };
        let message = summary.into_error().expect("error expected");
        assert!(message.len() <= SITEMAP_SUBMIT_MAX_ERROR_LEN + 3);
        assert!(message.ends_with("..."));
    }
}
