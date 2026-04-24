use rustok_seo_targets::{SeoTargetCapabilityKind, SeoTargetSitemapRequest};
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use uuid::Uuid;

use rustok_api::TenantContext;

use crate::dto::{SeoRobotsPreviewRecord, SeoSitemapFileRecord, SeoSitemapStatusRecord};
use crate::entities::{seo_sitemap_file, seo_sitemap_job};
use crate::{SeoError, SeoResult};

use super::routing::locale_prefixed_path;
use super::{normalize_effective_locale, SeoService, SITEMAP_CHUNK_SIZE};

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

#[cfg(test)]
mod tests {
    use super::render_robots_body;
    use crate::SeoService;
    use rustok_api::TenantContext;
    use rustok_tenant::entities::tenant_module;
    use sea_orm::{
        ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
        DatabaseConnection, DbBackend, Statement,
    };
    use serde_json::json;
    use uuid::Uuid;

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
            settings: Set(settings.into()),
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
                "x_default_locale": " EN-us "
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
}
