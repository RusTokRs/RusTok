use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_commerce_foundation::entities;
use rustok_core::{generate_id, normalize_locale_tag};

use crate::dto::{
    CreateRegionInput, RegionCountryTaxPolicyInput, RegionCountryTaxPolicyResponse, RegionResponse,
    RegionTranslationInput, RegionTranslationResponse, UpdateRegionInput,
};
use crate::error::{RegionError, RegionResult};

pub struct RegionService {
    db: DatabaseConnection,
}

impl RegionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_region(
        &self,
        tenant_id: Uuid,
        input: CreateRegionInput,
    ) -> RegionResult<RegionResponse> {
        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        let currency_code = normalize_currency_code(&input.currency_code)?;
        let countries = normalize_countries(input.countries)?;
        let tax_provider_id = normalize_tax_provider_id(input.tax_provider_id.as_deref())?;
        let country_tax_policies =
            normalize_country_tax_policies(input.country_tax_policies.unwrap_or_default())?;
        let now = Utc::now();
        let region_id = generate_id();
        let translations = normalize_translation_inputs(input.translations)?;

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
        .insert(&self.db)
        .await?;

        insert_translations(&self.db, region_id, &translations).await?;
        replace_country_tax_policies(&self.db, region_id, &country_tax_policies).await?;

        self.get_region(tenant_id, region_id, None, None).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn get_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<RegionResponse> {
        let model = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;
        let items = load_regions_with_translations(
            &self.db,
            vec![model],
            requested_locale,
            tenant_default_locale,
        )
        .await?;
        items
            .into_iter()
            .next()
            .ok_or(RegionError::RegionNotFound(region_id))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn list_regions(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<Vec<RegionResponse>> {
        let rows = entities::region::Entity::find()
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::region::Column::CreatedAt)
            .all(&self.db)
            .await?;

        load_regions_with_translations(&self.db, rows, requested_locale, tenant_default_locale)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn update_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        input: UpdateRegionInput,
    ) -> RegionResult<RegionResponse> {
        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        let existing = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;

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
        if let Some(country_tax_policies) = input.country_tax_policies {
            let normalized = normalize_country_tax_policies(country_tax_policies)?;
            replace_country_tax_policies(&self.db, region_id, &normalized).await?;
        }
        if let Some(countries) = input.countries {
            active.countries = Set(serde_json::to_value(normalize_countries(countries)?)
                .map_err(|error| RegionError::Validation(error.to_string()))?);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        if let Some(translations) = input.translations {
            let normalized = normalize_translation_inputs(translations)?;
            replace_translations(&self.db, region_id, &normalized).await?;
        }

        self.get_region(tenant_id, region_id, None, None).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, country_code = %country_code))]
    pub async fn resolve_region_for_country(
        &self,
        tenant_id: Uuid,
        country_code: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<Option<RegionResponse>> {
        let normalized_country = normalize_country_code(country_code)?;
        let regions = self
            .list_regions(tenant_id, requested_locale, tenant_default_locale)
            .await?;
        Ok(regions.into_iter().find(|region| {
            region
                .countries
                .iter()
                .any(|country| country.eq_ignore_ascii_case(&normalized_country))
        }))
    }
}

async fn load_regions_with_translations(
    db: &DatabaseConnection,
    rows: Vec<entities::region::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> RegionResult<Vec<RegionResponse>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let translations = entities::region_translation::Entity::find()
        .filter(entities::region_translation::Column::RegionId.is_in(ids.clone()))
        .all(db)
        .await?;
    let country_tax_policies = entities::region_country_tax_policy::Entity::find()
        .filter(entities::region_country_tax_policy::Column::RegionId.is_in(ids.clone()))
        .all(db)
        .await?;

    let mut translations_by_region: HashMap<Uuid, Vec<entities::region_translation::Model>> =
        HashMap::new();
    for translation in translations {
        translations_by_region
            .entry(translation.region_id)
            .or_default()
            .push(translation);
    }
    let mut country_tax_policies_by_region: HashMap<
        Uuid,
        Vec<entities::region_country_tax_policy::Model>,
    > = HashMap::new();
    for policy in country_tax_policies {
        country_tax_policies_by_region
            .entry(policy.region_id)
            .or_default()
            .push(policy);
    }

    rows.into_iter()
        .map(|row| {
            let translations = translations_by_region.remove(&row.id).unwrap_or_default();
            let country_tax_policies = country_tax_policies_by_region
                .remove(&row.id)
                .unwrap_or_default();
            map_region(
                row,
                translations,
                country_tax_policies,
                requested_locale,
                tenant_default_locale,
            )
        })
        .collect()
}

fn map_region(
    model: entities::region::Model,
    translations: Vec<entities::region_translation::Model>,
    country_tax_policies: Vec<entities::region_country_tax_policy::Model>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> RegionResult<RegionResponse> {
    let countries = serde_json::from_value::<Vec<String>>(model.countries)
        .map_err(|error| RegionError::Validation(error.to_string()))?;
    let available_locales = translations
        .iter()
        .map(|translation| translation.locale.clone())
        .collect::<Vec<_>>();
    let requested_locale = requested_locale
        .and_then(normalize_locale_tag)
        .filter(|value| !value.is_empty());
    let (resolved, effective_locale) = resolve_translation(
        &translations,
        requested_locale.as_deref(),
        tenant_default_locale,
    );

    let name = resolved
        .map(|translation| translation.name.clone())
        .unwrap_or_default();

    Ok(RegionResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        name,
        currency_code: model.currency_code,
        tax_provider_id: model.tax_provider_id,
        tax_rate: model.tax_rate,
        tax_included: model.tax_included,
        country_tax_policies: country_tax_policies
            .into_iter()
            .map(|policy| RegionCountryTaxPolicyResponse {
                country_code: policy.country_code,
                tax_rate: policy.tax_rate,
                tax_included: policy.tax_included,
            })
            .collect(),
        countries,
        metadata: model.metadata,
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
        requested_locale,
        effective_locale,
        available_locales,
        translations: translations
            .into_iter()
            .map(|translation| RegionTranslationResponse {
                locale: translation.locale,
                name: translation.name,
            })
            .collect(),
    })
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
        if policy.tax_rate < rust_decimal::Decimal::ZERO {
            return Err(RegionError::Validation(
                "country tax policy tax_rate must be zero or greater".to_string(),
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

async fn insert_translations(
    db: &DatabaseConnection,
    region_id: Uuid,
    translations: &[RegionTranslationInput],
) -> RegionResult<()> {
    for translation in translations {
        entities::region_translation::ActiveModel {
            id: Set(generate_id()),
            region_id: Set(region_id),
            locale: Set(translation.locale.clone()),
            name: Set(translation.name.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn replace_translations(
    db: &DatabaseConnection,
    region_id: Uuid,
    translations: &[RegionTranslationInput],
) -> RegionResult<()> {
    entities::region_translation::Entity::delete_many()
        .filter(entities::region_translation::Column::RegionId.eq(region_id))
        .exec(db)
        .await?;
    insert_translations(db, region_id, translations).await
}

async fn replace_country_tax_policies(
    db: &DatabaseConnection,
    region_id: Uuid,
    policies: &[RegionCountryTaxPolicyInput],
) -> RegionResult<()> {
    entities::region_country_tax_policy::Entity::delete_many()
        .filter(entities::region_country_tax_policy::Column::RegionId.eq(region_id))
        .exec(db)
        .await?;
    for policy in policies {
        entities::region_country_tax_policy::ActiveModel {
            id: Set(generate_id()),
            region_id: Set(region_id),
            country_code: Set(policy.country_code.clone()),
            tax_rate: Set(policy.tax_rate),
            tax_included: Set(policy.tax_included),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

fn resolve_translation<'a>(
    translations: &'a [entities::region_translation::Model],
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> (
    Option<&'a entities::region_translation::Model>,
    Option<String>,
) {
    let mut lookup = HashMap::new();
    for translation in translations {
        if let Some(normalized) = normalize_locale_tag(&translation.locale) {
            lookup.insert(normalized, translation);
        }
    }

    if let Some(locale) = requested_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some(found.locale.clone()));
        }
    }
    if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag) {
        if let Some(found) = lookup.get(&locale) {
            return (Some(*found), Some(found.locale.clone()));
        }
    }
    translations
        .first()
        .map(|item| (Some(item), Some(item.locale.clone())))
        .unwrap_or((None, None))
}

fn normalize_currency_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 3 {
        Ok(normalized)
    } else {
        Err(RegionError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ))
    }
}

fn normalize_tax_provider_id(value: Option<&str>) -> RegionResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();
    if normalized.len() > 64 {
        return Err(RegionError::Validation(
            "tax_provider_id must be at most 64 characters".to_string(),
        ));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
    {
        return Err(RegionError::Validation(
            "tax_provider_id must use lowercase ASCII, digits, underscore, or hyphen".to_string(),
        ));
    }
    Ok(Some(normalized))
}

fn normalize_countries(values: Vec<String>) -> RegionResult<Vec<String>> {
    if values.is_empty() {
        return Err(RegionError::Validation(
            "countries must contain at least one country code".to_string(),
        ));
    }

    values
        .into_iter()
        .map(|value| normalize_country_code(&value))
        .collect()
}

fn normalize_country_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(RegionError::InvalidCountryCode(value.to_string()))
    }
}
