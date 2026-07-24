use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use moka::future::Cache;
use once_cell::sync::Lazy;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, Order, QueryFilter,
    QueryOrder, TransactionTrait,
};
use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_core::{DomainEvent, simple_hash};

use crate::dto::{SeoRedirectInput, SeoRedirectMatchType, SeoRedirectRecord};
use crate::entities::{seo_event_delivery, seo_index_cursor, seo_index_delivery, seo_redirect};
use crate::{SeoError, SeoResult};

use super::{
    REDIRECT_CACHE, REDIRECT_CACHE_MAX_WEIGHT_BYTES, REDIRECT_CACHE_TTL_SECS, SeoService,
    normalize_route,
};

const DELIVERY_STATUS_SENT: &str = "sent";
const INDEX_DELIVERY_STATUS_SENT: &str = "sent";
const INDEX_TARGET_SCOPE_KIND: &str = "kind";
const INDEX_SCOPE_KEY_ALL: &str = "*";
const INDEX_CURSOR_REPLAY_MODE_NOT_STARTED: &str = "not_started";

static REDIRECT_LOOKUP_CACHE: Lazy<Cache<Uuid, Arc<RedirectLookup>>> = Lazy::new(|| {
    Cache::builder()
        .time_to_live(Duration::from_secs(REDIRECT_CACHE_TTL_SECS))
        .weigher(redirect_lookup_cache_entry_weight)
        .max_capacity(REDIRECT_CACHE_MAX_WEIGHT_BYTES)
        .build()
});

#[derive(Debug)]
struct RedirectLookup {
    source: Arc<Vec<seo_redirect::Model>>,
    exact: HashMap<String, usize>,
    wildcard_literals: HashMap<String, usize>,
    wildcard_prefix_lengths: Vec<usize>,
    wildcards: HashMap<String, RedirectWildcardBucket>,
}

#[derive(Debug, Default)]
struct RedirectWildcardBucket {
    suffix_lengths: Vec<usize>,
    suffixes: HashMap<String, usize>,
}

impl RedirectLookup {
    fn from_source(source: Arc<Vec<seo_redirect::Model>>) -> Self {
        let mut exact = HashMap::new();
        let mut wildcard_literals = HashMap::new();
        let mut wildcard_prefix_lengths = Vec::new();
        let mut wildcards: HashMap<String, RedirectWildcardBucket> = HashMap::new();

        for (source_index, redirect) in source.iter().enumerate() {
            if redirect.match_type == SeoRedirectMatchType::Exact.as_str() {
                exact
                    .entry(redirect.source_pattern.clone())
                    .or_insert(source_index);
                continue;
            }
            if redirect.match_type != SeoRedirectMatchType::Wildcard.as_str() {
                continue;
            }

            let Some((prefix, suffix)) = redirect.source_pattern.split_once('*') else {
                wildcard_literals
                    .entry(redirect.source_pattern.clone())
                    .or_insert(source_index);
                continue;
            };

            wildcard_prefix_lengths.push(prefix.len());
            let bucket = wildcards.entry(prefix.to_string()).or_default();
            bucket.suffix_lengths.push(suffix.len());
            bucket
                .suffixes
                .entry(suffix.to_string())
                .or_insert(source_index);
        }

        wildcard_prefix_lengths.sort_unstable();
        wildcard_prefix_lengths.dedup();
        for bucket in wildcards.values_mut() {
            bucket.suffix_lengths.sort_unstable();
            bucket.suffix_lengths.dedup();
        }

        Self {
            source,
            exact,
            wildcard_literals,
            wildcard_prefix_lengths,
            wildcards,
        }
    }

    fn matches(&self, route: &str, now: DateTime<FixedOffset>) -> Option<seo_redirect::Model> {
        if let Some(source_index) = self.exact.get(route).copied() {
            let redirect = &self.source[source_index];
            if redirect_is_live(redirect, &now) {
                return Some(redirect.clone());
            }
        }

        let mut wildcard_match = self
            .wildcard_literals
            .get(route)
            .copied()
            .filter(|source_index| redirect_is_live(&self.source[*source_index], &now));

        for prefix_len in &self.wildcard_prefix_lengths {
            if *prefix_len > route.len() || !route.is_char_boundary(*prefix_len) {
                continue;
            }
            let Some(bucket) = self.wildcards.get(&route[..*prefix_len]) else {
                continue;
            };

            for suffix_len in &bucket.suffix_lengths {
                if *suffix_len > route.len() {
                    continue;
                }
                let suffix_start = route.len() - *suffix_len;
                if !route.is_char_boundary(suffix_start) {
                    continue;
                }
                let Some(source_index) = bucket.suffixes.get(&route[suffix_start..]).copied()
                else {
                    continue;
                };
                if !redirect_is_live(&self.source[source_index], &now) {
                    continue;
                }
                if wildcard_match
                    .map(|current| source_index < current)
                    .unwrap_or(true)
                {
                    wildcard_match = Some(source_index);
                }
            }
        }

        wildcard_match.map(|source_index| self.source[source_index].clone())
    }
}

fn redirect_is_live(redirect: &seo_redirect::Model, now: &DateTime<FixedOffset>) -> bool {
    redirect.is_active
        && redirect
            .expires_at
            .as_ref()
            .map(|expires_at| expires_at > now)
            .unwrap_or(true)
}

fn redirect_lookup_cache_entry_weight(_tenant_id: &Uuid, lookup: &Arc<RedirectLookup>) -> u32 {
    let mut weight = std::mem::size_of::<Uuid>()
        .saturating_add(std::mem::size_of::<Arc<RedirectLookup>>())
        .saturating_add(std::mem::size_of::<RedirectLookup>())
        .saturating_add(std::mem::size_of::<Arc<Vec<seo_redirect::Model>>>());

    for route in lookup.exact.keys() {
        weight = weight
            .saturating_add(std::mem::size_of::<String>())
            .saturating_add(route.len())
            .saturating_add(std::mem::size_of::<usize>());
    }
    for route in lookup.wildcard_literals.keys() {
        weight = weight
            .saturating_add(std::mem::size_of::<String>())
            .saturating_add(route.len())
            .saturating_add(std::mem::size_of::<usize>());
    }
    weight = weight.saturating_add(
        lookup
            .wildcard_prefix_lengths
            .len()
            .saturating_mul(std::mem::size_of::<usize>()),
    );
    for (prefix, bucket) in &lookup.wildcards {
        weight = weight
            .saturating_add(std::mem::size_of::<String>())
            .saturating_add(prefix.len())
            .saturating_add(std::mem::size_of::<RedirectWildcardBucket>())
            .saturating_add(
                bucket
                    .suffix_lengths
                    .len()
                    .saturating_mul(std::mem::size_of::<usize>()),
            );
        for suffix in bucket.suffixes.keys() {
            weight = weight
                .saturating_add(std::mem::size_of::<String>())
                .saturating_add(suffix.len())
                .saturating_add(std::mem::size_of::<usize>());
        }
    }

    weight.clamp(1, u32::MAX as usize) as u32
}

pub(super) async fn invalidate_redirect_lookup_cache(tenant_id: Uuid) {
    REDIRECT_LOOKUP_CACHE.invalidate(&tenant_id).await;
}

pub(super) async fn invalidate_all_redirect_lookup_cache() {
    REDIRECT_LOOKUP_CACHE.invalidate_all();
    REDIRECT_LOOKUP_CACHE.run_pending_tasks().await;
}

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

        super::invalidate_redirect_cache(tenant.id).await;
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

    async fn load_redirect_lookup(&self, tenant_id: Uuid) -> SeoResult<Arc<RedirectLookup>> {
        let source = self.load_redirect_models(tenant_id).await?;
        if let Some(lookup) = REDIRECT_LOOKUP_CACHE.get(&tenant_id).await {
            if Arc::ptr_eq(&lookup.source, &source) {
                return Ok(lookup);
            }
        }

        let lookup = Arc::new(RedirectLookup::from_source(source));
        REDIRECT_LOOKUP_CACHE
            .insert(tenant_id, Arc::clone(&lookup))
            .await;
        Ok(lookup)
    }

    pub(super) async fn match_redirect(
        &self,
        tenant_id: Uuid,
        route: &str,
    ) -> SeoResult<Option<seo_redirect::Model>> {
        let lookup = self.load_redirect_lookup(tenant_id).await?;
        Ok(lookup.matches(route, Utc::now().fixed_offset()))
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
    use std::sync::Arc;

    use chrono::{Duration as ChronoDuration, Utc};
    use uuid::Uuid;

    use super::{
        RedirectLookup, build_seo_event_key, normalize_hosts, validate_target_url,
    };
    use crate::entities::seo_redirect;

    fn redirect(
        match_type: &str,
        source_pattern: &str,
        target_url: &str,
        is_active: bool,
        expires_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> seo_redirect::Model {
        let now = Utc::now().fixed_offset();
        seo_redirect::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            match_type: match_type.to_string(),
            source_pattern: source_pattern.to_string(),
            target_url: target_url.to_string(),
            status_code: 301,
            expires_at,
            is_active,
            created_at: now,
            updated_at: now,
        }
    }

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

        assert!(
            validate_target_url(
                "https://allowed.example/path",
                &allowed_hosts,
                "target_url"
            )
            .is_ok()
        );
        assert!(
            validate_target_url(
                "javascript://allowed.example/path",
                &allowed_hosts,
                "target_url"
            )
            .is_err()
        );
        assert!(
            validate_target_url(
                "https://user:secret@allowed.example/path",
                &allowed_hosts,
                "target_url"
            )
            .is_err()
        );
    }

    #[test]
    fn redirect_lookup_prefers_exact_over_wildcard() {
        let lookup = RedirectLookup::from_source(Arc::new(vec![
            redirect("wildcard", "/docs/*", "/wildcard", true, None),
            redirect("exact", "/docs/start", "/exact", true, None),
        ]));

        let matched = lookup
            .matches("/docs/start", Utc::now().fixed_offset())
            .expect("exact redirect should match");

        assert_eq!(matched.target_url, "/exact");
    }

    #[test]
    fn redirect_lookup_preserves_first_matching_wildcard_order() {
        let lookup = RedirectLookup::from_source(Arc::new(vec![
            redirect("wildcard", "/docs/*", "/broad", true, None),
            redirect("wildcard", "/docs/guides/*", "/specific", true, None),
        ]));

        let matched = lookup
            .matches("/docs/guides/start", Utc::now().fixed_offset())
            .expect("wildcard redirect should match");

        assert_eq!(matched.target_url, "/broad");
    }

    #[test]
    fn redirect_lookup_skips_inactive_and_expired_candidates() {
        let now = Utc::now().fixed_offset();
        let lookup = RedirectLookup::from_source(Arc::new(vec![
            redirect("wildcard", "/docs/*", "/inactive", false, None),
            redirect(
                "wildcard",
                "/docs/guides/*",
                "/expired",
                true,
                Some(now - ChronoDuration::seconds(1)),
            ),
            redirect(
                "wildcard",
                "/docs/guides/*/start",
                "/active",
                true,
                None,
            ),
        ]));

        let matched = lookup
            .matches("/docs/guides/current/start", now)
            .expect("live wildcard redirect should match");

        assert_eq!(matched.target_url, "/active");
    }

    #[test]
    fn wildcard_without_token_remains_literal() {
        let lookup = RedirectLookup::from_source(Arc::new(vec![redirect(
            "wildcard",
            "/literal",
            "/target",
            true,
            None,
        )]));
        let now = Utc::now().fixed_offset();

        assert!(lookup.matches("/literal", now).is_some());
        assert!(lookup.matches("/literal/child", now).is_none());
    }

    #[test]
    fn redirect_lookup_handles_unicode_prefix_and_suffix_boundaries() {
        let lookup = RedirectLookup::from_source(Arc::new(vec![redirect(
            "wildcard",
            "/каталог/*/обзор",
            "/target",
            true,
            None,
        )]));

        let matched = lookup
            .matches("/каталог/товар/обзор", Utc::now().fixed_offset())
            .expect("unicode wildcard redirect should match");

        assert_eq!(matched.target_url, "/target");
    }
}
