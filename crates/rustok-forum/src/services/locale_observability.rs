use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use prometheus::{IntCounterVec, Opts};
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::{
    CategoryCursorPage, CategoryCursorQuery, ReplyCursorPage, ReplyCursorQuery, TopicCursorPage,
    TopicCursorQuery, TopicUnreadCursorPage, TopicUnreadCursorQuery, TopicUnreadSummaryReadModel,
};
use crate::error::ForumResult;

use super::read_model::ForumReadModelService as ForumReadModelOwnerService;

static FORUM_LOCALE_RESOLUTION_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();
static FORUM_LOCALE_RESOLUTION_REGISTERING: AtomicBool = AtomicBool::new(false);

const RESOURCE_CATEGORY: &str = "category";
const RESOURCE_TOPIC: &str = "topic";
const RESOURCE_REPLY: &str = "reply";
const OUTCOME_EXACT: &str = "exact";
const OUTCOME_FALLBACK: &str = "fallback";
const OUTCOME_MISSING: &str = "missing";

fn locale_resolution_counter() -> Option<&'static IntCounterVec> {
    let counter = FORUM_LOCALE_RESOLUTION_TOTAL.get_or_init(|| {
        IntCounterVec::new(
            Opts::new(
                "rustok_forum_read_model_locale_resolution_total",
                "Forum localized read-model items by fixed resource kind and locale resolution outcome",
            ),
            &["resource", "outcome"],
        )
        .expect("Forum locale resolution metric must use a valid fixed descriptor")
    });

    if FORUM_LOCALE_RESOLUTION_REGISTERING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
        && let Err(_error) = rustok_telemetry::register_runtime_collector(Box::new(counter.clone()))
    {
        // Telemetry can be initialized after owner services are constructed. Keep the
        // already accumulated counter and allow a later observation to retry registration.
        FORUM_LOCALE_RESOLUTION_REGISTERING.store(false, Ordering::Release);
        return None;
    }

    Some(counter)
}

fn record_locale_resolution(
    resource: &'static str,
    requested_locale: &str,
    effective_locale: &str,
    available_locale_count: usize,
) {
    let outcome = if available_locale_count == 0 {
        OUTCOME_MISSING
    } else if requested_locale == effective_locale {
        OUTCOME_EXACT
    } else {
        OUTCOME_FALLBACK
    };

    if let Some(counter) = locale_resolution_counter() {
        counter.with_label_values(&[resource, outcome]).inc();
    }
}

fn observe_category_page(page: &CategoryCursorPage) {
    for item in &page.items {
        record_locale_resolution(
            RESOURCE_CATEGORY,
            &item.requested_locale,
            &item.effective_locale,
            item.available_locales.len(),
        );
    }
}

fn observe_topic_page(page: &TopicCursorPage) {
    for item in &page.items {
        record_locale_resolution(
            RESOURCE_TOPIC,
            &item.requested_locale,
            &item.effective_locale,
            item.available_locales.len(),
        );
    }
}

fn observe_unread_topic_page(page: &TopicUnreadCursorPage) {
    for item in &page.items {
        record_locale_resolution(
            RESOURCE_TOPIC,
            &item.topic.requested_locale,
            &item.topic.effective_locale,
            item.topic.available_locales.len(),
        );
    }
}

fn observe_reply_page(page: &ReplyCursorPage) {
    for item in &page.items {
        record_locale_resolution(
            RESOURCE_REPLY,
            &item.requested_locale,
            &item.effective_locale,
            item.available_locales.len(),
        );
    }
}

/// Public Forum read-model service with fixed-cardinality locale resolution observability.
///
/// The owner service remains authoritative for selection, localization, fallback semantics,
/// cursors, RBAC and persistence. This wrapper records the already-materialized result only and
/// never changes the page, retries a query or reads additional owner state.
pub struct ForumObservedReadModelService {
    inner: ForumReadModelOwnerService,
}

impl ForumObservedReadModelService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: ForumReadModelOwnerService::new(db),
        }
    }

    pub async fn list_categories(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: CategoryCursorQuery,
    ) -> ForumResult<CategoryCursorPage> {
        let page = self.inner.list_categories(tenant_id, security, query).await?;
        observe_category_page(&page);
        Ok(page)
    }

    pub async fn list_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: TopicCursorQuery,
    ) -> ForumResult<TopicCursorPage> {
        let page = self.inner.list_topics(tenant_id, security, query).await?;
        observe_topic_page(&page);
        Ok(page)
    }

    pub async fn list_topics_with_unread(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        query: TopicUnreadCursorQuery,
    ) -> ForumResult<TopicUnreadCursorPage> {
        let page = self
            .inner
            .list_topics_with_unread(tenant_id, security, query)
            .await?;
        observe_unread_topic_page(&page);
        Ok(page)
    }

    pub async fn summarize_topic_ids(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: Vec<Uuid>,
    ) -> ForumResult<Vec<TopicUnreadSummaryReadModel>> {
        self.inner
            .summarize_topic_ids(tenant_id, security, topic_ids)
            .await
    }

    pub async fn list_replies(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        query: ReplyCursorQuery,
    ) -> ForumResult<ReplyCursorPage> {
        let page = self
            .inner
            .list_replies(tenant_id, security, topic_id, query)
            .await?;
        observe_reply_page(&page);
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OUTCOME_EXACT, OUTCOME_FALLBACK, OUTCOME_MISSING, record_locale_resolution,
    };

    #[test]
    fn locale_outcome_labels_remain_fixed() {
        assert_eq!(OUTCOME_EXACT, "exact");
        assert_eq!(OUTCOME_FALLBACK, "fallback");
        assert_eq!(OUTCOME_MISSING, "missing");

        // Source-only contract: none of these values becomes a metric label.
        record_locale_resolution("topic", "tenant-secret-locale", "different-secret", 1);
    }
}
