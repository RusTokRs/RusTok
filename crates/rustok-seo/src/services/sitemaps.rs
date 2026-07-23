use std::collections::HashMap;

use rustok_core::security::{SsrfProtection, ValidationResult};
use rustok_core::{simple_hash, DomainEvent};
use rustok_seo_targets::{SeoTargetCapabilityKind, SeoTargetSitemapRequest};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, Order, QueryFilter,
    QueryOrder, TransactionTrait,
};
use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;

use crate::dto::{
    SeoRobotsPreviewRecord, SeoSitemapFileRecord, SeoSitemapJobRecord, SeoSitemapStatusRecord,
};
use crate::entities::{seo_event_delivery, seo_sitemap_file, seo_sitemap_job};
use crate::{SeoError, SeoResult};

use super::routing::locale_prefixed_path;
use super::{normalize_effective_locale, SeoService, SITEMAP_CHUNK_SIZE};

mod index_generation;
mod submission_adapters;
mod submission_aggregation;

use index_generation::{render_sitemap_file, render_sitemap_index};
use submission_adapters::{
    SitemapSubmissionAdapter, SitemapSubmissionRuntime, SitemapSubmitEndpoint,
};
use submission_aggregation::{
    record_invalid_endpoint, record_submission_failure, record_submission_success,
    SitemapSubmissionSummary,
};

const SITEMAP_SUBMIT_TIMEOUT_SECS: u64 = 5;
const DELIVERY_STATUS_PENDING: &str = "pending";

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicOrigin(String);

struct SitemapEventPublication {
    tenant_id: Uuid,
    job_id: Uuid,
    event_type: String,
    idempotency_key: String,
    event: DomainEvent,
    occurred_at: chrono::DateTime<chrono::FixedOffset>,
}

impl PublicOrigin {
    fn resolve(tenant: &TenantContext) -> SeoResult<Self> {
        let public_url = std::env::var("RUSTOK_PUBLIC_URL").ok();
        let api_url = std::env::var("RUSTOK_API_URL").ok();
        resolve_public_origin_from_values(
            tenant.domain.as_deref(),
            public_url.as_deref(),
            api_url.as_deref(),
        )
    }

    fn parse(raw: &str, source: &str) -> SeoResult<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(SeoError::configuration(format!(
                "SEO public origin from {source} must not be empty"
            )));
        }

        let candidate = if raw.contains("://") {
            raw.to_string()
        } else {
            format!("https://{raw}")
        };
        let mut parsed = Url::parse(candidate.as_str()).map_err(|error| {
            SeoError::configuration(format!("invalid SEO public origin from {source}: {error}"))
        })?;

        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(SeoError::configuration(format!(
                "invalid SEO public origin from {source}: URL credentials are not allowed"
            )));
        }
        if parsed.path() != "/" {
            return Err(SeoError::configuration(format!(
                "invalid SEO public origin from {source}: URL path must be empty"
            )));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(SeoError::configuration(format!(
                "invalid SEO public origin from {source}: query and fragment are not allowed"
            )));
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| {
                SeoError::configuration(format!(
                    "invalid SEO public origin from {source}: URL host is required"
                ))
            })?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty()
            || host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || host.ends_with(".internal")
            || host.ends_with(".home.arpa")
        {
            return Err(SeoError::configuration(format!(
                "invalid SEO public origin from {source}: local or internal hosts are not allowed"
            )));
        }
        parsed.set_host(Some(host.as_str())).map_err(|_| {
            SeoError::configuration(format!(
                "invalid SEO public origin from {source}: URL host could not be canonicalized"
            ))
        })?;

        match SsrfProtection::new().validate_url(parsed.as_str()) {
            ValidationResult::Valid => {}
            ValidationResult::Invalid { reason } => {
                return Err(SeoError::configuration(format!(
                    "invalid SEO public origin from {source}: {reason}"
                )));
            }
            ValidationResult::Sanitized { .. } => {
                return Err(SeoError::configuration(format!(
                    "invalid SEO public origin from {source}: unexpected validation result"
                )));
            }
        }

        Ok(Self(parsed.as_str().trim_end_matches('/').to_string()))
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn resolve_public_origin_from_values(
    tenant_domain: Option<&str>,
    public_url: Option<&str>,
    api_url: Option<&str>,
) -> SeoResult<PublicOrigin> {
    let candidate = tenant_domain
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| ("tenant domain", value))
        .or_else(|| {
            public_url
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| ("RUSTOK_PUBLIC_URL", value))
        })
        .or_else(|| {
            api_url
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| ("RUSTOK_API_URL", value))
        })
        .ok_or_else(|| {
            SeoError::configuration(
                "SEO public origin is not configured; set a tenant domain, RUSTOK_PUBLIC_URL, or RUSTOK_API_URL",
            )
        })?;

    PublicOrigin::parse(candidate.1, candidate.0)
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
        let public_origin = PublicOrigin::resolve(tenant)?;
        let started_at = chrono::Utc::now().fixed_offset();
        let urls = self.collect_sitemap_urls(tenant, &public_origin).await?;
        let completed_at = chrono::Utc::now().fixed_offset();
        let completed_job = self
            .persist_generated_sitemap_in_tx(
                tenant,
                &public_origin,
                &urls,
                started_at,
                completed_at,
            )
            .await?;

        if !settings.sitemap_submission_endpoints.is_empty() {
            let submission_error = match self
                .submit_sitemap_endpoints(&public_origin, &settings)
                .await
            {
                Ok(()) => None,
                Err(error) => {
                    tracing::warn!(tenant_id = %tenant.id, error = %error, "SEO sitemap submission failed");
                    Some(error)
                }
            };
            self.record_sitemap_submission_outcome(
                tenant.id,
                completed_job.id,
                settings.sitemap_submission_endpoints.len() as i32,
                submission_error,
            )
            .await?;
        }

        self.sitemap_status(tenant).await
    }

    async fn persist_generated_sitemap_in_tx(
        &self,
        tenant: &TenantContext,
        public_origin: &PublicOrigin,
        urls: &[String],
        started_at: chrono::DateTime<chrono::FixedOffset>,
        completed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<seo_sitemap_job::Model> {
        let txn = self.db.begin().await?;
        let file_count = sitemap_file_count(urls.len()) as i32;
        let job = seo_sitemap_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            status: Set("completed".to_string()),
            file_count: Set(file_count),
            started_at: Set(Some(started_at)),
            completed_at: Set(Some(completed_at)),
            last_error: Set(None),
            created_at: Set(started_at),
            updated_at: Set(completed_at),
        }
        .insert(&txn)
        .await?;

        self.persist_sitemap_files_in_tx(&txn, tenant, public_origin, job.id, urls, completed_at)
            .await?;

        let event_type = "seo.sitemap.generated";
        let idempotency_key = sitemap_event_key(
            event_type,
            tenant.id,
            &[job.id.to_string(), file_count.to_string()],
        );
        self.publish_sitemap_event_in_tx(
            &txn,
            SitemapEventPublication {
                tenant_id: tenant.id,
                job_id: job.id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: DomainEvent::SeoSitemapGenerated {
                    job_id: job.id,
                    file_count,
                    idempotency_key,
                },
                occurred_at: completed_at,
            },
        )
        .await?;

        txn.commit().await?;
        Ok(job)
    }

    async fn record_sitemap_submission_outcome(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        endpoint_count: i32,
        error: Option<String>,
    ) -> SeoResult<()> {
        let txn = self.db.begin().await?;
        let Some(job) = seo_sitemap_job::Entity::find_by_id(job_id)
            .filter(seo_sitemap_job::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
        else {
            return Err(SeoError::NotFound);
        };

        let now = chrono::Utc::now().fixed_offset();
        let success = error.is_none();
        let mut active_job: seo_sitemap_job::ActiveModel = job.into();
        active_job.last_error = Set(error.clone());
        active_job.updated_at = Set(now);
        active_job.update(&txn).await?;

        let event_type = "seo.sitemap.submitted";
        let idempotency_key = sitemap_event_key(
            event_type,
            tenant_id,
            &[
                job_id.to_string(),
                endpoint_count.to_string(),
                success.to_string(),
            ],
        );
        self.publish_sitemap_event_in_tx(
            &txn,
            SitemapEventPublication {
                tenant_id,
                job_id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: DomainEvent::SeoSitemapSubmitted {
                    job_id,
                    endpoint_count,
                    success,
                    error,
                    idempotency_key,
                },
                occurred_at: now,
            },
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn publish_sitemap_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        publication: SitemapEventPublication,
    ) -> SeoResult<()> {
        let SitemapEventPublication {
            tenant_id,
            job_id,
            event_type,
            idempotency_key,
            event,
            occurred_at,
        } = publication;
        let existing = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::IdempotencyKey.eq(idempotency_key.as_str()))
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, None, event)
            .await
            .map_err(|error| transactional_event_error("sitemap event", error))?;
        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_type),
            idempotency_key: Set(idempotency_key),
            source_kind: Set(Some("sitemap_job".to_string())),
            source_id: Set(Some(job_id)),
            status: Set(DELIVERY_STATUS_PENDING.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(occurred_at),
            updated_at: Set(occurred_at),
            dispatched_at: Set(None),
        }
        .insert(txn)
        .await?;

        Ok(())
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

    pub async fn list_sitemap_jobs(
        &self,
        tenant_id: Uuid,
        limit: usize,
    ) -> SeoResult<Vec<SeoSitemapJobRecord>> {
        let jobs = seo_sitemap_job::Entity::find()
            .filter(seo_sitemap_job::Column::TenantId.eq(tenant_id))
            .order_by_desc(seo_sitemap_job::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let jobs = jobs.into_iter().take(limit.max(1)).collect::<Vec<_>>();
        let job_ids = jobs.iter().map(|job| job.id).collect::<Vec<_>>();
        let files_map = self
            .load_sitemap_files_for_jobs(tenant_id, &job_ids)
            .await?;

        Ok(jobs
            .into_iter()
            .map(|job| map_sitemap_job_record(job, &files_map))
            .collect())
    }

    pub async fn sitemap_job(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> SeoResult<Option<SeoSitemapJobRecord>> {
        let Some(job) = seo_sitemap_job::Entity::find()
            .filter(seo_sitemap_job::Column::TenantId.eq(tenant_id))
            .filter(seo_sitemap_job::Column::Id.eq(job_id))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let files_map = self
            .load_sitemap_files_for_jobs(tenant_id, &[job.id])
            .await?;

        Ok(Some(map_sitemap_job_record(job, &files_map)))
    }

    pub async fn render_robots(&self, tenant: &TenantContext) -> SeoResult<String> {
        let settings = self.load_settings(tenant.id).await?;
        if !settings.sitemap_enabled {
            return Ok(render_robots_body("", false));
        }
        let public_origin = PublicOrigin::resolve(tenant)?;
        Ok(render_robots_body(public_origin.as_str(), true))
    }

    pub async fn robots_preview(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoRobotsPreviewRecord> {
        let settings = self.load_settings(tenant.id).await?;
        let public_origin = PublicOrigin::resolve(tenant)?;
        let base_url = public_origin.as_str();

        Ok(SeoRobotsPreviewRecord {
            body: render_robots_body(base_url, settings.sitemap_enabled),
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

    async fn load_sitemap_files_for_jobs(
        &self,
        tenant_id: Uuid,
        job_ids: &[Uuid],
    ) -> SeoResult<HashMap<Uuid, Vec<SeoSitemapFileRecord>>> {
        if job_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let files = seo_sitemap_file::Entity::find()
            .filter(seo_sitemap_file::Column::TenantId.eq(tenant_id))
            .filter(seo_sitemap_file::Column::JobId.is_in(job_ids.to_vec()))
            .order_by_asc(seo_sitemap_file::Column::Path)
            .all(&self.db)
            .await?;
        let mut map = HashMap::<Uuid, Vec<SeoSitemapFileRecord>>::new();
        for file in files {
            map.entry(file.job_id)
                .or_default()
                .push(SeoSitemapFileRecord {
                    id: file.id,
                    path: file.path,
                    url_count: file.url_count,
                    created_at: file.created_at.into(),
                });
        }

        Ok(map)
    }

    async fn persist_sitemap_files_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant: &TenantContext,
        public_origin: &PublicOrigin,
        job_id: Uuid,
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
                    job_id: Set(job_id),
                    path: Set(format!("sitemap-{}.xml", index + 1)),
                    url_count: Set(chunk.len() as i32),
                    content: Set(render_sitemap_file(chunk)),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?,
            );
        }

        let index_urls = files
            .iter()
            .map(|file| format!("{}/sitemaps/{}", public_origin.as_str(), file.path))
            .collect::<Vec<_>>();
        files.insert(
            0,
            seo_sitemap_file::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                job_id: Set(job_id),
                path: Set("sitemap.xml".to_string()),
                url_count: Set(urls.len() as i32),
                content: Set(render_sitemap_index(index_urls.as_slice())),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(txn)
            .await?,
        );

        Ok(files)
    }

    async fn collect_sitemap_urls(
        &self,
        tenant: &TenantContext,
        public_origin: &PublicOrigin,
    ) -> SeoResult<Vec<String>> {
        let base_url = public_origin.as_str();
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
        public_origin: &PublicOrigin,
        settings: &crate::dto::SeoModuleSettings,
    ) -> Result<(), String> {
        if settings.sitemap_submission_endpoints.is_empty() {
            return Ok(());
        }
        let sitemap_index_url = format!("{}/sitemap.xml", public_origin.as_str());
        let runtime = SitemapSubmissionRuntime::default_with_timeout(SITEMAP_SUBMIT_TIMEOUT_SECS)?;
        self.submit_sitemap_endpoints_with_adapter(
            settings.sitemap_submission_endpoints.as_slice(),
            sitemap_index_url.as_str(),
            runtime.adapter(),
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
        let telemetry = summary.telemetry_snapshot();
        tracing::debug!(
            success_count = summary.success_count,
            failure_count = summary.failure_count,
            endpoint_status_count = telemetry.endpoint_statuses.len(),
            endpoint_statuses = ?telemetry.endpoint_statuses,
            omitted_endpoint_status_count = telemetry.omitted_endpoint_status_count,
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
        let mut summary = SitemapSubmissionSummary::default();
        for endpoint in endpoints {
            let Some(url) = build_sitemap_submission_url(endpoint.as_str(), sitemap_index_url)
            else {
                record_invalid_endpoint(&mut summary, endpoint.as_str());
                continue;
            };
            let request = SitemapSubmitEndpoint {
                endpoint: endpoint.clone(),
                request_url: url,
            };
            match adapter.submit_sitemap_index(request).await {
                Ok(()) => record_submission_success(&mut summary, endpoint.as_str()),
                Err(message) => {
                    record_submission_failure(&mut summary, endpoint.as_str(), message);
                }
            }
        }
        summary
    }
}

fn sitemap_file_count(url_count: usize) -> usize {
    if url_count == 0 {
        1
    } else {
        ((url_count - 1) / SITEMAP_CHUNK_SIZE) + 2
    }
}

fn sitemap_event_key(scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
    let mut payload = format!("{scope}|{tenant_id}");
    for part in parts {
        payload.push('|');
        payload.push_str(part.as_str());
    }
    format!("{scope}:{:016x}", simple_hash(payload.as_str()))
}

fn transactional_event_error(context: &str, error: rustok_core::Error) -> SeoError {
    SeoError::Database(sea_orm::DbErr::Custom(format!(
        "failed to enqueue {context} transactionally: {error}"
    )))
}

fn map_sitemap_job_record(
    job: seo_sitemap_job::Model,
    files_map: &HashMap<Uuid, Vec<SeoSitemapFileRecord>>,
) -> SeoSitemapJobRecord {
    SeoSitemapJobRecord {
        id: job.id,
        status: job.status,
        file_count: job.file_count,
        started_at: job.started_at.map(Into::into),
        completed_at: job.completed_at.map(Into::into),
        last_error: job.last_error,
        files: files_map.get(&job.id).cloned().unwrap_or_default(),
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

fn render_robots_body(base_url: &str, sitemap_enabled: bool) -> String {
    if sitemap_enabled {
        format!("User-agent: *\nAllow: /\nSitemap: {base_url}/sitemap.xml\n")
    } else {
        "User-agent: *\nAllow: /\n".to_string()
    }
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
mod tests;
