use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;
use rustok_profiles::ProfilesReader;

use crate::dto::{
    CreateCustomerInput, CustomerResponse, CustomerWithProfileResponse, ListCustomersInput,
    UpdateCustomerInput,
};
use crate::entities;
use crate::error::{CustomerError, CustomerResult};

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
        input: CreateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
        input
            .validate()
            .map_err(|error| CustomerError::Validation(error.to_string()))?;

        self.ensure_email_available(tenant_id, &input.email, None)
            .await?;
        if let Some(user_id) = input.user_id {
            self.ensure_user_available(tenant_id, user_id, None).await?;
        }

        let customer_id = generate_id();
        let now = Utc::now();

        entities::customer::ActiveModel {
            id: Set(customer_id),
            tenant_id: Set(tenant_id),
            user_id: Set(input.user_id),
            email: Set(input.email.trim().to_string()),
            first_name: Set(normalize_optional_text(input.first_name)),
            last_name: Set(normalize_optional_text(input.last_name)),
            phone: Set(normalize_optional_text(input.phone)),
            locale: Set(normalize_optional_text(input.locale)),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        self.get_customer(tenant_id, customer_id).await
    }

    pub async fn get_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
    ) -> CustomerResult<CustomerResponse> {
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
        let customer = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(entities::customer::Column::UserId.eq(user_id))
            .one(&self.db)
            .await?
            .ok_or(CustomerError::CustomerByUserNotFound(user_id))?;
        Ok(map_customer(customer))
    }

    pub async fn list_customers(
        &self,
        tenant_id: Uuid,
        input: ListCustomersInput,
    ) -> CustomerResult<(Vec<CustomerResponse>, u64)> {
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
            query = query.filter(
                Condition::any()
                    .add(entities::customer::Column::Email.contains(search))
                    .add(entities::customer::Column::FirstName.contains(search))
                    .add(entities::customer::Column::LastName.contains(search))
                    .add(entities::customer::Column::Phone.contains(search)),
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

    pub async fn update_customer(
        &self,
        tenant_id: Uuid,
        customer_id: Uuid,
        input: UpdateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
        input
            .validate()
            .map_err(|error| CustomerError::Validation(error.to_string()))?;

        let customer = entities::customer::Entity::find_by_id(customer_id)
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CustomerError::CustomerNotFound(customer_id))?;

        if let Some(email) = input.email.as_deref() {
            self.ensure_email_available(tenant_id, email, Some(customer_id))
                .await?;
        }

        let mut active: entities::customer::ActiveModel = customer.into();
        if let Some(email) = input.email {
            active.email = Set(email.trim().to_string());
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
        if let Some(locale) = input.locale {
            active.locale = Set(normalize_text(locale));
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_customer(tenant_id, customer_id).await
    }

    pub async fn upsert_customer_for_user(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        input: CreateCustomerInput,
    ) -> CustomerResult<CustomerResponse> {
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
        let existing = entities::customer::Entity::find()
            .filter(entities::customer::Column::TenantId.eq(tenant_id))
            .filter(entities::customer::Column::Email.eq(email))
            .one(&self.db)
            .await?;
        if let Some(existing) = existing {
            if Some(existing.id) != except_customer_id {
                return Err(CustomerError::DuplicateEmail(email.to_string()));
            }
        }
        Ok(())
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
        if let Some(existing) = existing {
            if Some(existing.id) != except_customer_id {
                return Err(CustomerError::DuplicateUserLink(user_id));
            }
        }
        Ok(())
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(normalize_text)
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
