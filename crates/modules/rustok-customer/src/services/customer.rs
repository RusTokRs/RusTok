use chrono::Utc;
use sea_orm::sea_query::{Expr, Func};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, SqlErr,
};
use std::collections::HashSet;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_api::normalize_locale_tag;
use rustok_core::generate_id;
use rustok_profiles::ProfilesReader;

use crate::dto::{
    CreateCustomerInput, CustomerResponse, CustomerWithProfileResponse, ListCustomersInput,
    UpdateCustomerInput,
};
use crate::entities;
use crate::error::{CustomerError, CustomerResult};

#[derive(Clone, Debug)]
pub struct CustomerService {
    db: DatabaseConnection,
}

impl CustomerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_customer(
        &self,
        tenant_id: Uuid,
        mut input: CreateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        input.email = input.email.trim().to_string();
        input
            .validate()
            .map_err(|error| CustomerError::Validation(error.to_string()))?;

        let email = input.email.clone();
        let locale = normalize_and_validate_locale(input.locale)?;
        self.ensure_email_available(tenant_id, &email, None).await?;
        if let Some(user_id) = input.user_id {
            if user_id.is_nil() {
                return Err(CustomerError::Validation(
                    "user_id must not be nil".to_string(),
                ));
            }
            self.ensure_user_available(tenant_id, user_id, None).await?;
        }

        let customer_id = generate_id();
        let now = Utc::now();

        if let Err(err) = (entities::customer::ActiveModel {
            id: Set(customer_id),
            tenant_id: Set(tenant_id),
            user_id: Set(input.user_id),
            email: Set(email.clone()),
            first_name: Set(normalize_optional_text(input.first_name)),
            last_name: Set(normalize_optional_text(input.last_name)),
            phone: Set(normalize_optional_text(input.phone)),
            locale: Set(locale),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await)
        {
            return Err(self
                .map_write_db_error(tenant_id, Some(&email), input.user_id, None, err)
                .await);
        }

        self.get_customer(tenant_id, customer_id).await
    }

    pub async fn get_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
    ) -> CustomerResult<CustomerResponse> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if customer_id.is_nil() {
            return Err(CustomerError::CustomerNotFound(customer_id));
        }
        let customer = entities::customer::Entity::find_by_id(customer_id)
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CustomerError::CustomerNotFound(customer_id))?;
        Ok(map_customer(customer))
    }

    pub async fn get_customer_by_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> CustomerResult<CustomerResponse> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if user_id.is_nil() {
            return Err(CustomerError::CustomerByUserNotFound(user_id));
        }
        let customer = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(entities::customer::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or(CustomerError::CustomerByUserNotFound(user_id))?;
        Ok(map_customer(customer))
    }

    pub async fn list_profile_enrichment(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
    ) -> CustomerResult<Vec<crate::CustomerProfileEnrichment>> {
        if tenant_id.is_nil() || user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = HashSet::new();
        let valid_user_ids: Vec<Uuid> = user_ids
            .iter()
            .copied()
            .filter(|id| !id.is_nil() && seen.insert(*id))
            .take(500)
            .collect();
        if valid_user_ids.is_empty() {
            return Ok(Vec::new());
        }

        entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(entities::customer::Column::UserId.is_in(valid_user_ids))
            .all(&self.db)
            .await
            .map(|customers| {
                customers
                    .into_iter()
                    .filter_map(|customer| {
                        customer
                            .user_id
                            .map(|user_id| crate::CustomerProfileEnrichment {
                                user_id,
                                first_name: customer.first_name,
                                last_name: customer.last_name,
                                preferred_locale: customer.locale,
                            })
                    })
                    .collect()
            })
            .map_err(Into::into)
    }

    pub async fn list_customers(
        &self,
        tenant_id: Uuid,
        input: ListCustomersInput,
    ) -> CustomerResult<(Vec<CustomerResponse>, u64)> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);

        let mut query = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id));

        if let Some(search) = input
            .search
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let bounded_search = match search.char_indices().nth(256) {
                Some((idx, _)) => &search[..idx],
                None => search,
            };
            query = query.filter(
                Condition::any()
                    .add(entities::customer::Column::Email.contains(bounded_search))
                    .add(entities::customer::Column::FirstName.contains(bounded_search))
                    .add(entities::customer::Column::LastName.contains(bounded_search))
                    .add(entities::customer::Column::Phone.contains(bounded_search)),
            );
        }

        let paginator = query
            .order_by_desc(entities::customer::Column::UpdatedAt)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_customer)
            .collect();

        Ok((items, total))
    }

    pub async fn get_customer_with_profile<R: ProfilesReader>(
        &self,
        reader: &R,
        tenant_id: Uuid,
        customer_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CustomerResult<CustomerWithProfileResponse> {
        let customer = self.get_customer(tenant_id, customer_id).await?;
        let profile = load_customer_profile(
            reader,
            tenant_id,
            customer.user_id,
            requested_locale,
            tenant_default_locale,
        )
        .await?;

        Ok(CustomerWithProfileResponse { customer, profile })
    }

    pub async fn get_customer_by_user_with_profile<R: ProfilesReader>(
        &self,
        reader: &R,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CustomerResult<CustomerWithProfileResponse> {
        let customer = self.get_customer_by_user(tenant_id, user_id).await?;
        let profile = load_customer_profile(
            reader,
            tenant_id,
            customer.user_id,
            requested_locale,
            tenant_default_locale,
        )
        .await?;

        Ok(CustomerWithProfileResponse { customer, profile })
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, customer_id = %customer_id))]
    pub async fn update_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        input: UpdateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if customer_id.is_nil() {
            return Err(CustomerError::CustomerNotFound(customer_id));
        }
        let mut input = input;
        if let Some(email) = input.email.as_deref() {
            input.email = Some(email.trim().to_string());
        }
        input
            .validate()
            .map_err(|error| CustomerError::Validation(error.to_string()))?;

        let customer = entities::customer::Entity::find_by_id(customer_id)
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CustomerError::CustomerNotFound(customer_id))?;

        let normalized_email = input.email.as_deref().map(str::trim).map(str::to_string);
        if let Some(email) = normalized_email.as_deref() {
            self.ensure_email_available(tenant_id, email, Some(customer_id))
                .await?;
        }
        let normalized_locale = match input.locale {
            Some(locale) => Some(normalize_and_validate_locale(Some(locale))?),
            None => None,
        };

        let mut active: entities::customer::ActiveModel = customer.into();
        if let Some(email) = normalized_email.as_ref() {
            active.email = Set(email.clone());
        }
        if let Some(first_name) = input.first_name {
            active.first_name = Set(normalize_text(first_name));
        }
        if let Some(last_name) = input.last_name {
            active.last_name = Set(normalize_text(last_name));
        }
        if let Some(phone) = input.phone {
            active.phone = Set(normalize_text(phone));
        }
        if let Some(locale) = normalized_locale {
            active.locale = Set(locale);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        if let Err(err) = active.update(&self.db).await {
            return Err(self
                .map_write_db_error(
                    tenant_id,
                    normalized_email.as_deref(),
                    None,
                    Some(customer_id),
                    err,
                )
                .await);
        }

        self.get_customer(tenant_id, customer_id).await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, user_id = %user_id))]
    pub async fn upsert_customer_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        input: CreateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
        if tenant_id.is_nil() {
            return Err(CustomerError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if user_id.is_nil() {
            return Err(CustomerError::Validation(
                "user_id must not be nil".to_string(),
            ));
        }
        match self.get_customer_by_user(tenant_id, user_id).await {
            Ok(existing) => {
                self.update_customer(
                    tenant_id,
                    existing.id,
                    UpdateCustomerInput {
                        email: Some(input.email),
                        first_name: input.first_name,
                        last_name: input.last_name,
                        phone: input.phone,
                        locale: input.locale,
                        metadata: Some(input.metadata),
                    },
                )
                .await
            }
            Err(CustomerError::CustomerByUserNotFound(_)) => {
                self.create_customer(
                    tenant_id,
                    CreateCustomerInput {
                        user_id: Some(user_id),
                        ..input
                    },
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    async fn ensure_email_available(
        &self,
        tenant_id: Uuid,
        email: &str,
        except_customer_id: Option<Uuid>,
    ) -> CustomerResult<()> {
        use sea_orm::sea_query::ExprTrait;

        let existing = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(
                Expr::expr(Func::lower(Expr::col(entities::customer::Column::Email)))
                    .eq(email.to_lowercase()),
            )
            .one(&self.db)
            .await?;
        if let Some(existing) = existing
            && Some(existing.id) != except_customer_id
        {
            return Err(CustomerError::DuplicateEmail(email.to_string()));
        }
        Ok(())
    }

    async fn map_write_db_error(
        &self,
        tenant_id: Uuid,
        email: Option<&str>,
        user_id: Option<Uuid>,
        except_customer_id: Option<Uuid>,
        err: DbErr,
    ) -> CustomerError {
        if !matches!(err.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
            return CustomerError::Database(err);
        }
        if let Some(email) = email
            && let Err(mapped) = self
                .ensure_email_available(tenant_id, email, except_customer_id)
                .await
        {
            return mapped;
        }
        if let Some(user_id) = user_id
            && let Err(mapped) = self
                .ensure_user_available(tenant_id, user_id, except_customer_id)
                .await
        {
            return mapped;
        }
        let message = err.to_string();
        if let Some(email) = email
            && (message.contains("email") || message.contains("ux_customers_tenant_email_canonical"))
        {
            return CustomerError::DuplicateEmail(email.to_string());
        }
        if let Some(user_id) = user_id
            && (message.contains("user") || message.contains("idx_customers_tenant_user"))
        {
            return CustomerError::DuplicateUserLink(user_id);
        }
        CustomerError::Database(err)
    }

    async fn ensure_user_available(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        except_customer_id: Option<Uuid>,
    ) -> CustomerResult<()> {
        let existing = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(entities::customer::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?;
        if let Some(existing) = existing
            && Some(existing.id) != except_customer_id
        {
            return Err(CustomerError::DuplicateUserLink(user_id));
        }
        Ok(())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_text)
}

fn normalize_and_validate_locale(value: Option<String>) -> CustomerResult<Option<String>> {
    let Some(trimmed) = normalize_optional_text(value) else {
        return Ok(None);
    };
    normalize_locale_tag(&trimmed)
        .map(Some)
        .ok_or_else(|| CustomerError::Validation("locale must be a valid BCP-47 tag".to_string()))
}

fn normalize_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn map_customer(customer: entities::customer::Model) -> CustomerResponse {
    CustomerResponse {
        id: customer.id,
        tenant_id: customer.tenant_id,
        user_id: customer.user_id,
        email: customer.email,
        first_name: customer.first_name,
        last_name: customer.last_name,
        phone: customer.phone,
        locale: customer.locale,
        metadata: customer.metadata,
        created_at: customer.created_at.with_timezone(&Utc),
        updated_at: customer.updated_at.with_timezone(&Utc),
    }
}

async fn load_customer_profile<R: ProfilesReader>(
    reader: &R,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> CustomerResult<Option<rustok_profiles::ProfileSummary>> {
    let Some(user_id) = user_id else {
        return Ok(None);
    };

    reader
        .find_profile_summary(tenant_id, user_id, requested_locale, tenant_default_locale)
        .await
        .map_err(CustomerError::from)
}
