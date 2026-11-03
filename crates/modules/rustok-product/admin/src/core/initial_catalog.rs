/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Explicit sparse initial selection. This core never generates a Cartesian product.
use std::collections::{BTreeMap, HashSet};

use crate::model::{
    ProductDraft, ProductEffectiveForm, VariantAxisDraft, VariantAxisValueDraft, VariantDraft,
    VariantPriceDraft,
};

pub(crate) const MAX_INITIAL_VARIANTS: usize = 128;

pub(crate) fn suggested_axes(form: &ProductEffectiveForm) -> Result<Vec<VariantAxisDraft>, String> {
    let axes = form
        .attributes
        .iter()
        .filter(|attribute| {
            super::is_eligible_axis(attribute)
                && (attribute.default_variant_axis || attribute.variant_axis_policy == "required")
        })
        .enumerate()
        .map(|(position, attribute)| VariantAxisDraft {
            attribute_id: attribute.attribute_id.clone(),
            position: Some(position as i32),
            allowed_option_ids: attribute
                .options
                .iter()
                .map(|option| option.id.clone())
                .collect(),
        })
        .collect::<Vec<_>>();
    validate_axes(&axes, Some(form))?;
    Ok(axes)
}

pub(crate) fn validate_axes(
    axes: &[VariantAxisDraft],
    form: Option<&ProductEffectiveForm>,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    if axes.len() > 32 {
        return Err("At most 32 distinct axes are supported".into());
    }
    for axis in axes {
        if !seen.insert(axis.attribute_id.as_str()) {
            return Err("Duplicate variant axis".into());
        }
        let attribute = form
            .and_then(|form| {
                form.attributes
                    .iter()
                    .find(|attribute| attribute.attribute_id == axis.attribute_id)
            })
            .filter(|attribute| super::is_eligible_axis(attribute))
            .ok_or(
                "An axis is no longer allowed in the selected category. Review the retained draft.",
            )?;
        let values = axis.allowed_option_ids.iter().collect::<HashSet<_>>();
        if values.len() != axis.allowed_option_ids.len()
            || values.is_empty()
            || values.len() > 256
            || values
                .iter()
                .any(|id| !attribute.options.iter().any(|option| &option.id == *id))
        {
            return Err("Every axis requires 1–256 distinct active dictionary values".into());
        }
    }
    for attribute in form.into_iter().flat_map(|form| &form.attributes) {
        if !attribute.is_disabled
            && attribute.variant_axis_policy == "required"
            && !seen.contains(attribute.attribute_id.as_str())
        {
            return Err(format!("Required axis {} is missing", attribute.label));
        }
    }
    Ok(())
}

pub(crate) fn validate_selection(
    axes: &[VariantAxisDraft],
    variants: &[VariantDraft],
    form: Option<&ProductEffectiveForm>,
) -> Result<(), String> {
    validate_axes(axes, form)?;
    if variants.is_empty()
        || variants.len() > MAX_INITIAL_VARIANTS
        || (axes.is_empty() && variants.len() != 1)
    {
        return Err(
            "Select 1–128 initial SKUs; a product without axes has exactly one default SKU".into(),
        );
    }
    let mut identities = HashSet::new();
    let mut skus = HashSet::new();
    for variant in variants {
        if variant.axis_values.len() != axes.len() {
            return Err("Select one allowed value per axis for each SKU".into());
        }
        let mut values = BTreeMap::new();
        for value in &variant.axis_values {
            let valid = axes.iter().any(|axis| {
                axis.attribute_id == value.attribute_id
                    && axis.allowed_option_ids.contains(&value.option_id)
            });
            if !valid
                || values
                    .insert(value.attribute_id.clone(), value.option_id.clone())
                    .is_some()
            {
                return Err(
                    "An SKU has a duplicate axis or a value outside the allowed set".into(),
                );
            }
        }
        // Draft-only identity; the Product owner computes the durable combination identity.
        if !identities.insert(values.into_iter().collect::<Vec<_>>()) {
            return Err("Duplicate SKU combination".into());
        }
        if let Some(sku) = variant.sku.as_ref().filter(|sku| !sku.trim().is_empty()) {
            if sku.chars().count() > 100 || !skus.insert(sku.trim()) {
                return Err("SKU codes must be unique and at most 100 characters".into());
            }
        }
        if variant
            .barcode
            .as_ref()
            .is_some_and(|barcode| barcode.chars().count() > 100)
            || variant
                .inventory_quantity
                .is_some_and(|quantity| quantity < 0)
        {
            return Err("Check barcode length and non-negative inventory".into());
        }
        if variant
            .inventory_policy
            .as_ref()
            .is_some_and(|policy| !matches!(policy.as_str(), "deny" | "continue"))
        {
            return Err("Invalid inventory policy".into());
        }
        for price in &variant.prices {
            if price.currency_code.len() != 3
                || !price
                    .currency_code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphabetic())
            {
                return Err("Currency must use a three-letter code".into());
            }
            validate_amount(&price.amount)?;
            if let Some(amount) = price
                .compare_at_amount
                .as_ref()
                .filter(|amount| !amount.trim().is_empty())
            {
                validate_amount(amount)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_amount(value: &str) -> Result<(), String> {
    let amount =
        rust_decimal::Decimal::from_str_exact(value.trim()).map_err(|_| "Invalid decimal price")?;
    if amount.is_sign_negative() {
        return Err("Price cannot be negative".into());
    }
    Ok(())
}

pub(crate) fn scalar_variant(draft: &ProductDraft) -> VariantDraft {
    variant_from_editor_fields(
        draft.sku.clone(),
        draft.barcode.clone(),
        draft.inventory_quantity,
        draft.inventory_policy.to_ascii_lowercase(),
        VariantPriceDraft {
            currency_code: draft.currency_code.trim().to_ascii_uppercase(),
            amount: draft.amount.clone(),
            compare_at_amount: super::text_or_none(draft.compare_at_amount.clone()),
        },
        Vec::new(),
    )
}

/// Shared scalar inputs are consumed only when no explicit axis/SKU draft exists.
pub(crate) fn uses_scalar_initial_variant(
    axes: &[VariantAxisDraft],
    variants: &[VariantDraft],
) -> bool {
    axes.is_empty() && variants.is_empty()
}

pub(crate) fn apply_selection(
    draft: &mut ProductDraft,
    axes: Vec<VariantAxisDraft>,
    variants: Vec<VariantDraft>,
    form: Option<&ProductEffectiveForm>,
) -> Result<(), String> {
    let variants = if uses_scalar_initial_variant(&axes, &variants) {
        vec![scalar_variant(draft)]
    } else {
        variants
    };
    validate_selection(&axes, &variants, form)?;
    draft.variant_axes = axes;
    draft.initial_variants = Some(variants);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_product_has_exactly_one_unassigned_sku() {
        assert!(validate_selection(&[], &[VariantDraft::default()], None).is_ok());
        assert!(validate_selection(&[], &[], None).is_err());
        assert!(validate_selection(
            &[],
            &[VariantDraft::default(), VariantDraft::default()],
            None
        )
        .is_err());
    }

    #[test]
    fn negative_prices_and_stale_assignments_are_not_accepted() {
        let variant = VariantDraft {
            prices: vec![VariantPriceDraft {
                amount: "-1.00".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(validate_selection(&[], &[variant], None).is_err());
        let variant = VariantDraft {
            axis_values: vec![crate::model::VariantAxisValueDraft {
                attribute_id: "color".into(),
                option_id: "red".into(),
            }],
            ..Default::default()
        };
        assert!(validate_selection(&[], &[variant], None).is_err());
    }
}

pub(crate) fn add_axis(
    axes: &mut Vec<VariantAxisDraft>,
    attribute_id: &str,
    form: &ProductEffectiveForm,
) -> Result<(), String> {
    if axes.len() >= 32 || axes.iter().any(|axis| axis.attribute_id == attribute_id) {
        return Err("Select a distinct eligible axis (at most 32)".into());
    }
    let attribute = form
        .attributes
        .iter()
        .find(|attribute| {
            attribute.attribute_id == attribute_id && super::is_eligible_axis(attribute)
        })
        .ok_or("Select an eligible attribute")?;
    if attribute.options.is_empty() || attribute.options.len() > 256 {
        return Err("An axis must allow 1–256 dictionary values".into());
    }
    for (position, axis) in axes.iter_mut().enumerate() {
        axis.position = Some(position as i32);
    }
    axes.push(VariantAxisDraft {
        attribute_id: attribute_id.into(),
        position: Some(axes.len() as i32),
        allowed_option_ids: attribute
            .options
            .iter()
            .map(|option| option.id.clone())
            .collect(),
    });
    Ok(())
}

pub(crate) fn set_allowed_value(
    axes: &mut [VariantAxisDraft],
    attribute_id: &str,
    option_id: &str,
    allowed: bool,
) {
    if let Some(axis) = axes
        .iter_mut()
        .find(|axis| axis.attribute_id == attribute_id)
    {
        if allowed && !axis.allowed_option_ids.iter().any(|id| id == option_id) {
            axis.allowed_option_ids.push(option_id.into());
        }
        if !allowed {
            axis.allowed_option_ids.retain(|id| id != option_id);
        }
    }
}

pub(crate) fn append_variant(
    axes: &[VariantAxisDraft],
    variants: &mut Vec<VariantDraft>,
    mut variant: VariantDraft,
    form: Option<&ProductEffectiveForm>,
) -> Result<(), String> {
    variant.inventory_policy = variant
        .inventory_policy
        .map(|policy| policy.trim().to_ascii_lowercase());
    for price in &mut variant.prices {
        price.currency_code = price.currency_code.trim().to_ascii_uppercase();
    }
    let mut next = variants.clone();
    next.push(variant);
    validate_selection(axes, &next, form)?;
    *variants = next;
    Ok(())
}

pub(crate) fn remove_axis(axes: &mut Vec<VariantAxisDraft>, attribute_id: &str) {
    axes.retain(|axis| axis.attribute_id != attribute_id);
    for (position, axis) in axes.iter_mut().enumerate() {
        axis.position = Some(position as i32);
    }
}

pub(crate) fn validate_saved_tuple(
    axes: &[crate::model::VariantAxisConfig],
    values: &[crate::model::VariantAxisValueDraft],
) -> Result<(), String> {
    if values.len() != axes.len() {
        return Err("Select exactly one allowed value for each configured axis".into());
    }
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(&value.attribute_id)
            || !axes.iter().any(|axis| {
                axis.attribute_id == value.attribute_id
                    && axis
                        .allowed_values
                        .iter()
                        .any(|allowed| allowed.option_id == value.option_id)
            })
        {
            return Err(
                "An SKU assignment is duplicated or outside its axis allowed values".into(),
            );
        }
    }
    Ok(())
}

pub(crate) fn parse_inventory_quantity(value: &str) -> Result<i32, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Inventory must be a non-negative 32-bit integer".into());
    }
    value
        .parse::<i32>()
        .map_err(|_| "Inventory must be a non-negative 32-bit integer".into())
}

pub(crate) fn normalize_standalone_variant(
    mut draft: VariantDraft,
    axes: &[crate::model::VariantAxisConfig],
) -> Result<VariantDraft, String> {
    validate_saved_tuple(axes, &draft.axis_values)?;
    for price in &mut draft.prices {
        price.currency_code = price.currency_code.trim().to_ascii_uppercase();
        if price.currency_code.len() != 3
            || !price
                .currency_code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase())
        {
            return Err("Use a three-letter ISO currency code".into());
        }
        validate_amount(&price.amount)?;
        price.amount = price
            .amount
            .trim()
            .parse::<rust_decimal::Decimal>()
            .map_err(|_| "Invalid decimal price")?
            .normalize()
            .to_string();
        if let Some(amount) = price.compare_at_amount.as_mut() {
            validate_amount(amount)?;
            *amount = amount
                .trim()
                .parse::<rust_decimal::Decimal>()
                .map_err(|_| "Invalid compare-at price")?
                .normalize()
                .to_string();
        }
    }
    if draft
        .inventory_quantity
        .is_some_and(|quantity| quantity < 0)
    {
        return Err("Inventory must be a non-negative integer".into());
    }
    if let Some(policy) = draft.inventory_policy.as_mut() {
        *policy = policy.to_ascii_lowercase();
        if !matches!(policy.as_str(), "deny" | "continue") {
            return Err("Choose a supported inventory policy".into());
        }
    }
    draft.sku = draft.sku.and_then(super::text_or_none);
    draft.barcode = draft.barcode.and_then(super::text_or_none);
    Ok(draft)
}

#[cfg(test)]
mod assignment_tests {
    use super::*;
    use crate::model::{AxisAllowedValue, VariantAxisConfig, VariantAxisValueDraft};

    #[test]
    fn inventory_input_never_coerces_invalid_or_overflowed_text_to_zero() {
        assert_eq!(parse_inventory_quantity("0").unwrap(), 0);
        assert_eq!(parse_inventory_quantity("2147483647").unwrap(), i32::MAX);
        for value in ["", "-1", "1.5", "NaN", "2147483648"] {
            assert!(parse_inventory_quantity(value).is_err());
        }
    }

    #[test]
    fn standalone_sku_uses_exact_saved_assignments_and_does_not_invent_a_free_price() {
        let axes = vec![VariantAxisConfig {
            id: "axis".into(),
            attribute_id: "color".into(),
            code: "color".into(),
            name: "Color".into(),
            position: 0,
            allowed_values: vec![AxisAllowedValue {
                option_id: "red".into(),
                value: "Red".into(),
                position: 0,
            }],
        }];
        let draft = VariantDraft {
            axis_values: vec![VariantAxisValueDraft {
                attribute_id: "color".into(),
                option_id: "red".into(),
            }],
            prices: vec![VariantPriceDraft {
                currency_code: "usd".into(),
                amount: "12.500001".into(),
                compare_at_amount: None,
            }],
            inventory_quantity: Some(3),
            inventory_policy: Some("DENY".into()),
            ..Default::default()
        };
        let normalized = normalize_standalone_variant(draft.clone(), &axes).unwrap();
        assert_eq!(normalized.prices[0].amount, "12.500001");
        assert_eq!(normalized.inventory_policy.as_deref(), Some("deny"));
        let mut blank = draft.clone();
        blank.prices[0].amount.clear();
        assert!(normalize_standalone_variant(blank, &axes).is_err());
        let mut missing = draft.clone();
        missing.axis_values.clear();
        assert!(normalize_standalone_variant(missing, &axes).is_err());
        let mut duplicate = draft.clone();
        duplicate.axis_values.push(draft.axis_values[0].clone());
        assert!(normalize_standalone_variant(duplicate, &axes).is_err());
        let mut outside = draft;
        outside.axis_values[0].option_id = "blue".into();
        assert!(normalize_standalone_variant(outside, &axes).is_err());
    }

    #[test]
    fn removing_an_axis_renumbers_positions_without_changing_identity() {
        let mut axes = vec![
            VariantAxisDraft {
                attribute_id: "color".into(),
                position: Some(0),
                allowed_option_ids: vec!["red".into()],
            },
            VariantAxisDraft {
                attribute_id: "size".into(),
                position: Some(1),
                allowed_option_ids: vec!["small".into()],
            },
        ];
        remove_axis(&mut axes, "color");
        assert_eq!(axes[0].attribute_id, "size");
        assert_eq!(axes[0].position, Some(0));
        assert_eq!(axes[0].allowed_option_ids, vec!["small"]);
    }
}

/// Read-only control projection. Mounting/resetting a Select never publishes a value.
pub(crate) fn pending_assignment_value(
    values: &[VariantAxisValueDraft],
    attribute_id: &str,
) -> String {
    values
        .iter()
        .find(|value| value.attribute_id == attribute_id)
        .map(|value| value.option_id.clone())
        .unwrap_or_default()
}

/// Keep a stale selected ID visible and disabled instead of showing an unrelated default.
pub(crate) fn pending_assignment_choices(
    axes: &[VariantAxisDraft],
    values: &[VariantAxisValueDraft],
    attribute_id: &str,
    form: Option<&ProductEffectiveForm>,
) -> Vec<super::variant_axes::AxisOption> {
    let allowed = axes
        .iter()
        .find(|axis| axis.attribute_id == attribute_id)
        .map(|axis| axis.allowed_option_ids.as_slice())
        .unwrap_or_default();
    let attribute = form.and_then(|form| {
        form.attributes
            .iter()
            .find(|attribute| attribute.attribute_id == attribute_id)
    });
    let mut choices = attribute
        .map(|attribute| {
            attribute
                .options
                .iter()
                .filter(|option| allowed.contains(&option.id))
                .map(|option| super::variant_axes::AxisOption {
                    id: option.id.clone(),
                    label: option.label.clone(),
                    available: super::is_eligible_axis(attribute),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let selected = pending_assignment_value(values, attribute_id);
    if !selected.is_empty() && !choices.iter().any(|option| option.id == selected) {
        choices.push(super::variant_axes::AxisOption {
            label: attribute
                .and_then(|attribute| {
                    attribute
                        .options
                        .iter()
                        .find(|option| option.id == selected)
                })
                .map(|option| option.label.clone())
                .unwrap_or_else(|| selected.clone()),
            id: selected,
            available: false,
        });
    }
    choices
}

pub(crate) fn set_pending_assignment(
    values: &mut Vec<crate::model::VariantAxisValueDraft>,
    attribute_id: &str,
    option_id: String,
) {
    values.retain(|value| value.attribute_id != attribute_id);
    if !option_id.is_empty() {
        values.push(crate::model::VariantAxisValueDraft {
            attribute_id: attribute_id.into(),
            option_id,
        });
    }
}

pub(crate) fn suggested_axes_for_empty_draft(
    axes: &[VariantAxisDraft],
    form: Option<&ProductEffectiveForm>,
) -> Result<Vec<VariantAxisDraft>, String> {
    if !axes.is_empty() {
        return Err("Remove existing draft axes explicitly before applying suggestions".into());
    }
    suggested_axes(form.ok_or("Load the selected category form first")?)
}

pub(crate) fn initial_variant_summary(variant: &VariantDraft) -> String {
    let price = variant
        .prices
        .iter()
        .map(|price| format!("{} {}", price.amount, price.currency_code))
        .collect::<Vec<_>>()
        .join(", ");
    let tuple = variant
        .axis_values
        .iter()
        .map(|value| format!("{}={}", value.attribute_id, value.option_id))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}; stock {}; price [{}]; {}",
        variant.sku.as_deref().unwrap_or("No SKU code"),
        variant.inventory_quantity.unwrap_or_default(),
        price,
        tuple
    )
}

pub(crate) fn variant_from_editor_fields(
    sku: String,
    barcode: String,
    inventory_quantity: i32,
    inventory_policy: String,
    mut price: VariantPriceDraft,
    axis_values: Vec<crate::model::VariantAxisValueDraft>,
) -> VariantDraft {
    price.compare_at_amount = price.compare_at_amount.and_then(super::text_or_none);
    VariantDraft {
        sku: super::text_or_none(sku),
        barcode: super::text_or_none(barcode),
        shipping_profile_slug: None,
        axis_values,
        // Blank is no initial price, not a free price. Keep an orphan compare-at for validation.
        prices: if price.amount.trim().is_empty() && price.compare_at_amount.is_none() {
            Vec::new()
        } else {
            vec![price]
        },
        inventory_quantity: Some(inventory_quantity),
        inventory_policy: Some(inventory_policy),
    }
}

#[cfg(test)]
mod optional_initial_price_tests {
    use super::*;
    #[test]
    fn blank_is_absent_but_explicit_zero_and_invalid_compare_at_are_not_discarded() {
        let build = |amount: &str, compare: Option<&str>| {
            variant_from_editor_fields(
                String::new(),
                String::new(),
                0,
                "deny".into(),
                VariantPriceDraft {
                    currency_code: "USD".into(),
                    amount: amount.into(),
                    compare_at_amount: compare.map(str::to_string),
                },
                vec![],
            )
        };
        let absent = build(" ", Some(" "));
        assert!(absent.prices.is_empty());
        assert!(validate_selection(&[], &[absent], None).is_ok());
        let zero = build("0", Some("1.000001"));
        assert_eq!(zero.prices[0].amount, "0");
        assert!(validate_selection(&[], &[zero], None).is_ok());
        let orphan = build("", Some("1"));
        assert_eq!(orphan.prices.len(), 1);
        assert!(validate_selection(&[], &[orphan], None).is_err());
    }
}

#[cfg(test)]
mod pending_assignment_tests {
    use super::*;
    use crate::model::{ProductAttributeOptionSummary, ProductEffectiveFormAttribute};

    fn source() -> (Vec<VariantAxisDraft>, ProductEffectiveForm) {
        let axes = vec![VariantAxisDraft {
            attribute_id: "color".into(),
            position: Some(0),
            allowed_option_ids: vec!["red".into(), "blue".into()],
        }];
        let form = ProductEffectiveForm {
            category_id: "category".into(),
            detached_attribute_ids: vec![],
            attributes: vec![ProductEffectiveFormAttribute {
                attribute_id: "color".into(),
                code: "color".into(),
                label: "Color".into(),
                value_type: "select".into(),
                scope: "variant".into(),
                is_localized: false,
                options: [("red", "Red"), ("blue", "Blue")]
                    .into_iter()
                    .enumerate()
                    .map(|(position, (id, label))| ProductAttributeOptionSummary {
                        id: id.into(),
                        code: id.into(),
                        label: label.into(),
                        position: position as i32,
                    })
                    .collect(),
                group_code: None,
                group_label: None,
                is_required: false,
                is_disabled: false,
                position: 0,
                source: "direct".into(),
                variant_axis_policy: "allowed".into(),
                default_variant_axis: false,
            }],
        };
        (axes, form)
    }

    #[test]
    fn pending_controls_seed_from_the_picked_tuple_without_mutating_it() {
        let (axes, form) = source();
        let mut values = vec![
            VariantAxisValueDraft {
                attribute_id: "color".into(),
                option_id: "blue".into(),
            },
            VariantAxisValueDraft {
                attribute_id: "size".into(),
                option_id: "large".into(),
            },
        ];
        let before = values.clone();
        assert_eq!(pending_assignment_value(&values, "color"), "blue");
        let choices = pending_assignment_choices(&axes, &values, "color", Some(&form));
        assert!(choices
            .iter()
            .any(|option| option.id == "blue" && option.available));
        assert_eq!(values, before);
        set_pending_assignment(&mut values, "color", "red".into());
        assert_eq!(pending_assignment_value(&values, "color"), "red");
        assert_eq!(pending_assignment_value(&values, "size"), "large");
        values.clear();
        assert_eq!(pending_assignment_value(&values, "color"), "");
    }

    #[test]
    fn stale_pending_selection_stays_visible_but_unavailable_without_pruning() {
        let (mut axes, mut form) = source();
        let values = vec![VariantAxisValueDraft {
            attribute_id: "color".into(),
            option_id: "blue".into(),
        }];
        axes[0].allowed_option_ids = vec!["red".into()];
        let choices = pending_assignment_choices(&axes, &values, "color", Some(&form));
        assert_eq!(
            choices
                .iter()
                .find(|option| option.id == "blue")
                .unwrap()
                .label,
            "Blue"
        );
        assert!(
            !choices
                .iter()
                .find(|option| option.id == "blue")
                .unwrap()
                .available
        );
        let missing_form = pending_assignment_choices(&axes, &values, "color", None);
        assert_eq!(missing_form.len(), 1);
        assert_eq!(missing_form[0].id, "blue");
        assert!(!missing_form[0].available);
        form.attributes[0].is_disabled = true;
        assert!(
            pending_assignment_choices(&axes, &values, "color", Some(&form))
                .iter()
                .all(|option| !option.available)
        );
        assert_eq!(values[0].option_id, "blue");
    }

    #[test]
    fn explicit_rows_do_not_consume_unused_invalid_shared_scalar_fields() {
        let mut draft = ProductDraft {
            inventory_quantity: -1,
            amount: "invalid-unused-amount".into(),
            inventory_policy: "invalid-unused-policy".into(),
            ..Default::default()
        };
        let row = VariantDraft {
            inventory_quantity: Some(4),
            ..Default::default()
        };
        assert!(!uses_scalar_initial_variant(&[], &[row.clone()]));
        apply_selection(&mut draft, vec![], vec![row], None).unwrap();
        let selected = draft.initial_variants.as_ref().unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].inventory_quantity, Some(4));
        assert!(selected[0].prices.is_empty());
        assert!(uses_scalar_initial_variant(&[], &[]));
        assert!(apply_selection(&mut draft, vec![], vec![], None).is_err());
    }

    #[test]
    fn axes_without_selected_rows_do_not_fall_back_to_a_scalar_default_sku() {
        let (axes, form) = source();
        let mut draft = ProductDraft::default();
        assert!(!uses_scalar_initial_variant(&axes, &[]));
        assert!(apply_selection(&mut draft, axes, vec![], Some(&form)).is_err());
        assert!(draft.initial_variants.is_none());
    }
}
