use uuid::Uuid;

use rustok_core::{simple_hash, DomainEvent};

use super::SeoService;

#[allow(clippy::too_many_arguments)]
fn seo_bulk_terminal_event(
    job_id: Uuid,
    target_kind: &str,
    locale: &str,
    status: &str,
    processed_count: i32,
    succeeded_count: i32,
    failed_count: i32,
    idempotency_key: String,
) -> DomainEvent {
    match status {
        "partial" => DomainEvent::SeoBulkPartial {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
        "failed" => DomainEvent::SeoBulkFailed {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
        _ => DomainEvent::SeoBulkCompleted {
            job_id,
            target_kind: target_kind.to_string(),
            locale: locale.to_string(),
            status: status.to_string(),
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        },
    }
}

fn build_seo_event_key(scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
    let mut payload = format!("{scope}|{tenant_id}");
    for part in parts {
        payload.push('|');
        payload.push_str(part.as_str());
    }
    format!("{scope}:{:016x}", simple_hash(payload.as_str()))
}

impl SeoService {
    pub(super) async fn publish_seo_meta_upserted_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        locale: &str,
        source: &str,
        transition_ref: Option<&str>,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.meta.upserted",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                locale.to_string(),
                transition_ref.unwrap_or("direct").to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoMetaUpserted {
                target_kind: target_kind.to_string(),
                target_id,
                locale: locale.to_string(),
                source: source.to_string(),
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_revision_published_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.revision.published",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                revision.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRevisionPublished {
                target_kind: target_kind.to_string(),
                target_id,
                revision,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_revision_rolled_back_event(
        &self,
        tenant_id: Uuid,
        target_kind: &str,
        target_id: Uuid,
        revision: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.revision.rolled_back",
            tenant_id,
            &[
                target_kind.to_string(),
                target_id.to_string(),
                revision.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRevisionRolledBack {
                target_kind: target_kind.to_string(),
                target_id,
                revision,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_redirect_upserted_event(
        &self,
        tenant_id: Uuid,
        redirect_id: Uuid,
        source_pattern: &str,
        target_url: &str,
        status_code: i32,
        is_active: bool,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.redirect.upserted",
            tenant_id,
            &[
                redirect_id.to_string(),
                source_pattern.to_string(),
                target_url.to_string(),
                status_code.to_string(),
                is_active.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRedirectUpserted {
                redirect_id,
                source_pattern: source_pattern.to_string(),
                target_url: target_url.to_string(),
                status_code,
                is_active,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_redirect_disabled_event(
        &self,
        tenant_id: Uuid,
        redirect_id: Uuid,
        source_pattern: &str,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.redirect.disabled",
            tenant_id,
            &[redirect_id.to_string(), source_pattern.to_string()],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoRedirectDisabled {
                redirect_id,
                source_pattern: source_pattern.to_string(),
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_sitemap_generated_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        file_count: i32,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.sitemap.generated",
            tenant_id,
            &[job_id.to_string(), file_count.to_string()],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoSitemapGenerated {
                job_id,
                file_count,
                idempotency_key,
            },
        )
        .await;
    }

    pub(super) async fn publish_seo_sitemap_submitted_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        endpoint_count: i32,
        success: bool,
        error: Option<String>,
    ) {
        let idempotency_key = self.build_event_key(
            "seo.sitemap.submitted",
            tenant_id,
            &[
                job_id.to_string(),
                endpoint_count.to_string(),
                success.to_string(),
            ],
        );
        self.publish_seo_event(
            tenant_id,
            DomainEvent::SeoSitemapSubmitted {
                job_id,
                endpoint_count,
                success,
                error,
                idempotency_key,
            },
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn publish_seo_bulk_completed_event(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        target_kind: &str,
        locale: &str,
        status: &str,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
    ) {
        let event_scope = match status {
            "partial" => "seo.bulk.partial",
            "failed" => "seo.bulk.failed",
            _ => "seo.bulk.completed",
        };
        let idempotency_key = self.build_event_key(
            event_scope,
            tenant_id,
            &[
                target_kind.to_string(),
                locale.to_string(),
                job_id.to_string(),
                status.to_string(),
                processed_count.to_string(),
                succeeded_count.to_string(),
                failed_count.to_string(),
            ],
        );
        let event = seo_bulk_terminal_event(
            job_id,
            target_kind,
            locale,
            status,
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key,
        );
        self.publish_seo_event(tenant_id, event).await;
    }

    async fn publish_seo_event(&self, tenant_id: Uuid, event: DomainEvent) {
        if let Err(error) = self.event_bus.publish(tenant_id, None, event.clone()).await {
            tracing::warn!(
                tenant_id = %tenant_id,
                event_type = event.event_type(),
                error = %error,
                "failed to publish SEO domain event"
            );
        }
    }

    fn build_event_key(&self, scope: &str, tenant_id: Uuid, parts: &[String]) -> String {
        build_seo_event_key(scope, tenant_id, parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seo_bulk_terminal_event_uses_status_specific_variants() {
        let job_id = Uuid::from_u128(42);

        let completed = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "completed",
            3,
            3,
            0,
            "completed-key".to_string(),
        );
        assert!(matches!(completed, DomainEvent::SeoBulkCompleted { .. }));
        assert_eq!(completed.event_type(), "seo.bulk.completed");

        let partial = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "partial",
            3,
            2,
            1,
            "partial-key".to_string(),
        );
        assert!(matches!(partial, DomainEvent::SeoBulkPartial { .. }));
        assert_eq!(partial.event_type(), "seo.bulk.partial");

        let failed = seo_bulk_terminal_event(
            job_id,
            "product",
            "en-US",
            "failed",
            3,
            0,
            3,
            "failed-key".to_string(),
        );
        assert!(matches!(failed, DomainEvent::SeoBulkFailed { .. }));
        assert_eq!(failed.event_type(), "seo.bulk.failed");
    }

    #[test]
    fn seo_event_keys_are_deterministic_and_scope_sensitive() {
        let tenant_id = Uuid::from_u128(7);
        let completed = build_seo_event_key(
            "seo.bulk.completed",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "completed".to_string(),
            ],
        );
        let repeated = build_seo_event_key(
            "seo.bulk.completed",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "completed".to_string(),
            ],
        );
        let partial = build_seo_event_key(
            "seo.bulk.partial",
            tenant_id,
            &[
                "product".to_string(),
                "en-US".to_string(),
                "job-1".to_string(),
                "partial".to_string(),
            ],
        );

        assert_eq!(completed, repeated);
        assert_ne!(completed, partial);
        assert!(completed.starts_with("seo.bulk.completed:"));
        assert!(partial.starts_with("seo.bulk.partial:"));
    }
}
