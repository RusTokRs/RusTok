#[path = "sitemaps/index_generation.rs"]
mod background_sitemap_index_generation;
#[path = "sitemaps/submission_adapters.rs"]
mod background_sitemap_submission_adapters;
#[path = "sitemaps/submission_aggregation.rs"]
mod background_sitemap_submission_aggregation;

const SITEMAP_JOB_QUEUED: &str = "queued";
const SITEMAP_JOB_RUNNING: &str = "running";
const SITEMAP_JOB_SUBMITTING: &str = "submitting";
const SITEMAP_JOB_COMPLETED: &str = "completed";
const SITEMAP_JOB_FAILED: &str = "failed";
const SITEMAP_BACKGROUND_SUBMIT_TIMEOUT_SECS: u64 = 5;
const SITEMAP_BACKGROUND_DELIVERY_STATUS_SENT: &str = "sent";

struct BackgroundSitemapEventPublication {
    tenant_id: Uuid,
    job_id: Uuid,
    event_type: String,
    idempotency_key: String,
    event: rustok_core::DomainEvent,
    occurred_at: chrono::DateTime<chrono::FixedOffset>,
}

impl SeoService {
    pub(super) async fn queue_sitemap_generation_background(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<crate::dto::SeoSitemapStatusRecord> {
        let settings = self.load_settings(tenant.id).await?;
        if !settings.sitemap_enabled {
            return Ok(background_disabled_sitemap_status());
        }

        // Validate the public origin while the caller can still receive a configuration error,
        // but do not collect targets, render XML, or perform external submissions here.
        self.robots_preview(tenant).await?;

        let active = crate::entities::seo_sitemap_job::Entity::find()
            .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(tenant.id))
            .filter(crate::entities::seo_sitemap_job::Column::Status.is_in([
                SITEMAP_JOB_QUEUED,
                SITEMAP_JOB_RUNNING,
                SITEMAP_JOB_SUBMITTING,
            ]))
            .order_by_asc(crate::entities::seo_sitemap_job::Column::CreatedAt)
            .one(&self.db)
            .await?;
        if active.is_some() {
            return self.sitemap_status(tenant).await;
        }

        let now = chrono::Utc::now().fixed_offset();
        crate::entities::seo_sitemap_job::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            status: Set(SITEMAP_JOB_QUEUED.to_string()),
            file_count: Set(0),
            started_at: Set(None),
            completed_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.db)
        .await?;

        self.sitemap_status(tenant).await
    }

    pub(super) async fn execute_next_sitemap_job_background(
        &self,
    ) -> SeoResult<Option<crate::dto::SeoSitemapJobRecord>> {
        let active = crate::entities::seo_sitemap_job::Entity::find()
            .filter(crate::entities::seo_sitemap_job::Column::Status.is_in([
                SITEMAP_JOB_RUNNING,
                SITEMAP_JOB_SUBMITTING,
            ]))
            .order_by_asc(crate::entities::seo_sitemap_job::Column::UpdatedAt)
            .one(&self.db)
            .await?;

        let job = if let Some(job) = active {
            job
        } else {
            let Some(job) = crate::entities::seo_sitemap_job::Entity::find()
                .filter(
                    crate::entities::seo_sitemap_job::Column::Status.eq(SITEMAP_JOB_QUEUED),
                )
                .order_by_asc(crate::entities::seo_sitemap_job::Column::CreatedAt)
                .one(&self.db)
                .await?
            else {
                return Ok(None);
            };

            let now = chrono::Utc::now().fixed_offset();
            let mut active: crate::entities::seo_sitemap_job::ActiveModel = job.into();
            active.status = Set(SITEMAP_JOB_RUNNING.to_string());
            active.started_at = Set(Some(now));
            active.completed_at = Set(None);
            active.last_error = Set(None);
            active.updated_at = Set(now);
            active.update(&self.db).await?
        };

        let result = if job.status == SITEMAP_JOB_SUBMITTING {
            self.execute_sitemap_submission_phase(&job).await
        } else {
            self.execute_sitemap_generation_phase(&job).await
        };
        if let Err(error) = result {
            self.fail_background_sitemap_job(&job, error.to_string())
                .await?;
        }

        self.sitemap_job(job.tenant_id, job.id).await
    }

    async fn execute_sitemap_generation_phase(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
    ) -> SeoResult<()> {
        let tenant = self.load_background_sitemap_tenant(job.tenant_id).await?;
        let settings = self.load_settings(tenant.id).await?;
        if !settings.sitemap_enabled {
            return Err(SeoError::configuration(
                "sitemap generation was disabled after the job was queued",
            ));
        }

        let preview = self.robots_preview(&tenant).await?;
        let public_origin = preview
            .public_url
            .strip_suffix("/robots.txt")
            .ok_or_else(|| SeoError::configuration("invalid SEO robots public URL"))?
            .to_string();
        let urls = self
            .collect_background_sitemap_urls(&tenant, public_origin.as_str())
            .await?;
        let generated_at = chrono::Utc::now().fixed_offset();
        self.persist_background_sitemap_generation(
            job,
            &tenant,
            public_origin.as_str(),
            urls.as_slice(),
            !settings.sitemap_submission_endpoints.is_empty(),
            generated_at,
        )
        .await
    }

    async fn execute_sitemap_submission_phase(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
    ) -> SeoResult<()> {
        let tenant = self.load_background_sitemap_tenant(job.tenant_id).await?;
        let settings = self.load_settings(tenant.id).await?;
        let preview = self.robots_preview(&tenant).await?;
        let sitemap_index_url = preview
            .public_url
            .strip_suffix("/robots.txt")
            .map(|origin| format!("{origin}/sitemap.xml"))
            .ok_or_else(|| SeoError::configuration("invalid SEO robots public URL"))?;
        let endpoint_count = settings.sitemap_submission_endpoints.len() as i32;
        let error = if settings.sitemap_submission_endpoints.is_empty() {
            None
        } else {
            match submit_background_sitemap_endpoints(
                settings.sitemap_submission_endpoints.as_slice(),
                sitemap_index_url.as_str(),
            )
            .await
            {
                Ok(()) => None,
                Err(error) => {
                    tracing::warn!(
                        tenant_id = %tenant.id,
                        job_id = %job.id,
                        error = %error,
                        "background SEO sitemap submission failed"
                    );
                    Some(error)
                }
            }
        };

        self.record_background_sitemap_submission(job, endpoint_count, error)
            .await
    }

    async fn load_background_sitemap_tenant(
        &self,
        tenant_id: Uuid,
    ) -> SeoResult<TenantContext> {
        let tenant = rustok_tenant::entities::tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(SeoError::NotFound)?;
        Ok(TenantContext {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            settings: tenant.settings,
            default_locale: tenant.default_locale,
            is_active: tenant.is_active,
        })
    }

    async fn collect_background_sitemap_urls(
        &self,
        tenant: &TenantContext,
        public_origin: &str,
    ) -> SeoResult<Vec<String>> {
        let mut urls = Vec::new();
        for provider in self.registry.providers_with_capability(
            rustok_seo_targets::SeoTargetCapabilityKind::Sitemaps,
        ) {
            let candidates = provider
                .sitemap_candidates(
                    &self.target_runtime(),
                    rustok_seo_targets::SeoTargetSitemapRequest {
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
                let locale = super::normalize_effective_locale(
                    candidate.locale.as_str(),
                    tenant.default_locale.as_str(),
                )?;
                urls.push(format!(
                    "{public_origin}{}",
                    super::routing::locale_prefixed_path(
                        locale.as_str(),
                        candidate.route.as_str(),
                    )
                ));
            }
        }

        urls.sort();
        urls.dedup();
        Ok(urls)
    }

    async fn persist_background_sitemap_generation(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        tenant: &TenantContext,
        public_origin: &str,
        urls: &[String],
        requires_submission: bool,
        generated_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let txn = self.db.begin().await?;
        let file_count = background_sitemap_file_count(urls.len()) as i32;

        crate::entities::seo_sitemap_file::Entity::delete_many()
            .filter(crate::entities::seo_sitemap_file::Column::TenantId.eq(tenant.id))
            .filter(crate::entities::seo_sitemap_file::Column::JobId.eq(job.id))
            .exec(&txn)
            .await?;
        self.persist_background_sitemap_files(
            &txn,
            tenant,
            public_origin,
            job.id,
            urls,
            generated_at,
        )
        .await?;

        let mut active: crate::entities::seo_sitemap_job::ActiveModel = job.clone().into();
        active.status = Set(if requires_submission {
            SITEMAP_JOB_SUBMITTING.to_string()
        } else {
            SITEMAP_JOB_COMPLETED.to_string()
        });
        active.file_count = Set(file_count);
        active.last_error = Set(None);
        active.completed_at = Set((!requires_submission).then_some(generated_at));
        active.updated_at = Set(generated_at);
        active.update(&txn).await?;

        let event_type = "seo.sitemap.generated";
        let idempotency_key = background_sitemap_event_key(
            event_type,
            tenant.id,
            &[job.id.to_string(), file_count.to_string()],
        );
        self.publish_background_sitemap_event(
            &txn,
            BackgroundSitemapEventPublication {
                tenant_id: tenant.id,
                job_id: job.id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: rustok_core::DomainEvent::SeoSitemapGenerated {
                    job_id: job.id,
                    file_count,
                    idempotency_key,
                },
                occurred_at: generated_at,
            },
        )
        .await?;

        txn.commit().await?;
        Ok(())
    }

    async fn persist_background_sitemap_files(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant: &TenantContext,
        public_origin: &str,
        job_id: Uuid,
        urls: &[String],
        now: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let mut files = Vec::new();
        for (index, chunk) in urls.chunks(super::SITEMAP_CHUNK_SIZE).enumerate() {
            files.push(
                crate::entities::seo_sitemap_file::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant.id),
                    job_id: Set(job_id),
                    path: Set(format!("sitemap-{}.xml", index + 1)),
                    url_count: Set(chunk.len() as i32),
                    content: Set(background_sitemap_index_generation::render_sitemap_file(chunk)),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?,
            );
        }

        let index_urls = files
            .iter()
            .map(|file| format!("{public_origin}/sitemaps/{}", file.path))
            .collect::<Vec<_>>();
        crate::entities::seo_sitemap_file::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            job_id: Set(job_id),
            path: Set("sitemap.xml".to_string()),
            url_count: Set(urls.len() as i32),
            content: Set(background_sitemap_index_generation::render_sitemap_index(
                index_urls.as_slice(),
            )),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(txn)
        .await?;
        Ok(())
    }

    async fn record_background_sitemap_submission(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        endpoint_count: i32,
        error: Option<String>,
    ) -> SeoResult<()> {
        let txn = self.db.begin().await?;
        let current = crate::entities::seo_sitemap_job::Entity::find_by_id(job.id)
            .filter(crate::entities::seo_sitemap_job::Column::TenantId.eq(job.tenant_id))
            .one(&txn)
            .await?
            .ok_or(SeoError::NotFound)?;
        let now = chrono::Utc::now().fixed_offset();
        let success = error.is_none();
        let mut active: crate::entities::seo_sitemap_job::ActiveModel = current.into();
        active.status = Set(SITEMAP_JOB_COMPLETED.to_string());
        active.last_error = Set(error.clone());
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&txn).await?;

        let event_type = "seo.sitemap.submitted";
        let idempotency_key = background_sitemap_event_key(
            event_type,
            job.tenant_id,
            &[
                job.id.to_string(),
                endpoint_count.to_string(),
                success.to_string(),
            ],
        );
        self.publish_background_sitemap_event(
            &txn,
            BackgroundSitemapEventPublication {
                tenant_id: job.tenant_id,
                job_id: job.id,
                event_type: event_type.to_string(),
                idempotency_key: idempotency_key.clone(),
                event: rustok_core::DomainEvent::SeoSitemapSubmitted {
                    job_id: job.id,
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

    async fn publish_background_sitemap_event(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        publication: BackgroundSitemapEventPublication,
    ) -> SeoResult<()> {
        let existing = crate::entities::seo_event_delivery::Entity::find()
            .filter(
                crate::entities::seo_event_delivery::Column::TenantId.eq(publication.tenant_id),
            )
            .filter(
                crate::entities::seo_event_delivery::Column::IdempotencyKey
                    .eq(publication.idempotency_key.as_str()),
            )
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(
                txn,
                publication.tenant_id,
                None,
                publication.event,
            )
            .await
            .map_err(|error| background_sitemap_transactional_event_error(error))?;
        crate::entities::seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(publication.tenant_id),
            event_type: Set(publication.event_type),
            idempotency_key: Set(publication.idempotency_key),
            source_kind: Set(Some("sitemap_job".to_string())),
            source_id: Set(Some(publication.job_id)),
            status: Set(SITEMAP_BACKGROUND_DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(publication.occurred_at),
            updated_at: Set(publication.occurred_at),
            dispatched_at: Set(Some(publication.occurred_at)),
        }
        .insert(txn)
        .await?;
        Ok(())
    }

    async fn fail_background_sitemap_job(
        &self,
        job: &crate::entities::seo_sitemap_job::Model,
        message: String,
    ) -> SeoResult<()> {
        let now = chrono::Utc::now().fixed_offset();
        let mut active: crate::entities::seo_sitemap_job::ActiveModel = job.clone().into();
        active.status = Set(SITEMAP_JOB_FAILED.to_string());
        active.last_error = Set(Some(rustok_core::truncate(message.trim(), 2048)));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        active.update(&self.db).await?;
        Ok(())
    }
}

async fn submit_background_sitemap_endpoints(
    endpoints: &[String],
    sitemap_index_url: &str,
) -> Result<(), String> {
    let runtime = background_sitemap_submission_adapters::SitemapSubmissionRuntime::default_with_timeout(
        SITEMAP_BACKGROUND_SUBMIT_TIMEOUT_SECS,
    )?;
    let mut summary =
        background_sitemap_submission_aggregation::SitemapSubmissionSummary::default();
    for endpoint in endpoints {
        let Some(request_url) =
            build_background_sitemap_submission_url(endpoint.as_str(), sitemap_index_url)
        else {
            background_sitemap_submission_aggregation::record_invalid_endpoint(
                &mut summary,
                endpoint.as_str(),
            );
            continue;
        };
        let request = background_sitemap_submission_adapters::SitemapSubmitEndpoint {
            endpoint: endpoint.clone(),
            request_url,
        };
        match runtime.adapter().submit_sitemap_index(request).await {
            Ok(()) => background_sitemap_submission_aggregation::record_submission_success(
                &mut summary,
                endpoint.as_str(),
            ),
            Err(message) => {
                background_sitemap_submission_aggregation::record_submission_failure(
                    &mut summary,
                    endpoint.as_str(),
                    message,
                );
            }
        }
    }

    let telemetry = summary.telemetry_snapshot();
    tracing::debug!(
        success_count = summary.success_count,
        failure_count = summary.failure_count,
        endpoint_status_count = telemetry.endpoint_statuses.len(),
        omitted_endpoint_status_count = telemetry.omitted_endpoint_status_count,
        "background SEO sitemap endpoint submission finished"
    );
    match summary.into_error() {
        Some(message) => Err(message),
        None => Ok(()),
    }
}

fn build_background_sitemap_submission_url(
    endpoint: &str,
    sitemap_index_url: &str,
) -> Option<String> {
    let normalized = endpoint.trim();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("{sitemap_url}") {
        let encoded: String =
            url::form_urlencoded::byte_serialize(sitemap_index_url.as_bytes()).collect();
        let replaced = normalized.replace("{sitemap_url}", encoded.as_str());
        let parsed = url::Url::parse(replaced.as_str()).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        return Some(parsed.to_string());
    }
    let mut parsed = url::Url::parse(normalized).ok()?;
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

fn background_sitemap_file_count(url_count: usize) -> usize {
    if url_count == 0 {
        1
    } else {
        ((url_count - 1) / super::SITEMAP_CHUNK_SIZE) + 2
    }
}

fn background_sitemap_event_key(scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
    let mut payload = format!("{scope}|{tenant_id}");
    for part in parts {
        payload.push('|');
        payload.push_str(part.as_str());
    }
    format!(
        "{scope}:{:016x}",
        rustok_core::simple_hash(payload.as_str())
    )
}

fn background_sitemap_transactional_event_error(error: rustok_core::Error) -> SeoError {
    SeoError::Database(sea_orm::DbErr::Custom(format!(
        "failed to enqueue sitemap event transactionally: {error}"
    )))
}

fn background_disabled_sitemap_status() -> crate::dto::SeoSitemapStatusRecord {
    crate::dto::SeoSitemapStatusRecord {
        enabled: false,
        latest_job_id: None,
        status: None,
        file_count: 0,
        generated_at: None,
        files: Vec::new(),
    }
}

#[cfg(test)]
mod sitemap_background_tests {
    use super::*;

    #[test]
    fn background_file_count_keeps_the_index_and_chunks() {
        assert_eq!(background_sitemap_file_count(0), 1);
        assert_eq!(background_sitemap_file_count(1), 2);
        assert_eq!(
            background_sitemap_file_count(super::SITEMAP_CHUNK_SIZE + 1),
            3
        );
    }

    #[test]
    fn background_job_states_are_phase_distinct() {
        assert_ne!(SITEMAP_JOB_QUEUED, SITEMAP_JOB_RUNNING);
        assert_ne!(SITEMAP_JOB_RUNNING, SITEMAP_JOB_SUBMITTING);
        assert_ne!(SITEMAP_JOB_SUBMITTING, SITEMAP_JOB_COMPLETED);
    }
}
