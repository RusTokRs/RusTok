use std::collections::{BTreeMap, BTreeSet};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use crate::dto::{
    CreateTenantInput, TenantModuleResponse, TenantResponse, ToggleModuleInput, UpdateTenantInput,
};
use crate::entities::tenant::{self, ActiveModel as TenantActiveModel};
use crate::entities::tenant_locale::{self, ActiveModel as TenantLocaleActiveModel};
use crate::entities::tenant_locale_policy_receipt::{
    self, ActiveModel as TenantLocalePolicyReceiptActiveModel,
};
use crate::entities::tenant_module::{self, ActiveModel as TenantModuleActiveModel};
use crate::error::TenantError;
use crate::ports::{
    ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyProjection,
};
use crate::settings_schema::validate_tenant_settings;

pub type TenantResult<T> = Result<T, TenantError>;

pub struct TenantService {
    db: DatabaseConnection,
    event_bus: Option<TransactionalEventBus>,
}

impl TenantService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            event_bus: None,
        }
    }

    pub fn with_event_bus(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus: Some(event_bus),
        }
    }

    #[instrument(skip(self, input), fields(slug = %input.slug))]
    pub async fn create_tenant(&self, input: CreateTenantInput) -> TenantResult<TenantResponse> {
        let txn = self.db.begin().await?;
        if let Some(_existing) = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(&input.slug))
            .one(&txn)
            .await?
        {
            return Err(TenantError::SlugAlreadyExists(input.slug));
        }

        let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
        let id = generate_id();
        let model = TenantActiveModel {
            id: Set(id),
            name: Set(input.name),
            slug: Set(input.slug),
            domain: Set(input.domain),
            settings: Set(serde_json::json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;

        TenantLocaleActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(id),
            locale: Set("en".to_string()),
            name: Set("English".to_string()),
            native_name: Set("English".to_string()),
            is_default: Set(true),
            is_enabled: Set(true),
            fallback_locale: Set(None),
            policy_revision: Set(1),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;

        self.publish_event_in_tx(&txn, id, DomainEvent::TenantCreated { tenant_id: id })
            .await?;

        txn.commit().await?;

        Ok(to_tenant_response(model))
    }

    /// Returns the tenant identified by `input.slug`, creating it when absent.
    ///
    /// Bootstrap and installer flows need idempotent tenant provisioning, while
    /// ordinary tenant creation must continue to reject duplicate slugs through
    /// [`Self::create_tenant`].  Keeping both operations in the tenant owner
    /// avoids host-side copies of the tenant persistence policy.
    #[instrument(skip(self, input), fields(slug = %input.slug))]
    pub async fn ensure_tenant(
        &self,
        input: CreateTenantInput,
    ) -> TenantResult<(TenantResponse, bool)> {
        if let Some(existing) = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(&input.slug))
            .one(&self.db)
            .await?
        {
            return Ok((to_tenant_response(existing), false));
        }

        let tenant = self.create_tenant(input).await?;
        Ok((tenant, true))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn get_tenant(&self, tenant_id: Uuid) -> TenantResult<TenantResponse> {
        let model = tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self), fields(slug = %slug))]
    pub async fn get_tenant_by_slug(&self, slug: &str) -> TenantResult<TenantResponse> {
        let model = tenant::Entity::find()
            .filter(tenant::Column::Slug.eq(slug))
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self), fields(domain = %domain))]
    pub async fn get_tenant_by_domain(&self, domain: &str) -> TenantResult<TenantResponse> {
        let model = tenant::Entity::find()
            .filter(tenant::Column::Domain.eq(domain))
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        Ok(to_tenant_response(model))
    }

    #[instrument(skip(self))]
    pub async fn first_active_tenant(&self) -> TenantResult<TenantResponse> {
        tenant::Entity::find()
            .filter(tenant::Column::IsActive.eq(true))
            .order_by_asc(tenant::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(to_tenant_response)
            .ok_or(TenantError::NotFound)
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn update_tenant(
        &self,
        tenant_id: Uuid,
        input: UpdateTenantInput,
    ) -> TenantResult<TenantResponse> {
        let txn = self.db.begin().await?;

        let existing = tenant::Entity::find_by_id(tenant_id)
            .one(&txn)
            .await?
            .ok_or(TenantError::NotFound)?;

        let now = chrono::Utc::now().into();
        let mut active: tenant::ActiveModel = existing.into();
        if let Some(name) = input.name {
            active.name = Set(name);
        }
        if let Some(domain) = input.domain {
            active.domain = Set(Some(domain));
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        if let Some(settings) = input.settings {
            validate_tenant_settings(&settings)?;
            active.settings = Set(settings);
        }
        active.updated_at = Set(now);

        let model = active.update(&txn).await?;

        self.publish_event_in_tx(&txn, tenant_id, DomainEvent::TenantUpdated { tenant_id })
            .await?;

        txn.commit().await?;

        Ok(to_tenant_response(model))
    }

    pub async fn list_tenants(
        &self,
        page: u64,
        per_page: u64,
    ) -> TenantResult<(Vec<TenantResponse>, u64)> {
        let paginator = tenant::Entity::find().paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let items = models.into_iter().map(to_tenant_response).collect();
        Ok((items, total))
    }

    /// Deprecated low-level tenant override writer.
    ///
    /// Runtime module enable/disable paths must go through the host
    /// `ModuleLifecycleService` so policy resolution, dependency checks, hooks,
    /// and operation journaling stay consistent.
    #[deprecated(
        note = "use the host ModuleLifecycleService for runtime module enable/disable paths"
    )]
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, module_slug = %input.module_slug))]
    pub async fn toggle_module(
        &self,
        tenant_id: Uuid,
        input: ToggleModuleInput,
    ) -> TenantResult<TenantModuleResponse> {
        let txn = self.db.begin().await?;

        tenant::Entity::find_by_id(tenant_id)
            .one(&txn)
            .await?
            .ok_or(TenantError::NotFound)?;

        let existing = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(&input.module_slug))
            .one(&txn)
            .await?;

        let now = chrono::Utc::now().into();
        let module_slug = input.module_slug;
        let enabled = input.enabled;

        let model = match existing {
            Some(m) => {
                let mut active: tenant_module::ActiveModel = m.into();
                active.enabled = Set(enabled);
                active.updated_at = Set(now);
                active.update(&txn).await?
            }
            None => {
                TenantModuleActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    module_slug: Set(module_slug.clone()),
                    enabled: Set(enabled),
                    settings: Set(serde_json::json!({})),
                    created_at: Set(now),
                    updated_at: Set(now),
                }
                .insert(&txn)
                .await?
            }
        };

        self.publish_event_in_tx(
            &txn,
            tenant_id,
            DomainEvent::TenantModuleToggled {
                tenant_id,
                module_slug,
                enabled,
            },
        )
        .await?;

        txn.commit().await?;

        Ok(to_module_response(model))
    }

    pub async fn list_tenant_modules(
        &self,
        tenant_id: Uuid,
    ) -> TenantResult<Vec<TenantModuleResponse>> {
        tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;

        let modules = tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?;

        Ok(modules.into_iter().map(to_module_response).collect())
    }

    /// Returns one tenant module configuration for an owner-module read path.
    ///
    /// This read deliberately does not require an existing tenant row because
    /// callers may use the absence of both records as the default feature
    /// configuration during bootstrap and isolated module tests.
    pub async fn find_tenant_module(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> TenantResult<Option<TenantModuleResponse>> {
        tenant_module::Entity::find()
            .filter(tenant_module::Column::TenantId.eq(tenant_id))
            .filter(tenant_module::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await
            .map(|module| module.map(to_module_response))
            .map_err(Into::into)
    }

    pub(crate) async fn read_locale_policy_owned(
        &self,
        tenant_id: Uuid,
    ) -> TenantResult<TenantLocalePolicyProjection> {
        let tenant = tenant::Entity::find_by_id(tenant_id)
            .one(&self.db)
            .await?
            .ok_or(TenantError::NotFound)?;
        let locales = tenant_locale::Entity::find()
            .filter(tenant_locale::Column::TenantId.eq(tenant_id))
            .order_by_desc(tenant_locale::Column::IsDefault)
            .order_by_asc(tenant_locale::Column::Locale)
            .all(&self.db)
            .await?;

        locale_policy_projection(&tenant, locales)
    }

    pub(crate) async fn replace_locale_policy_owned(
        &self,
        tenant_id: Uuid,
        request: ReplaceTenantLocalePolicyRequest,
        idempotency_key: &str,
    ) -> TenantResult<TenantLocalePolicyProjection> {
        let idempotency_key = idempotency_key.trim();
        if idempotency_key.is_empty() || idempotency_key.len() > 191 {
            return Err(TenantError::InvalidLocalePolicy(
                "idempotency key must contain between 1 and 191 bytes".to_string(),
            ));
        }

        let request = canonicalize_locale_policy_request(request)?;
        let request_hash = locale_policy_request_hash(&request)?;
        let txn = self.db.begin().await?;

        if let Some(receipt) = tenant_locale_policy_receipt::Entity::find_by_id((
            tenant_id,
            idempotency_key.to_string(),
        ))
        .one(&txn)
        .await?
        {
            if receipt.request_hash != request_hash {
                return Err(TenantError::LocalePolicyIdempotencyConflict);
            }
            return serde_json::from_value(receipt.response).map_err(|error| {
                TenantError::LocalePolicyInvariant(format!(
                    "stored idempotency receipt is invalid: {error}"
                ))
            });
        }

        let tenant = tenant::Entity::find_by_id(tenant_id)
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(TenantError::NotFound)?;
        let existing = tenant_locale::Entity::find()
            .filter(tenant_locale::Column::TenantId.eq(tenant_id))
            .order_by_asc(tenant_locale::Column::Locale)
            .all(&txn)
            .await?;
        let actual_revision = existing
            .iter()
            .map(|locale| locale.policy_revision)
            .max()
            .unwrap_or(0);
        if actual_revision != request.expected_revision {
            return Err(TenantError::LocalePolicyConflict {
                expected: request.expected_revision,
                actual: actual_revision,
            });
        }

        let next_revision = actual_revision.checked_add(1).ok_or_else(|| {
            TenantError::LocalePolicyInvariant(
                "tenant locale policy revision overflowed".to_string(),
            )
        })?;
        let default_locale = request
            .locales
            .iter()
            .find(|locale| locale.is_default)
            .expect("validated locale policy has exactly one default")
            .locale
            .clone();
        let previous_enabled = existing
            .iter()
            .filter(|locale| locale.is_enabled)
            .map(|locale| locale.locale.clone())
            .collect::<BTreeSet<_>>();
        let next_enabled = request
            .locales
            .iter()
            .filter(|locale| locale.is_enabled)
            .map(|locale| locale.locale.as_str().to_string())
            .collect::<BTreeSet<_>>();

        tenant_locale::Entity::delete_many()
            .filter(tenant_locale::Column::TenantId.eq(tenant_id))
            .exec(&txn)
            .await?;

        let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
        let locale_models = request
            .locales
            .iter()
            .map(|locale| TenantLocaleActiveModel {
                id: Set(generate_id()),
                tenant_id: Set(tenant_id),
                locale: Set(locale.locale.as_str().to_string()),
                name: Set(locale.name.clone()),
                native_name: Set(locale.native_name.clone()),
                is_default: Set(locale.is_default),
                is_enabled: Set(locale.is_enabled),
                fallback_locale: Set(locale
                    .fallback_locale
                    .as_ref()
                    .map(|fallback| fallback.as_str().to_string())),
                policy_revision: Set(next_revision),
                created_at: Set(now.clone()),
                updated_at: Set(now.clone()),
            })
            .collect::<Vec<_>>();
        tenant_locale::Entity::insert_many(locale_models)
            .exec(&txn)
            .await?;

        let mut active_tenant: tenant::ActiveModel = tenant.into();
        active_tenant.default_locale = Set(default_locale.as_str().to_string());
        active_tenant.updated_at = Set(now.clone());
        active_tenant.update(&txn).await?;

        let projection = TenantLocalePolicyProjection {
            tenant_id,
            revision: next_revision,
            default_locale,
            locales: request.locales.clone(),
        };
        TenantLocalePolicyReceiptActiveModel {
            tenant_id: Set(tenant_id),
            idempotency_key: Set(idempotency_key.to_string()),
            request_hash: Set(request_hash),
            response: Set(serde_json::to_value(&projection).map_err(|error| {
                TenantError::LocalePolicyInvariant(format!(
                    "failed to serialize locale policy receipt: {error}"
                ))
            })?),
            created_at: Set(now),
        }
        .insert(&txn)
        .await?;

        self.publish_event_in_tx(&txn, tenant_id, DomainEvent::TenantUpdated { tenant_id })
            .await?;
        for locale in next_enabled.difference(&previous_enabled) {
            self.publish_event_in_tx(
                &txn,
                tenant_id,
                DomainEvent::LocaleEnabled {
                    tenant_id,
                    locale: locale.clone(),
                },
            )
            .await?;
        }
        for locale in previous_enabled.difference(&next_enabled) {
            self.publish_event_in_tx(
                &txn,
                tenant_id,
                DomainEvent::LocaleDisabled {
                    tenant_id,
                    locale: locale.clone(),
                },
            )
            .await?;
        }

        txn.commit().await?;
        Ok(projection)
    }

    async fn publish_event_in_tx<C>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        event: DomainEvent,
    ) -> TenantResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        if let Some(event_bus) = &self.event_bus {
            event_bus
                .publish_in_tx(txn, tenant_id, None, event)
                .await
                .map_err(|error| TenantError::EventPublish(error.to_string()))?;
        }

        Ok(())
    }
}

fn canonicalize_locale_policy_request(
    mut request: ReplaceTenantLocalePolicyRequest,
) -> TenantResult<ReplaceTenantLocalePolicyRequest> {
    if request.expected_revision < 0 {
        return Err(TenantError::InvalidLocalePolicy(
            "expected_revision cannot be negative".to_string(),
        ));
    }
    if request.locales.is_empty() {
        return Err(TenantError::InvalidLocalePolicy(
            "at least one tenant locale is required".to_string(),
        ));
    }

    for entry in &mut request.locales {
        entry.name = entry.name.trim().to_string();
        entry.native_name = entry.native_name.trim().to_string();
        if entry.name.is_empty()
            || entry.native_name.is_empty()
            || entry.name.chars().count() > 50
            || entry.native_name.chars().count() > 50
        {
            return Err(TenantError::InvalidLocalePolicy(format!(
                "locale {} requires non-empty name/native_name values of at most 50 characters",
                entry.locale
            )));
        }
    }
    request
        .locales
        .sort_by(|left, right| left.locale.cmp(&right.locale));
    validate_locale_policy_entries(&request.locales)?;
    Ok(request)
}

fn validate_locale_policy_entries(entries: &[TenantLocalePolicyEntry]) -> TenantResult<()> {
    let by_locale = entries
        .iter()
        .map(|entry| (entry.locale.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if by_locale.len() != entries.len() {
        return Err(TenantError::InvalidLocalePolicy(
            "tenant locale policy contains duplicate canonical locale tags".to_string(),
        ));
    }

    let defaults = entries
        .iter()
        .filter(|entry| entry.is_default)
        .collect::<Vec<_>>();
    if defaults.len() != 1 {
        return Err(TenantError::InvalidLocalePolicy(
            "tenant locale policy requires exactly one default locale".to_string(),
        ));
    }
    if !defaults[0].is_enabled {
        return Err(TenantError::InvalidLocalePolicy(
            "tenant default locale must be enabled".to_string(),
        ));
    }

    for entry in entries {
        let Some(fallback) = entry.fallback_locale.as_ref() else {
            continue;
        };
        if fallback == &entry.locale {
            return Err(TenantError::InvalidLocalePolicy(format!(
                "locale {} cannot fall back to itself",
                entry.locale
            )));
        }
        let target = by_locale.get(fallback.as_str()).ok_or_else(|| {
            TenantError::InvalidLocalePolicy(format!(
                "fallback locale {} is not part of the tenant policy",
                fallback
            ))
        })?;
        if !target.is_enabled {
            return Err(TenantError::InvalidLocalePolicy(format!(
                "fallback locale {} must be enabled",
                fallback
            )));
        }
    }

    for entry in entries {
        let mut visited = BTreeSet::new();
        let mut cursor = Some(entry.locale.as_str());
        while let Some(locale) = cursor {
            if !visited.insert(locale) {
                return Err(TenantError::InvalidLocalePolicy(format!(
                    "fallback cycle detected at locale {locale}"
                )));
            }
            cursor = by_locale
                .get(locale)
                .and_then(|candidate| candidate.fallback_locale.as_ref())
                .map(|fallback| fallback.as_str());
        }
    }

    Ok(())
}

fn locale_policy_request_hash(request: &ReplaceTenantLocalePolicyRequest) -> TenantResult<String> {
    let encoded = serde_json::to_vec(request).map_err(|error| {
        TenantError::InvalidLocalePolicy(format!("failed to encode locale policy request: {error}"))
    })?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

fn locale_policy_projection(
    tenant: &tenant::Model,
    locales: Vec<tenant_locale::Model>,
) -> TenantResult<TenantLocalePolicyProjection> {
    let default_locale =
        rustok_api::TenantLocale::new(&tenant.default_locale).map_err(|error| {
            TenantError::LocalePolicyInvariant(format!("tenant default locale is invalid: {error}"))
        })?;
    let revision = locales
        .iter()
        .map(|locale| locale.policy_revision)
        .max()
        .unwrap_or(0);
    let entries = locales
        .into_iter()
        .map(|locale| {
            Ok(TenantLocalePolicyEntry {
                locale: rustok_api::TenantLocale::new(locale.locale).map_err(|error| {
                    TenantError::LocalePolicyInvariant(format!(
                        "stored tenant locale is invalid: {error}"
                    ))
                })?,
                name: locale.name,
                native_name: locale.native_name,
                is_default: locale.is_default,
                is_enabled: locale.is_enabled,
                fallback_locale: locale
                    .fallback_locale
                    .map(rustok_api::TenantLocale::new)
                    .transpose()
                    .map_err(|error| {
                        TenantError::LocalePolicyInvariant(format!(
                            "stored tenant fallback locale is invalid: {error}"
                        ))
                    })?,
            })
        })
        .collect::<TenantResult<Vec<_>>>()?;

    if !entries.is_empty() {
        validate_locale_policy_entries(&entries)
            .map_err(|error| TenantError::LocalePolicyInvariant(error.to_string()))?;
        let stored_default = entries
            .iter()
            .find(|entry| entry.is_default)
            .expect("validated locale policy has a default");
        if stored_default.locale != default_locale {
            return Err(TenantError::LocalePolicyInvariant(format!(
                "tenant default locale {} does not match policy default {}",
                default_locale, stored_default.locale
            )));
        }
    }

    Ok(TenantLocalePolicyProjection {
        tenant_id: tenant.id,
        revision,
        default_locale,
        locales: entries,
    })
}

fn to_tenant_response(m: tenant::Model) -> TenantResponse {
    TenantResponse {
        id: m.id,
        name: m.name,
        slug: m.slug,
        domain: m.domain,
        is_active: m.is_active,
        default_locale: m.default_locale,
        settings: m.settings,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn to_module_response(m: tenant_module::Model) -> TenantModuleResponse {
    TenantModuleResponse {
        id: m.id,
        tenant_id: m.tenant_id,
        module_slug: m.module_slug,
        enabled: m.enabled,
        settings: m.settings,
    }
}
