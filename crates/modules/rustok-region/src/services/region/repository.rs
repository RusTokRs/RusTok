use crate::entities;
use chrono::Utc;
use rustok_api::normalize_locale_tag;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::domain::tax_policy::channel_tax_provider_id;
use crate::dto::{
    RegionCountryTaxPolicyInput, RegionCountryTaxPolicyResponse, RegionResponse, RegionTaxPolicy,
    RegionTranslationInput, RegionTranslationResponse,
};
use crate::error::{RegionError, RegionResult};

pub(super) async fn load_tax_policy(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    region_id: Uuid,
    channel_id: Option<Uuid>,
) -> Result<Option<RegionTaxPolicy>, DbErr> {
    // The tenant filter applies to the owning region before any rules are
    // returned. A left join retains the base policy when there are no rules.
    let result = entities::region::Entity::find_by_id(region_id)
        .filter(entities::region::Column::TenantId.eq(tenant_id))
        .find_with_related(entities::region_country_tax_policy::Entity)
        .all(db)
        .await?;

    Ok(result
        .into_iter()
        .next()
        .map(|(region, rules)| RegionTaxPolicy {
            provider_id: region.tax_provider_id,
            channel_provider_id: channel_tax_provider_id(&region.metadata, channel_id),
            tax_rate: region.tax_rate,
            tax_included: region.tax_included,
            country_rules: map_country_tax_policies(rules),
        }))
}

fn map_country_tax_policies(
    policies: Vec<entities::region_country_tax_policy::Model>,
) -> Vec<RegionCountryTaxPolicyResponse> {
    let mut policies = policies
        .into_iter()
        .map(|policy| RegionCountryTaxPolicyResponse {
            country_code: policy.country_code,
            tax_rate: policy.tax_rate,
            tax_included: policy.tax_included,
        })
        .collect::<Vec<_>>();
    policies.sort_by(|left, right| left.country_code.cmp(&right.country_code));
    policies
}

pub(super) async fn load_region(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    region_id: Uuid,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> RegionResult<RegionResponse> {
    let model = entities::region::Entity::find_by_id(region_id)
        .filter(entities::region::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(RegionError::RegionNotFound(region_id))?;
    let items =
        load_regions_with_translations(db, vec![model], requested_locale, tenant_default_locale)
            .await?;
    items
        .into_iter()
        .next()
        .ok_or(RegionError::RegionNotFound(region_id))
}

pub(super) async fn load_regions_with_translations(
    db: &impl ConnectionTrait,
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
        country_tax_policies: map_country_tax_policies(country_tax_policies),
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

pub(super) async fn insert_translations<C>(
    db: &C,
    region_id: Uuid,
    translations: &[RegionTranslationInput],
) -> RegionResult<()>
where
    C: ConnectionTrait,
{
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

pub(super) async fn replace_translations<C>(
    db: &C,
    region_id: Uuid,
    translations: &[RegionTranslationInput],
) -> RegionResult<()>
where
    C: ConnectionTrait,
{
    entities::region_translation::Entity::delete_many()
        .filter(entities::region_translation::Column::RegionId.eq(region_id))
        .exec(db)
        .await?;
    insert_translations(db, region_id, translations).await
}

pub(super) async fn replace_country_tax_policies<C>(
    db: &C,
    region_id: Uuid,
    policies: &[RegionCountryTaxPolicyInput],
) -> RegionResult<()>
where
    C: ConnectionTrait,
{
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

    if let Some(locale) = requested_locale.and_then(normalize_locale_tag)
        && let Some(found) = lookup.get(&locale)
    {
        return (Some(*found), Some(found.locale.clone()));
    }
    if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag)
        && let Some(found) = lookup.get(&locale)
    {
        return (Some(*found), Some(found.locale.clone()));
    }
    translations
        .first()
        .map(|item| (Some(item), Some(item.locale.clone())))
        .unwrap_or((None, None))
}
