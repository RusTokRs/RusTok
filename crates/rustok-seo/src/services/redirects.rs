use std::sync::Arc;

use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder};
use url::Url;
use uuid::Uuid;

use rustok_api::TenantContext;

use crate::dto::{SeoRedirectInput, SeoRedirectMatchType, SeoRedirectRecord};
use crate::entities::seo_redirect;
use crate::{SeoError, SeoResult};

use super::{normalize_route, SeoService, REDIRECT_CACHE};

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

        let model = if let Some(id) = input.id {
            let Some(existing) = seo_redirect::Entity::find_by_id(id)
                .filter(seo_redirect::Column::TenantId.eq(tenant.id))
                .one(&self.db)
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
            active.update(&self.db).await?
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
            .insert(&self.db)
            .await?
        };

        REDIRECT_CACHE.invalidate(&tenant.id).await;
        Ok(map_redirect_record(model))
    }

    pub(super) async fn load_redirect_models(
        &self,
        tenant_id: Uuid,
    ) -> SeoResult<Arc<Vec<seo_redirect::Model>>> {
        if let Some(cached) = REDIRECT_CACHE.get(&tenant_id).await {
            return Ok(cached);
        }

        let items = seo_redirect::Entity::find()
            .filter(seo_redirect::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?;
        let items = Arc::new(items);
        REDIRECT_CACHE.insert(tenant_id, items.clone()).await;
        Ok(items)
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

pub(super) fn normalize_hosts(hosts: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in hosts {
        let host = value.trim().to_ascii_lowercase();
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
    let host = parsed
        .host_str()
        .map(|value| value.to_ascii_lowercase())
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
