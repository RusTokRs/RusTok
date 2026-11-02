/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Bounded candidate browsing over canonical axes, never a SKU write or another identity model.
use crate::model::{
    ProductEffectiveForm, ProductVariant, VariantAxisConfig, VariantAxisDraft, VariantAxisValue,
    VariantAxisValueDraft, VariantDraft,
};
use std::collections::{BTreeMap, HashSet};

pub(crate) const MAX_CANDIDATES: usize = 4096;
pub(crate) const PAGE_SIZE: usize = 12;
pub(crate) const MAX_MEMBERS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CombinationOption {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CombinationAxis {
    pub attribute_id: String,
    pub label: String,
    pub options: Vec<CombinationOption>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CombinationMember {
    pub id: String,
    pub sku: Option<String>,
    pub axis_values: Vec<VariantAxisValueDraft>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CombinationRow {
    pub key: Vec<(String, String)>,
    pub axis_values: Vec<VariantAxisValueDraft>,
    pub labels: Vec<String>,
    pub existing: Option<CombinationMember>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CombinationPage {
    pub count: usize,
    pub page: usize,
    pub pages: usize,
    pub incomplete_drafts: usize,
    pub rows: Vec<CombinationRow>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CombinationError {
    Axes,
    Filters,
    Members,
    Limit,
    Taken,
    Capacity,
}
impl CombinationError {
    pub fn message_key(self) -> &'static str {
        match self {
            Self::Axes => "product-combinations-axes",
            Self::Filters => "product-combinations-filters",
            Self::Members => "product-combinations-members",
            Self::Limit => "product-combinations-limit",
            Self::Taken => "product-combinations-taken",
            Self::Capacity => "product-combinations-capacity",
        }
    }
}

pub(crate) fn saved_axes(
    axes: &[VariantAxisConfig],
) -> Result<Vec<CombinationAxis>, CombinationError> {
    if axes.len() > 32 || axes.iter().any(|axis| axis.allowed_values.len() > 256) {
        return Err(CombinationError::Axes);
    }
    let rows = axes
        .iter()
        .map(|axis| CombinationAxis {
            attribute_id: axis.attribute_id.clone(),
            label: axis.name.clone(),
            options: axis
                .allowed_values
                .iter()
                .map(|value| CombinationOption {
                    id: value.option_id.clone(),
                    label: value.value.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    validate_axes(&rows)?;
    Ok(rows)
}
pub(crate) fn initial_axes(
    axes: &[VariantAxisDraft],
    form: Option<&ProductEffectiveForm>,
) -> Result<Vec<CombinationAxis>, CombinationError> {
    super::initial_catalog::validate_axes(axes, form).map_err(|_| CombinationError::Axes)?;
    axes.iter()
        .map(|axis| {
            let attribute = form
                .and_then(|form| {
                    form.attributes
                        .iter()
                        .find(|attribute| attribute.attribute_id == axis.attribute_id)
                })
                .ok_or(CombinationError::Axes)?;
            let options = axis
                .allowed_option_ids
                .iter()
                .map(|id| {
                    attribute
                        .options
                        .iter()
                        .find(|option| option.id == *id)
                        .map(|option| CombinationOption {
                            id: option.id.clone(),
                            label: option.label.clone(),
                        })
                        .ok_or(CombinationError::Axes)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CombinationAxis {
                attribute_id: axis.attribute_id.clone(),
                label: attribute.label.clone(),
                options,
            })
        })
        .collect()
}
/// Loaded-assignment presentation only; missing dictionary metadata stays visible as UUIDs.
pub(crate) fn retained_labels(
    axes: &[VariantAxisConfig],
    values: &[VariantAxisValue],
) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            let axis = axes
                .iter()
                .find(|axis| axis.attribute_id == value.attribute_id);
            let name = axis
                .map(|axis| axis.name.as_str())
                .or(value.code.as_deref())
                .unwrap_or(&value.attribute_id);
            let option = axis
                .and_then(|axis| {
                    axis.allowed_values
                        .iter()
                        .find(|option| option.option_id == value.option_id)
                })
                .map(|option| option.value.as_str())
                .or(value.label.as_deref())
                .unwrap_or(&value.option_id);
            format!("{name}: {option}")
        })
        .collect()
}
pub(crate) fn draft_members(
    variants: &[VariantDraft],
) -> Result<Vec<CombinationMember>, CombinationError> {
    if variants.len() > MAX_MEMBERS {
        return Err(CombinationError::Members);
    }
    Ok(variants
        .iter()
        .enumerate()
        .map(|(index, variant)| CombinationMember {
            id: format!("draft-{}", index + 1),
            sku: variant.sku.clone(),
            axis_values: variant.axis_values.clone(),
        })
        .collect())
}
pub(crate) fn saved_members(
    variants: &[ProductVariant],
) -> Result<Vec<CombinationMember>, CombinationError> {
    if variants.len() > MAX_MEMBERS {
        return Err(CombinationError::Members);
    }
    Ok(variants
        .iter()
        .map(|variant| CombinationMember {
            id: variant.id.clone(),
            sku: variant.sku.clone(),
            axis_values: variant
                .axis_values
                .iter()
                .map(|value| VariantAxisValueDraft {
                    attribute_id: value.attribute_id.clone(),
                    option_id: value.option_id.clone(),
                })
                .collect(),
        })
        .collect())
}
/// Local drafts only: no scalar fields copied, no zero price, no stock initialization/write.
pub(crate) fn append_initial_combination(
    axes: &[VariantAxisDraft],
    variants: &mut Vec<VariantDraft>,
    form: Option<&ProductEffectiveForm>,
    values: &[VariantAxisValueDraft],
) -> Result<(), CombinationError> {
    if variants.len() >= super::initial_catalog::MAX_INITIAL_VARIANTS {
        return Err(CombinationError::Capacity);
    }
    let axes = initial_axes(axes, form)?;
    // A complete explicit tuple is a one-candidate space, even when the unfiltered matrix
    // exceeds the browser bound. This does not change any caller's filters/configuration.
    let selected = choose_combination(&axes, values, &draft_members(variants)?, values, true)?;
    variants.push(VariantDraft {
        axis_values: selected,
        ..VariantDraft::default()
    });
    Ok(())
}
type AxisIndex = BTreeMap<String, HashSet<String>>;
fn validate_axes(axes: &[CombinationAxis]) -> Result<AxisIndex, CombinationError> {
    let mut index = BTreeMap::new();
    if axes.len() > 32 {
        return Err(CombinationError::Axes);
    }
    for axis in axes {
        let mut options = HashSet::new();
        if axis.attribute_id.is_empty() || axis.options.is_empty() || axis.options.len() > 256 {
            return Err(CombinationError::Axes);
        }
        for option in &axis.options {
            if option.id.is_empty() || !options.insert(option.id.clone()) {
                return Err(CombinationError::Axes);
            }
        }
        if index.insert(axis.attribute_id.clone(), options).is_some() {
            return Err(CombinationError::Axes);
        }
    }
    Ok(index)
}
fn tuple_key(axes: &AxisIndex, values: &[VariantAxisValueDraft]) -> Option<Vec<(String, String)>> {
    if axes.len() != values.len() {
        return None;
    }
    let mut selected = BTreeMap::new();
    for value in values {
        if !axes
            .get(&value.attribute_id)
            .is_some_and(|options| options.contains(&value.option_id))
            || selected
                .insert(value.attribute_id.clone(), value.option_id.clone())
                .is_some()
        {
            return None;
        }
    }
    Some(selected.into_iter().collect())
}
struct CandidateSpace {
    dimensions: Vec<CombinationAxis>,
    allowed: AxisIndex,
    count: usize,
    index: BTreeMap<Vec<(String, String)>, CombinationMember>,
    incomplete_drafts: usize,
}
fn prepare(
    axes: &[CombinationAxis],
    filters: &[VariantAxisValueDraft],
    members: &[CombinationMember],
    draft: bool,
) -> Result<CandidateSpace, CombinationError> {
    let mut allowed = validate_axes(axes)?;
    if filters.len() > 32 {
        return Err(CombinationError::Filters);
    }
    if members.len() > MAX_MEMBERS {
        return Err(CombinationError::Members);
    }
    if !draft && axes.is_empty() && members.len() != 1 {
        return Err(CombinationError::Members);
    }
    let mut fixed = BTreeMap::new();
    for value in filters {
        if !allowed
            .get(&value.attribute_id)
            .is_some_and(|options| options.contains(&value.option_id))
            || fixed
                .insert(value.attribute_id.clone(), value.option_id.clone())
                .is_some()
        {
            return Err(CombinationError::Filters);
        }
    }
    let mut ids = HashSet::new();
    let mut index = BTreeMap::new();
    let mut incomplete_drafts = 0;
    for member in members {
        if member.id.is_empty() || !ids.insert(&member.id) {
            return Err(CombinationError::Members);
        }
        let Some(key) = tuple_key(&allowed, &member.axis_values) else {
            if !draft {
                return Err(CombinationError::Members);
            }
            incomplete_drafts += 1;
            continue;
        };
        if index.insert(key, member.clone()).is_some() {
            return Err(CombinationError::Members);
        }
    }
    let dimensions = axes
        .iter()
        .map(|axis| CombinationAxis {
            attribute_id: axis.attribute_id.clone(),
            label: axis.label.clone(),
            options: axis
                .options
                .iter()
                .filter(|option| {
                    fixed
                        .get(&axis.attribute_id)
                        .is_none_or(|id| *id == option.id)
                })
                .cloned()
                .collect(),
        })
        .collect::<Vec<_>>();
    let mut count = 1usize;
    for axis in &dimensions {
        if count > MAX_CANDIDATES / axis.options.len() {
            return Err(CombinationError::Limit);
        }
        count *= axis.options.len();
    }
    for (attribute_id, option_id) in fixed {
        allowed.insert(attribute_id, HashSet::from([option_id]));
    }
    Ok(CandidateSpace {
        dimensions,
        allowed,
        count,
        index,
        incomplete_drafts,
    })
}

pub(crate) fn combination_page(
    axes: &[CombinationAxis],
    filters: &[VariantAxisValueDraft],
    members: &[CombinationMember],
    requested_page: usize,
    draft: bool,
) -> Result<CombinationPage, CombinationError> {
    let space = prepare(axes, filters, members, draft)?;
    let pages = space.count.div_ceil(PAGE_SIZE);
    let page = requested_page.min(pages - 1);
    let start = page * PAGE_SIZE;
    let mut rows = Vec::with_capacity(PAGE_SIZE);
    // Mixed radix: one page only. validate_axes and fixed-value validation guarantee that
    // every radix is nonzero and cursor % radix is an in-bounds option index.
    for position in start..space.count.min(start + PAGE_SIZE) {
        let mut cursor = position;
        let mut values = Vec::with_capacity(axes.len());
        let mut labels = Vec::with_capacity(axes.len());
        for axis in space.dimensions.iter().rev() {
            let option = &axis.options[cursor % axis.options.len()];
            cursor /= axis.options.len();
            values.push(VariantAxisValueDraft {
                attribute_id: axis.attribute_id.clone(),
                option_id: option.id.clone(),
            });
            labels.push(format!("{}: {}", axis.label, option.label));
        }
        values.reverse();
        labels.reverse();
        let key = tuple_key(&space.allowed, &values).ok_or(CombinationError::Axes)?;
        let existing = space.index.get(&key).cloned();
        rows.push(CombinationRow {
            key,
            axis_values: values,
            labels,
            existing,
        });
    }
    Ok(CombinationPage {
        count: space.count,
        page,
        pages,
        incomplete_drafts: space.incomplete_drafts,
        rows,
    })
}

pub(crate) fn choose_combination(
    axes: &[CombinationAxis],
    filters: &[VariantAxisValueDraft],
    members: &[CombinationMember],
    values: &[VariantAxisValueDraft],
    draft: bool,
) -> Result<Vec<VariantAxisValueDraft>, CombinationError> {
    let space = prepare(axes, filters, members, draft)?;
    let key = tuple_key(&space.allowed, values).ok_or(CombinationError::Filters)?;
    if space.index.contains_key(&key) {
        return Err(CombinationError::Taken);
    }
    Ok(values.to_vec())
}
pub(crate) fn set_filter(
    filters: &mut Vec<VariantAxisValueDraft>,
    attribute_id: &str,
    option_id: String,
) {
    filters.retain(|value| value.attribute_id != attribute_id);
    if !option_id.is_empty() {
        filters.push(VariantAxisValueDraft {
            attribute_id: attribute_id.to_string(),
            option_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn axis(id: &str, count: usize) -> CombinationAxis {
        CombinationAxis {
            attribute_id: id.into(),
            label: id.into(),
            options: (0..count)
                .map(|n| CombinationOption {
                    id: format!("{id}-{n}"),
                    label: n.to_string(),
                })
                .collect(),
        }
    }
    #[test]
    fn pages_have_deterministic_mixed_radix_order_without_materializing_the_matrix() {
        let axes = vec![axis("color", 4), axis("size", 4)];
        let first = combination_page(&axes, &[], &[], 0, false).unwrap();
        let last = combination_page(&axes, &[], &[], 1, false).unwrap();
        assert_eq!(
            (first.count, first.pages, first.rows.len(), last.rows.len()),
            (16, 2, 12, 4)
        );
        assert_eq!(last.rows[0].axis_values[0].option_id, "color-3");
        assert_eq!(last.rows[0].axis_values[1].option_id, "size-0");
    }
    #[test]
    fn huge_spaces_refuse_instead_of_silently_truncating_and_filters_leave_axes_unchanged() {
        let axes = vec![axis("color", 256), axis("size", 256)];
        let before = axes.clone();
        assert_eq!(
            combination_page(&axes, &[], &[], 0, false),
            Err(CombinationError::Limit)
        );
        let filters = vec![VariantAxisValueDraft {
            attribute_id: "color".into(),
            option_id: "color-7".into(),
        }];
        assert_eq!(
            combination_page(&axes, &filters, &[], 0, false)
                .unwrap()
                .count,
            256
        );
        assert_eq!(axes, before);
        assert_eq!(
            combination_page(
                &axes,
                &[VariantAxisValueDraft {
                    attribute_id: "removed".into(),
                    option_id: "old".into()
                }],
                &[],
                0,
                false
            ),
            Err(CombinationError::Filters)
        );
    }
    #[test]
    fn exact_membership_blocks_duplicates_and_unknown_existing_tuples_never_look_available() {
        let axes = vec![axis("color", 2)];
        let values = vec![VariantAxisValueDraft {
            attribute_id: "color".into(),
            option_id: "color-0".into(),
        }];
        let member = CombinationMember {
            id: "sku-uuid".into(),
            sku: Some("sku".into()),
            axis_values: values.clone(),
        };
        let page = combination_page(&axes, &[], &[member.clone()], 0, false).unwrap();
        assert_eq!(page.rows[0].existing.as_ref().unwrap().id, "sku-uuid");
        assert_eq!(
            choose_combination(&axes, &[], &[member], &values, false),
            Err(CombinationError::Taken)
        );
        let incomplete = CombinationMember {
            id: "draft".into(),
            sku: None,
            axis_values: vec![],
        };
        assert_eq!(
            combination_page(&axes, &[], &[incomplete.clone()], 0, false),
            Err(CombinationError::Members)
        );
        assert_eq!(
            combination_page(&axes, &[], &[incomplete], 0, true)
                .unwrap()
                .incomplete_drafts,
            1
        );
    }
    #[test]
    fn exact_bound_and_extreme_page_are_safe_and_selection_rechecks_the_filter() {
        let axes = vec![axis("x", 64), axis("y", 64)];
        let page = combination_page(&axes, &[], &[], usize::MAX, false).unwrap();
        assert_eq!((page.count, page.page, page.rows.len()), (4096, 341, 4));
        let candidate = &page.rows[0].axis_values;
        let filter = vec![VariantAxisValueDraft {
            attribute_id: "x".into(),
            option_id: "x-0".into(),
        }];
        assert_eq!(
            choose_combination(&axes, &filter, &[], candidate, false),
            Err(CombinationError::Filters)
        );
        let selected = choose_combination(&axes, &[], &[], candidate, false).unwrap();
        assert_eq!(selected, *candidate);
        let occupied = vec![CombinationMember {
            id: "created-since-preview".into(),
            sku: None,
            axis_values: selected,
        }];
        assert_eq!(
            choose_combination(&axes, &[], &occupied, candidate, false),
            Err(CombinationError::Taken)
        );
    }
    #[test]
    fn all_fixed_thirty_two_axes_stay_bounded_and_membership_does_not_depend_on_axis_order() {
        let axes = (0..32)
            .map(|i| axis(&format!("axis-{i}"), 256))
            .collect::<Vec<_>>();
        let filters = axes
            .iter()
            .map(|axis| VariantAxisValueDraft {
                attribute_id: axis.attribute_id.clone(),
                option_id: axis.options[7].id.clone(),
            })
            .collect::<Vec<_>>();
        let page = combination_page(&axes, &filters, &[], 0, false).unwrap();
        assert_eq!(page.count, 1);
        let member = CombinationMember {
            id: "stable-uuid".into(),
            sku: None,
            axis_values: page.rows[0].axis_values.iter().rev().cloned().collect(),
        };
        let reversed = axes.iter().rev().cloned().collect::<Vec<_>>();
        let occupied = combination_page(&reversed, &filters, &[member], 0, false).unwrap();
        assert_eq!(occupied.rows[0].key, page.rows[0].key);
        assert_eq!(
            occupied.rows[0].existing.as_ref().unwrap().id,
            "stable-uuid"
        );
    }
    #[test]
    fn duplicate_and_oversized_membership_refuse_even_outside_the_filter() {
        let axes = vec![axis("x", 2)];
        let values = vec![VariantAxisValueDraft {
            attribute_id: "x".into(),
            option_id: "x-0".into(),
        }];
        let members = vec![
            CombinationMember {
                id: "one".into(),
                sku: None,
                axis_values: values.clone(),
            },
            CombinationMember {
                id: "two".into(),
                sku: None,
                axis_values: values,
            },
        ];
        let filter = vec![VariantAxisValueDraft {
            attribute_id: "x".into(),
            option_id: "x-1".into(),
        }];
        assert_eq!(
            combination_page(&axes, &filter, &members, 0, false),
            Err(CombinationError::Members)
        );
        assert_eq!(
            combination_page(&axes, &[], &vec![members[0].clone(); 257], 0, false),
            Err(CombinationError::Members)
        );
        assert_eq!(
            draft_members(&vec![VariantDraft::default(); 257]),
            Err(CombinationError::Members)
        );
    }
    #[test]
    fn picked_full_tuple_appends_without_bounding_or_narrowing_the_unfiltered_matrix() {
        let dimensions = vec![axis("color", 256), axis("size", 256)];
        let axes = dimensions
            .iter()
            .enumerate()
            .map(|(index, axis)| VariantAxisDraft {
                attribute_id: axis.attribute_id.clone(),
                position: Some(index as i32),
                allowed_option_ids: axis
                    .options
                    .iter()
                    .map(|option| option.id.clone())
                    .collect(),
            })
            .collect::<Vec<_>>();
        let form = ProductEffectiveForm {
            category_id: "category".into(),
            detached_attribute_ids: vec![],
            attributes: dimensions
                .iter()
                .map(|axis| crate::model::ProductEffectiveFormAttribute {
                    attribute_id: axis.attribute_id.clone(),
                    code: axis.attribute_id.clone(),
                    label: axis.label.clone(),
                    value_type: "select".into(),
                    scope: "variant".into(),
                    is_localized: false,
                    options: axis
                        .options
                        .iter()
                        .map(|option| crate::model::ProductAttributeOptionSummary {
                            id: option.id.clone(),
                            code: option.id.clone(),
                            label: option.label.clone(),
                            position: 0,
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
                })
                .collect(),
        };
        let values = vec![
            VariantAxisValueDraft {
                attribute_id: "color".into(),
                option_id: "color-7".into(),
            },
            VariantAxisValueDraft {
                attribute_id: "size".into(),
                option_id: "size-20".into(),
            },
        ];
        assert_eq!(
            combination_page(&dimensions, &[], &[], 0, false),
            Err(CombinationError::Limit)
        );
        assert_eq!(
            combination_page(&dimensions, &values[..1], &[], 0, false)
                .unwrap()
                .count,
            256
        );
        let mut variants = Vec::new();
        append_initial_combination(&axes, &mut variants, Some(&form), &values).unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].axis_values, values);
        assert!(variants[0].prices.is_empty());
        assert_eq!(axes[0].allowed_option_ids.len(), 256);
    }
    #[test]
    fn explicit_append_does_not_copy_price_stock_or_sku_and_default_tuple_is_unique() {
        let mut variants = Vec::new();
        append_initial_combination(&[], &mut variants, None, &[]).unwrap();
        assert_eq!(variants.len(), 1);
        assert!(variants[0].prices.is_empty());
        assert!(variants[0].sku.is_none());
        assert!(variants[0].inventory_quantity.is_none());
        assert_eq!(
            append_initial_combination(&[], &mut variants, None, &[]),
            Err(CombinationError::Taken)
        );
        assert_eq!(variants.len(), 1);
        let mut full = vec![VariantDraft::default(); 128];
        assert_eq!(
            append_initial_combination(&[], &mut full, None, &[]),
            Err(CombinationError::Capacity)
        );
        assert_eq!(full.len(), 128);
    }
    #[test]
    fn no_axes_has_one_default_slot_and_invalid_dictionary_rows_refuse() {
        let page = combination_page(&[], &[], &[], 0, true).unwrap();
        assert_eq!(page.count, 1);
        assert!(page.rows[0].axis_values.is_empty());
        assert_eq!(
            combination_page(&[], &[], &[], 0, false),
            Err(CombinationError::Members)
        );
        assert_eq!(
            choose_combination(
                &[],
                &[],
                &[CombinationMember {
                    id: "default".into(),
                    sku: None,
                    axis_values: vec![]
                }],
                &[],
                false
            ),
            Err(CombinationError::Taken)
        );
        assert_eq!(
            combination_page(&[axis("too-large", 257)], &[], &[], 0, false),
            Err(CombinationError::Axes)
        );
    }
}
