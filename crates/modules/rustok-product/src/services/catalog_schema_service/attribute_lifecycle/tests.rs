/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

use super::*;

fn definition() -> AttributeDefinition {
    AttributeDefinition {
        id: Uuid::new_v4(),
        code: "color".into(),
        value_type: "select".into(),
        scope: "both".into(),
        is_localized: false,
        is_filterable: false,
        is_searchable: false,
        is_sortable: false,
        is_comparable: false,
        show_on_storefront: true,
        show_in_admin_grid: false,
        search_weight: 0,
        position: 0,
        is_archived: false,
        translation: None,
        state_token: String::new(),
    }
}
fn input() -> UpdateAttributeDefinitionInput {
    UpdateAttributeDefinitionInput {
        attribute_id: Uuid::new_v4(),
        option_id: None,
        locale: "en".into(),
        expected_state: "a".repeat(64),
        action: AttributeDefinitionAction::Update,
        attribute_patch: Some(AttributeDefinitionPatch {
            is_filterable: Some(false),
            ..Default::default()
        }),
        option_patch: None,
    }
}

#[test]
fn fingerprints_bind_full_presentation_identity_tenant_and_exact_locale_not_product_revision() {
    let tenant = Uuid::new_v4();
    let row = definition();
    let original = fingerprint(tenant, "en", &row).unwrap();
    assert!(valid_state_token(&original));
    assert_eq!(original, fingerprint(tenant, "en", &row).unwrap());
    assert_ne!(original, fingerprint(Uuid::new_v4(), "en", &row).unwrap());
    assert_ne!(original, fingerprint(tenant, "ru", &row).unwrap());
    let mut changed = row.clone();
    changed.is_archived = true;
    assert_ne!(original, fingerprint(tenant, "en", &changed).unwrap());
    let mut changed = row.clone();
    changed.translation = Some(AttributeDefinitionCopy {
        label: "Exact copy".into(),
        help_text: None,
        facet_label: None,
        seo_label: None,
    });
    assert_ne!(original, fingerprint(tenant, "en", &changed).unwrap());
    let option = AttributeOptionDefinition {
        id: Uuid::new_v4(),
        attribute_id: row.id,
        code: "red".into(),
        position: 0,
        is_archived: false,
        attribute_archived: false,
        label: None,
        state_token: String::new(),
    };
    let mut parent_archived = option.clone();
    parent_archived.attribute_archived = true;
    assert_ne!(
        fingerprint(tenant, "en", &option).unwrap(),
        fingerprint(tenant, "en", &parent_archived).unwrap()
    );
}

#[test]
fn archive_restore_and_option_patches_are_disjoint_and_noops_are_explicit() {
    let mut value = input();
    assert!(value.validate().is_ok());
    value.attribute_patch = Some(AttributeDefinitionPatch::default());
    assert!(value.validate().is_err());
    value.action = AttributeDefinitionAction::Archive;
    assert!(value.validate().is_err());
    value.attribute_patch = None;
    assert!(value.validate().is_ok());
    value.action = AttributeDefinitionAction::Restore;
    assert!(value.validate().is_ok());
    value.option_id = Some(Uuid::new_v4());
    assert!(value.validate().is_ok());
    value.action = AttributeDefinitionAction::Update;
    assert!(value.validate().is_err());
    value.option_patch = Some(AttributeOptionDefinitionPatch {
        label: Some("  exact label  ".into()),
        position: Some(0),
    });
    assert!(value.validate().is_ok());
    value.attribute_patch = Some(AttributeDefinitionPatch {
        position: Some(0),
        ..Default::default()
    });
    assert!(value.validate().is_err());
}

#[test]
fn immutable_members_nil_ids_bad_locale_tokens_copy_and_negative_writes_are_refused() {
    for key in [
        "id",
        "code",
        "valueType",
        "scope",
        "isLocalized",
        "validation",
        "defaultValue",
        "metadata",
    ] {
        assert!(
            serde_json::from_value::<AttributeDefinitionPatch>(
                serde_json::json!({(key): "replacement"})
            )
            .is_err()
        );
    }
    let mut value = input();
    value.attribute_id = Uuid::nil();
    assert!(value.validate().is_err());
    let mut value = input();
    value.option_id = Some(Uuid::nil());
    assert!(value.validate().is_err());
    for locale in ["", "und", "EN", "en_US", "en-u-ca-hebrew"] {
        let mut value = input();
        value.locale = locale.into();
        assert!(value.validate().is_err());
    }
    for token in ["", "abc", &"A".repeat(64), &"a".repeat(65)] {
        assert!(!valid_state_token(token));
    }
    let mut value = input();
    value.attribute_patch.as_mut().unwrap().position = Some(-1);
    assert!(value.validate().is_err());
    let mut value = input();
    value.attribute_patch.as_mut().unwrap().search_weight = Some(-1);
    assert!(value.validate().is_err());
    for label in [String::new(), " ".into(), "a\0b".into(), "ю".repeat(256)] {
        assert!(validate_label(&label).is_err());
    }
    let copy = AttributeDefinitionCopy {
        label: "Label".into(),
        help_text: Some("я".repeat(2049)),
        facet_label: None,
        seo_label: None,
    };
    assert!(validate_copy(&copy).is_err());
    assert!(ensure_lifecycle_backend(DatabaseBackend::MySql).is_err());
}

#[test]
fn presentation_patch_preserves_ids_storage_shape_and_missing_locale_without_fallback() {
    let mut row = definition();
    let original = row.clone();
    apply_attribute_patch(
        &mut row,
        &AttributeDefinitionPatch {
            position: Some(7),
            is_searchable: Some(true),
            ..Default::default()
        },
    );
    assert_eq!(row.id, original.id);
    assert_eq!(row.code, original.code);
    assert_eq!(row.value_type, original.value_type);
    assert_eq!(row.scope, original.scope);
    assert_eq!(row.is_localized, original.is_localized);
    assert!(!row.is_archived);
    assert_eq!(row.translation, None);
    assert_eq!(row.position, 7);
    assert!(row.is_searchable);
    let copy = AttributeDefinitionCopy {
        label: "  exact label  ".into(),
        help_text: Some("".into()),
        facet_label: None,
        seo_label: None,
    };
    apply_attribute_patch(
        &mut row,
        &AttributeDefinitionPatch {
            translation: Some(copy.clone()),
            ..Default::default()
        },
    );
    assert_eq!(row.translation, Some(copy));
    let before = row.clone();
    apply_attribute_patch(&mut row, &AttributeDefinitionPatch::default());
    assert_eq!(row, before);
}
