use std::sync::Arc;

use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, Order, QueryFilter,
    QueryOrder, TransactionTrait,
};
use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_core::{simple_hash, DomainEvent};

use crate::dto::{SeoRedirectInput, SeoRedirectMatchType, SeoRedirectRecord};
use crate::entities::{seo_event_delivery, seo_index_cursor, seo_index_delivery, seo_redirect};
use crate::{SeoError, SeoResult};

use super::{normalize_route, SeoService, REDIRECT_CACHE};

const DELIVERY_STATUS_SENT: &str = "sent";
const INDEX_DELIVERY_STATUS_SENT: &str = "sent";
const INDEX_TARGET_SCOPE_KIND: &str = "kind";
const INDEX_SCOPE_KEY_ALL: &str = "*";
const INDEX_CURSOR_REPLAY_MODE_NOT_STARTED: &str = "not_started";

impl SeoService {
    pub async fn list_redirects(&self, tenant_id: Uuid) -> SeoResult<Vec<SeoRedirectRecord>> {
        let items = seo_redirect::Entity::find()
            .filter(seo_redirect::Column::TenantId.eq(tenant_id))
            .order_by(seo_redirect::Column::SourcePattern, Order::Asc)
            .all(&self.db)
            .await?;
        Ok(items.into_iter().map(map_redirect_record).collect())
    }

    pub async fn upsert_redirect(
        &self,
        tenant: &TenantContext,
        input: SeoRedirectInput,
    ) -> SeoResult<SeoRedirectRecord> {
        let settings = self.load_settings(tenant.id).await?;
        let source_pattern =
            normalize_source_pattern(input.source_pattern.as_str(), input.match_type)?;
        validate_target_url(
            input.target_url.as_str(),
            settings.allowed_redirect_hosts.as_slice(),
            "target_url",
        )?;
        let status_code = normalize_redirect_status(input.status_code)?;
        let now = Utc::now().fixed_offset();
        let transition_id = Uuid::new_v4();
        let txn = self.db.begin().await?;

        let model = if let Some(id) = input.id {
            let Some(existing) = seo_redirect::Entity::find_by_id(id)
                .filter(seo_redirect::Column::TenantId.eq(tenant.id))
                .one(&txn)
                .await?
            else {
                return Err(SeoError::NotFound);
            };
            let mut active: seo_redirect::ActiveModel = existing.into();
            active.match_type = Set(input.match_type.as_str().to_string());
            active.source_pattern = Set(source_pattern);
            active.target_url = Set(input.target_url);
            active.status_code = Set(status_code);
            active.expires_at = Set(input.expires_at.map(|value| value.into()));
            active.is_active = Set(input.is_active);
            active.updated_at = Set(now);
            active.update(&txn).await?
        } else {
            seo_redirect::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.id),
                match_type: Set(input.match_type.as_str().to_string()),
                source_pattern: Set(source_pattern),
                target_url: Set(input.target_url),
                status_code: Set(status_code),
                expires_at: Set(input.expires_at.map(|value| value.into())),
                is_active: Set(input.is_active),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&txn)
            .await?
        };

        let record = map_redirect_record(model);
        self.publish_redirect_transition_in_tx(&txn, tenant.id, transition_id, &record)
            .await?;
        txn.commit().await?;

        REDIRECT_CACHE.invalidate(&tenant.id).await;
        Ok(record)
    }

    async fn publish_redirect_transition_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        transition_id: Uuid,
        record: &SeoRedirectRecord,
    ) -> SeoResult<()> {
        let (event_type, idempotency_key, event) =
            redirect_domain_event(tenant_id, transition_id, record);
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
            .map_err(|error| transactional_event_error("redirect event", error))?;
        let now = Utc::now().fixed_offset();

        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_type.to_string()),
            idempotency_key: Set(idempotency_key.clone()),
            source_kind: Set(Some("redirect".to_string())),
            source_id: Set(Some(record.id)),
            status: Set(DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(Some(now)),
        }
        .insert(txn)
        .await?;

        for target_type in ["content", "product"] {
            self.publish_redirect_reindex_in_tx(
                txn,
                tenant_id,
                event_type,
                idempotency_key.as_str(),
                target_type,
                now,
            )
            .await?;
        }

        Ok(())
    }

    async fn publish_redirect_reindex_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        seo_event_type: &str,
        idempotency_key: &str,
        target_type: &str,
        observed_at: chrono::DateTime<chrono::FixedOffset>,
    ) -> SeoResult<()> {
        let existing = seo_index_delivery::Entity::find()
            .filter(seo_index_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_index_delivery::Column::IdempotencyKey.eq(idempotency_key))
            .filter(seo_index_delivery::Column::TargetType.eq(target_type))
            .filter(seo_index_delivery::Column::TargetScopeKey.eq(INDEX_SCOPE_KEY_ALL))
            .one(txn)
            .await?;
        if existing.is_some() {
            upsert_index_cursor_in_tx(txn, tenant_id, target_type, observed_at).await?;
            return Ok(());
        }

        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(
                txn,
                tenant_id,
                None,
                DomainEvent::ReindexRequested {
                    target_type: target_type.to_string(),
                    target_id: None,
                },
            )
            .await
            .map_err(|error| transactional_event_error("redirect reindex event", error))?;

        seo_index_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            seo_event_type: Set(seo_event_type.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            target_type: Set(target_type.to_string()),
            target_id: Set(None),
            target_scope: Set(INDEX_TARGET_SCOPE_KIND.to_string()),
            target_scope_key: Set(INDEX_SCOPE_KEY_ALL.to_string()),
            status: Set(INDEX_DELIVERY_STATUS_SENT.to_string()),
            attempt_count: Set(1),
            outbox_event_id: Set(Some(outbox_event_id)),
            next_attempt_at: Set(None),
            last_error: Set(None),
            dead_lettered_at: Set(None),
            created_at: Set(observed_at),
            updated_at: Set(observed_at),
            dispatched_at: Set(Some(observed_at)),
        }
        .insert(txn)
        .await?;

        upsert_index_cursor_in_tx(txn, tenant_id, target_type, observed_at).await
    }

    pub(super) async fn load_redirect_models(
        &self,
        tenant_id: Uuid,
    ) -> SeoResult<Arc<Vec<seo_redirect::Model>>> {
        REDIRECT_CACHE
            .try_get_with(tenant_id, async {
                let items = seo_redirect::Entity::find()
                    .filter(seo_redirect::Column::TenantId.eq(tenant_id))
                    .all(&self.db)
                    .await?;
                Ok::<_, sea_orm::DbErr>(Arc::new(items))
            })
            .await
            .map_err(|error| {
                SeoError::Database(sea_orm::DbErr::Custom(format!(
                    "SEO redirect cache load failed: {error}"
                )))
            })
    }

    pub(super) async fn match_redirect(
        &self,
        tenant_id: Uuid,
        route: &str,
    ) -> SeoResult<Option<seo_redirect::Model>> {
        let now = Utc::now().fixed_offset();
        let redirects = self.load_redirect_models(tenant_id).await?;
        if let Some(exact) = redirects.iter().find(|item| {
            item.is_active
                && item
                    .expires_at
                    .map(|expires_at| expires_at > now)
                    .unwrap_or(true)
                && item.match_type == SeoRedirectMatchType::Exact.as_str()
                && item.source_pattern == route
        }) {
            return Ok(Some(exact.clone()));
        }

        Ok(redirects
            .iter()
            .find(|item| {
                item.is_active
                    && item
                        .expires_at
                        .map(|expires_at| expires_at > now)
                        .unwrap_or(true)
                    && item.match_type == SeoRedirectMatchType::Wildcard.as_str()
                    && wildcard_matches(item.source_pattern.as_str(), route)
            })
            .cloned())
    }
}

async fn upsert_index_cursor_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    target_type: &str,
    observed_at: chrono::DateTime<chrono::FixedOffset>,
) -> SeoResult<()> {
    let existing = seo_index_cursor::Entity::find()
        .filter(seo_index_cursor::Column::TenantId.eq(tenant_id))
        .filter(seo_index_cursor::Column::TargetType.eq(target_type))
        .one(txn)
        .await?;

    if let Some(existing) = existing {
        if existing.high_water_mark_at >= observed_at {
            return Ok(());
        }

        let mut active: seo_index_cursor::ActiveModel = existing.into();
        active.high_water_mark_at = Set(observed_at);
        active.updated_at = Set(observed_at);
        active.update(txn).await?;
        return Ok(());
    }

    seo_index_cursor::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        target_type: Set(target_type.to_string()),
        initial_cursor_at: Set(observed_at),
        high_water_mark_at: Set(observed_at),
        last_repair_cursor_at: Set(None),
        replay_mode: Set(INDEX_CURSOR_REPLAY_MODE_NOT_STARTED.to_string()),
        replay_requested_at: Set(None),
        replay_completed_at: Set(None),
        created_at: Set(observed_at),
        updated_at: Set(observed_at),
    }
    .insert(txn)
    .await?;

    Ok(())
}

fn redirect_domain_event(
    tenant_id: Uuid,
    transition_id: Uuid,
    record: &SeoRedirectRecord,
) -> (&'static str, String, DomainEvent) {
    if record.is_active {
        let event_type = "seo.redirect.upserted";
        let idempotency_key = build_seo_event_key(
            event_type,
            tenant_id,
            &[
                transition_id.to_string(),
                record.id.to_string(),
                record.source_pattern.clone(),
                record.target_url.clone(),
                record.status_code.to_string(),
                record.is_active.to_string(),
            ],
        );
        let event = DomainEvent::SeoRedirectUpserted {
            redirect_id: record.id,
            source_pattern: record.source_pattern.clone(),
            target_url: record.target_url.clone(),
            status_code: record.status_code,
            is_active: record.is_active,
            idempotency_key: idempotency_key.clone(),
        };
        (event_type, idempotency_key, event)
    } else {
        let event_type = "seo.redirect.disabled";
        let idempotency_key = build_seo_event_key(
            event_type,
            tenant_id,
            &[
                transition_id.to_string(),
                record.id.to_string(),
                record.source_pattern.clone(),
            ],
        );
        let event = DomainEvent::SeoRedirectDisabled {
            redirect_id: record.id,
            source_pattern: record.source_pattern.clone(),
            idempotency_key: idempotency_key.clone(),
        };
        (event_type, idempotency_key, event)
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

fn transactional_event_error(context: &str, error: rustok_core::Error) -> SeoError {
    SeoError::Database(sea_orm::DbErr::Custom(format!(
        "failed to enqueue {context} transactionally: {error}"
    )))
}

pub(super) fn normalize_hosts(hosts: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in hosts {
        let host = value
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty() || normalized.iter().any(|item| item == &host) {
            continue;
        }
        normalized.push(host);
    }
    normalized
}

pub(super) fn validate_target_url(
    value: &str,
    allowed_hosts: &[String],
    field: &str,
) -> SeoResult<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(SeoError::validation(format!("{field} must not be empty")));
    }
    if trimmed.starts_with('/') {
        return normalize_route(trimmed).map(|_| ());
    }

    let parsed = Url::parse(trimmed)
        .map_err(|_| SeoError::validation(format!("{field} must be a valid URL")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(SeoError::validation(format!(
            "{field} scheme must be HTTP or HTTPS"
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(SeoError::validation(format!(
            "{field} URL credentials are not allowed"
        )));
    }
    let host = parsed
        .host_str()
        .map(|value| value.trim_end_matches('.').to_ascii_lowercase())
        .ok_or_else(|| SeoError::validation(format!("{field} must contain a host")))?;
    if allowed_hosts.iter().any(|item| item == &host) {
        Ok(())
    } else {
        Err(SeoError::validation(format!(
            "{field} host `{host}` is not allowed"
        )))
    }
}

pub(super) fn normalize_source_pattern(
    value: &str,
    match_type: SeoRedirectMatchType,
) -> SeoResult<String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('/') {
        return Err(SeoError::validation("source_pattern must start with `/`"));
    }
    if matches!(match_type, SeoRedirectMatchType::Wildcard) {
        if trimmed.matches('*').count() > 1 {
            return Err(SeoError::validation(
                "wildcard redirects support only one `*` token",
            ));
        }
        return Ok(trimmed.to_string());
    }
    normalize_route(trimmed)
}

pub(super) fn normalize_redirect_status(status_code: i32) -> SeoResult<i32> {
    match status_code {
        301 | 302 | 307 | 308 => Ok(status_code),
        _ => Err(SeoError::validation(
            "status_code must be one of 301, 302, 307, 308",
        )),
    }
}

pub(super) fn wildcard_matches(pattern: &str, route: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == route;
    };
    route.starts_with(prefix) && route.ends_with(suffix)
}

pub(super) fn map_redirect_record(model: seo_redirect::Model) -> SeoRedirectRecord {
    SeoRedirectRecord {
        id: model.id,
        match_type: SeoRedirectMatchType::parse(model.match_type.as_str())
            .unwrap_or(SeoRedirectMatchType::Exact),
        source_pattern: model.source_pattern,
        target_url: model.target_url,
        status_code: model.status_code,
        expires_at: model.expires_at.map(Into::into),
        is_active: model.is_active,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_seo_event_key, normalize_hosts, validate_target_url};
    use uuid::Uuid;

    #[test]
    fn redirect_event_keys_are_transition_sensitive() {
        let tenant_id = Uuid::new_v4();
        let redirect_id = Uuid::new_v4();
        let first_transition = Uuid::new_v4();
        let second_transition = Uuid::new_v4();
        let first = build_seo_event_key(
            "seo.redirect.upserted",
            tenant_id,
            &[
                first_transition.to_string(),
                redirect_id.to_string(),
                "/old".to_string(),
                "/new".to_string(),
                "301".to_string(),
                "true".to_string(),
            ],
        );
        let repeated_state = build_seo_event_key(
            "seo.redirect.upserted",
            tenant_id,
            &[
                second_transition.to_string(),
                redirect_id.to_string(),
                "/old".to_string(),
                "/new".to_string(),
                "301".to_string(),
                "true".to_string(),
            ],
        );

        assert_ne!(first, repeated_state);
        assert!(first.starts_with("seo.redirect.upserted:"));
    }

    #[test]
    fn redirect_target_urls_require_http_and_no_credentials() {
        let allowed_hosts = normalize_hosts(&["Allowed.Example.".to_string()]);

        assert!(validate_target_url("https://allowed.example/path", &allowed_hosts, "target_url").is_ok());
        assert!(validate_target_url("javascript://allowed.example/path", &allowed_hosts, "target_url").is_err());
        assert!(validate_target_url("https://user:secret@allowed.example/path", &allowed_hosts, "target_url").is_err());
    }
}
