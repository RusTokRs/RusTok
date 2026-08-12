use thiserror::Error;
use uuid::Uuid;

use rustok_index::{IndexQueryItem, IndexQueryPage, IndexValue};

const UNTITLED_PRODUCT: &str = "Untitled product";

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ProductStorefrontIndexPublicProjectionError {
    #[error("Product Storefront Index item {entity_id} is missing projected root field {field}")]
    MissingField {
        entity_id: Uuid,
        field: &'static str,
    },
    #[error("Product Storefront Index item {entity_id} projects root field {field} more than once")]
    DuplicateField {
        entity_id: Uuid,
        field: &'static str,
    },
    #[error(
        "Product Storefront Index item {entity_id} projected root field {field} must be string or null"
    )]
    InvalidFieldValue {
        entity_id: Uuid,
        field: &'static str,
    },
}

/// Apply Product owner public placeholders only after the generic Index page is fully decoded.
///
/// This transform never participates in filtering, sorting, localized identity folding, exact count,
/// pagination, or cursor construction. It changes only already-projected root `title`/`handle` nulls on the
/// retained page. `tag_ids` remain untouched until the separate post-page Taxonomy hydration slice.
pub(crate) fn project_product_storefront_index_page(
    mut page: IndexQueryPage,
) -> Result<IndexQueryPage, ProductStorefrontIndexPublicProjectionError> {
    for item in &mut page.items {
        apply_string_placeholder(item, "title", UNTITLED_PRODUCT)?;
        apply_string_placeholder(item, "handle", "")?;
    }
    Ok(page)
}

fn apply_string_placeholder(
    item: &mut IndexQueryItem,
    field: &'static str,
    placeholder: &'static str,
) -> Result<(), ProductStorefrontIndexPublicProjectionError> {
    let mut position = None;
    for (candidate, projected) in item.fields.iter().enumerate() {
        if projected.path.links().is_empty() && projected.path.field().as_str() == field {
            if position.replace(candidate).is_some() {
                return Err(
                    ProductStorefrontIndexPublicProjectionError::DuplicateField {
                        entity_id: item.entity_id,
                        field,
                    },
                );
            }
        }
    }
    let position = position.ok_or(ProductStorefrontIndexPublicProjectionError::MissingField {
        entity_id: item.entity_id,
        field,
    })?;
    let value = &mut item.fields[position].value;
    match value {
        IndexValue::Null => {
            *value = IndexValue::String(placeholder.to_owned());
            Ok(())
        }
        IndexValue::String(_) => Ok(()),
        _ => Err(
            ProductStorefrontIndexPublicProjectionError::InvalidFieldValue {
                entity_id: item.entity_id,
                field,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_index::{FieldName, FieldPath, IndexProjectedValue};

    fn projected(field: &str, value: IndexValue) -> IndexProjectedValue {
        IndexProjectedValue {
            path: FieldPath::new(FieldName::new(field).unwrap()),
            value,
        }
    }

    fn item(entity_id: Uuid, fields: Vec<IndexProjectedValue>) -> IndexQueryItem {
        IndexQueryItem {
            entity_id,
            relations: Vec::new(),
            fields,
            nested_relations: Vec::new(),
        }
    }

    fn value<'a>(item: &'a IndexQueryItem, field: &str) -> &'a IndexValue {
        &item
            .fields
            .iter()
            .find(|projected| {
                projected.path.links().is_empty() && projected.path.field().as_str() == field
            })
            .unwrap()
            .value
    }

    #[test]
    fn maps_only_public_placeholders_after_page_identity_is_fixed() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let tag_id = Uuid::new_v4();
        let page = IndexQueryPage {
            items: vec![
                item(
                    first,
                    vec![
                        projected("title", IndexValue::Null),
                        projected("handle", IndexValue::Null),
                        projected("vendor", IndexValue::Null),
                        projected("tag_ids", IndexValue::List(vec![IndexValue::Uuid(tag_id)])),
                    ],
                ),
                item(
                    second,
                    vec![
                        projected("title", IndexValue::String("Existing".to_owned())),
                        projected("handle", IndexValue::String("existing".to_owned())),
                        projected("vendor", IndexValue::String("Vendor".to_owned())),
                        projected("tag_ids", IndexValue::List(Vec::new())),
                    ],
                ),
            ],
            exact_count: Some(9),
            has_more: true,
            next_cursor: Some("opaque-cursor".to_owned()),
        };

        let projected = project_product_storefront_index_page(page).unwrap();
        assert_eq!(
            projected
                .items
                .iter()
                .map(|item| item.entity_id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(projected.exact_count, Some(9));
        assert!(projected.has_more);
        assert_eq!(projected.next_cursor.as_deref(), Some("opaque-cursor"));
        assert_eq!(
            value(&projected.items[0], "title"),
            &IndexValue::String("Untitled product".to_owned())
        );
        assert_eq!(
            value(&projected.items[0], "handle"),
            &IndexValue::String(String::new())
        );
        assert_eq!(value(&projected.items[0], "vendor"), &IndexValue::Null);
        assert_eq!(
            value(&projected.items[0], "tag_ids"),
            &IndexValue::List(vec![IndexValue::Uuid(tag_id)])
        );
        assert_eq!(
            value(&projected.items[1], "title"),
            &IndexValue::String("Existing".to_owned())
        );
        assert_eq!(
            value(&projected.items[1], "handle"),
            &IndexValue::String("existing".to_owned())
        );
    }

    #[test]
    fn fails_closed_on_missing_duplicate_or_wrong_typed_public_fields() {
        let missing = IndexQueryPage {
            items: vec![item(
                Uuid::new_v4(),
                vec![projected("title", IndexValue::Null)],
            )],
            exact_count: Some(1),
            has_more: false,
            next_cursor: None,
        };
        assert!(matches!(
            project_product_storefront_index_page(missing),
            Err(ProductStorefrontIndexPublicProjectionError::MissingField {
                field: "handle",
                ..
            })
        ));

        let duplicate = IndexQueryPage {
            items: vec![item(
                Uuid::new_v4(),
                vec![
                    projected("title", IndexValue::Null),
                    projected("title", IndexValue::String("duplicate".to_owned())),
                    projected("handle", IndexValue::Null),
                ],
            )],
            exact_count: Some(1),
            has_more: false,
            next_cursor: None,
        };
        assert!(matches!(
            project_product_storefront_index_page(duplicate),
            Err(ProductStorefrontIndexPublicProjectionError::DuplicateField { field: "title", .. })
        ));

        let wrong_typed = IndexQueryPage {
            items: vec![item(
                Uuid::new_v4(),
                vec![
                    projected("title", IndexValue::Uuid(Uuid::new_v4())),
                    projected("handle", IndexValue::Null),
                ],
            )],
            exact_count: Some(1),
            has_more: false,
            next_cursor: None,
        };
        assert!(matches!(
            project_product_storefront_index_page(wrong_typed),
            Err(
                ProductStorefrontIndexPublicProjectionError::InvalidFieldValue {
                    field: "title",
                    ..
                }
            )
        ));
    }
}
