use chrono::Utc;
use rustok_api::normalize_locale_tag;
use rustok_commerce_foundation::entities;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
};
use std::collections::HashSet;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use crate::domain::region::{
    normalize_countries, normalize_country_code, normalize_currency_code, normalize_tax_provider_id,
};
use crate::dto::{
    CreateRegionInput, RegionCountryTaxPolicyInput, RegionResponse, RegionTranslationInput,
    UpdateRegionInput,
};
use crate::error::{RegionError, RegionResult};
use crate::translation_changes::record_current_region_translation_change_in_tx;

use super::RegionService;
use super::repository::{
    insert_translations, load_region, replace_country_tax_policies, replace_translations,
};

impl RegionService {
    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_region(
        &self,
        tenant_id: Uuid,
        input: CreateRegionInput,
    ) -> RegionResult<RegionResponse> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        if input.tax_rate < rust_decimal::Decimal::ZERO
            || input.tax_rate > rust_decimal::Decimal::from(100)
        {
            return Err(RegionError::Validation(
                "tax_rate must be between 0 and 100".to_string(),
            ));
        }

        let currency_code = normalize_currency_code(&input.currency_code)?;
        let countries = normalize_countries(input.countries)?;
        let tax_provider_id = normalize_tax_provider_id(input.tax_provider_id.as_deref())?;
        let country_tax_policies =
            normalize_country_tax_policies(input.country_tax_policies.unwrap_or_default())?;

        for policy in &country_tax_policies {
            if !countries.contains(&policy.country_code) {
                return Err(RegionError::Validation(format!(
                    "country tax policy targets `{}` which is not in region countries",
                    policy.country_code
                )));
            }
        }

        let now = Utc::now();
        let region_id = generate_id();
        let translations = normalize_translation_inputs(input.translations)?;
        let txn = self.db.begin().await?;

        entities::region::ActiveModel {
            id: Set(region_id),
            tenant_id: Set(tenant_id),
            currency_code: Set(currency_code),
            tax_provider_id: Set(tax_provider_id),
            tax_rate: Set(input.tax_rate),
            tax_included: Set(input.tax_included),
            countries: Set(serde_json::to_value(&countries)
                .map_err(|error| RegionError::Validation(error.to_string()))?),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        insert_translations(&txn, region_id, &translations).await?;
        replace_country_tax_policies(&txn, region_id, &country_tax_policies).await?;
        record_current_region_translation_change_in_tx(&txn, tenant_id, region_id, generate_id())
            .await?;
        // Materialize the response before commit so a projection failure
        // rolls back the region, related rows, and translation journal.
        let response = load_region(&txn, tenant_id, region_id, None, None).await?;
        txn.commit().await?;
        Ok(response)
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn update_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        input: UpdateRegionInput,
    ) -> RegionResult<RegionResponse> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if region_id.is_nil() {
            return Err(RegionError::Validation(
                "region_id must not be nil".to_string(),
            ));
        }

        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        if let Some(rate) = input.tax_rate
            && (rate < rust_decimal::Decimal::ZERO || rate > rust_decimal::Decimal::from(100))
        {
            return Err(RegionError::Validation(
                "tax_rate must be between 0 and 100".to_string(),
            ));
        }

        let normalized_country_tax_policies = input
            .country_tax_policies
            .map(normalize_country_tax_policies)
            .transpose()?;
        let normalized_countries = input.countries.map(normalize_countries).transpose()?;
        let normalized_translations = input
            .translations
            .map(normalize_translation_inputs)
            .transpose()?;
        let translation_copy_changed = normalized_translations.is_some();
        let txn = self.db.begin().await?;
        let existing = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;

        let existing_countries = serde_json::from_value::<Vec<String>>(existing.countries.clone())
            .map_err(|error| RegionError::Validation(error.to_string()))?;
        let effective_countries = normalized_countries.as_ref().unwrap_or(&existing_countries);

        if let Some(policies) = &normalized_country_tax_policies {
            for policy in policies {
                if !effective_countries.contains(&policy.country_code) {
                    return Err(RegionError::Validation(format!(
                        "country tax policy targets `{}` which is not in region countries",
                        policy.country_code
                    )));
                }
            }
        } else if let Some(new_countries) = &normalized_countries {
            let existing_policies = entities::region_country_tax_policy::Entity::find()
                .filter(entities::region_country_tax_policy::Column::RegionId.eq(region_id))
                .all(&txn)
                .await?;
            for policy in existing_policies {
                if !new_countries.contains(&policy.country_code) {
                    return Err(RegionError::Validation(format!(
                        "region countries cannot exclude existing country tax policy for `{}` without providing updated country tax policies",
                        policy.country_code
                    )));
                }
            }
        }

        // If country tax policies are being replaced, delete old ones before updating regions
        // to prevent triggering the region exclusion guard constraint.
        if normalized_country_tax_policies.is_some() {
            entities::region_country_tax_policy::Entity::delete_many()
                .filter(entities::region_country_tax_policy::Column::RegionId.eq(region_id))
                .exec(&txn)
                .await?;
        }

        let mut active: entities::region::ActiveModel = existing.into();
        if let Some(currency_code) = input.currency_code {
            active.currency_code = Set(normalize_currency_code(&currency_code)?);
        }
        if let Some(tax_provider_id) = input.tax_provider_id {
            active.tax_provider_id = Set(normalize_tax_provider_id(tax_provider_id.as_deref())?);
        }
        if let Some(tax_rate) = input.tax_rate {
            active.tax_rate = Set(tax_rate);
        }
        if let Some(tax_included) = input.tax_included {
            active.tax_included = Set(tax_included);
        }
        if let Some(countries) = normalized_countries {
            active.countries = Set(serde_json::to_value(countries)
                .map_err(|error| RegionError::Validation(error.to_string()))?);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;

        if let Some(country_tax_policies) = normalized_country_tax_policies {
            for policy in country_tax_policies {
                entities::region_country_tax_policy::ActiveModel {
                    id: Set(generate_id()),
                    region_id: Set(region_id),
                    country_code: Set(policy.country_code),
                    tax_rate: Set(policy.tax_rate),
                    tax_included: Set(policy.tax_included),
                }
                .insert(&txn)
                .await?;
            }
        }
        if let Some(translations) = normalized_translations {
            replace_translations(&txn, region_id, &translations).await?;
        }
        if translation_copy_changed {
            record_current_region_translation_change_in_tx(
                &txn,
                tenant_id,
                region_id,
                generate_id(),
            )
            .await?;
        }
        // Materialize the response before commit so a projection failure
        // rolls back the region, related rows, and translation journal.
        let response = load_region(&txn, tenant_id, region_id, None, None).await?;
        txn.commit().await?;
        Ok(response)
    }
}

fn normalize_country_tax_policies(
    policies: Vec<RegionCountryTaxPolicyInput>,
) -> RegionResult<Vec<RegionCountryTaxPolicyInput>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(policies.len());
    for policy in policies {
        let country_code = normalize_country_code(&policy.country_code)?;
        if !seen.insert(country_code.clone()) {
            return Err(RegionError::Validation(
                "Duplicate country_code in region country tax policies".to_string(),
            ));
        }
        if policy.tax_rate < rust_decimal::Decimal::ZERO
            || policy.tax_rate > rust_decimal::Decimal::from(100)
        {
            return Err(RegionError::Validation(
                "country tax policy tax_rate must be between 0 and 100".to_string(),
            ));
        }
        normalized.push(RegionCountryTaxPolicyInput {
            country_code,
            tax_rate: policy.tax_rate,
            tax_included: policy.tax_included,
        });
    }
    normalized.sort_by(|left, right| left.country_code.cmp(&right.country_code));
    Ok(normalized)
}

fn normalize_translation_inputs(
    translations: Vec<RegionTranslationInput>,
) -> RegionResult<Vec<RegionTranslationInput>> {
    if translations.is_empty() {
        return Err(RegionError::Validation(
            "At least one translation is required".to_string(),
        ));
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(translations.len());
    for translation in translations {
        let locale = normalize_locale_tag(&translation.locale)
            .ok_or_else(|| RegionError::Validation("Invalid locale".to_string()))?;
        if !seen.insert(locale.clone()) {
            return Err(RegionError::Validation(
                "Duplicate locale in region translations".to_string(),
            ));
        }
        let name = translation.name.trim();
        if name.is_empty() {
            return Err(RegionError::Validation(
                "Region name cannot be empty".to_string(),
            ));
        }
        normalized.push(RegionTranslationInput {
            locale,
            name: name.to_string(),
        });
    }
    Ok(normalized)
}
