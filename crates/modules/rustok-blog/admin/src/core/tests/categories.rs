use super::fixtures::category_snapshot;
use crate::core::*;
use crate::model::BlogCategoryDetail;

#[test]
fn category_save_command_validates_the_name_length() {
    let issue = prepare_blog_category_save_command(
        BlogCategoryFormInput {
            editing_category_id: None,
            loaded_category: None,
            locale: "en",
            name: "   ",
            slug: "",
            description: "",
            parent_id: None,
            position: "0",
        },
        "name too short".to_string(),
        "invalid position".to_string(),
        "slug required".into(),
    )
    .unwrap_err();
    assert_eq!(issue.message, "name too short");
}

#[test]
fn category_save_command_create_trims_optionals_and_parses_position() {
    let command = prepare_blog_category_save_command(
        BlogCategoryFormInput {
            editing_category_id: None,
            loaded_category: None,
            locale: " en ",
            name: " News ",
            slug: "  ",
            description: "  Team updates  ",
            parent_id: Some("  "),
            position: "3",
        },
        "name too short".to_string(),
        "invalid position".to_string(),
        "slug required".into(),
    )
    .unwrap();
    match command.operation {
        BlogCategorySaveOperation::Create(draft) => {
            assert_eq!(draft.locale, "en");
            assert_eq!(draft.name, "News");
            assert_eq!(draft.slug, None);
            assert_eq!(draft.description.as_deref(), Some("Team updates"));
            assert_eq!(draft.parent_id, None);
            assert_eq!(draft.position, Some(3));
        }
        BlogCategorySaveOperation::Update { .. } => panic!("expected a create command"),
    }
    assert_eq!(command.busy_key, busy_key_for_category_save(None));
}

#[test]
fn category_save_command_rejects_negative_or_non_numeric_position() {
    for position in ["-1", "abc"] {
        let issue = prepare_blog_category_save_command(
            BlogCategoryFormInput {
                editing_category_id: None,
                loaded_category: None,
                locale: "en",
                name: "News",
                slug: "",
                description: "",
                parent_id: None,
                position,
            },
            "name too short".to_string(),
            "invalid position".to_string(),
            "slug required".into(),
        )
        .unwrap_err();
        assert_eq!(issue.message, "invalid position");
    }
}

#[test]
fn category_save_command_update_uses_description_patch_semantics() {
    let command = prepare_blog_category_save_command(
        BlogCategoryFormInput {
            editing_category_id: Some("11111111-1111-1111-1111-111111111111"),
            loaded_category: Some(&category_snapshot()),
            locale: "en",
            name: "News",
            slug: "news",
            description: "loaded text",
            parent_id: Some("00000000-0000-0000-0000-000000000002"),
            position: "7",
        },
        "name too short".to_string(),
        "invalid position".to_string(),
        "slug required".into(),
    )
    .unwrap();
    match command.operation {
        BlogCategorySaveOperation::Update {
            category_id,
            update,
        } => {
            assert_eq!(category_id, "11111111-1111-1111-1111-111111111111");
            assert_eq!(update.locale, "en");
            assert_eq!(update.name, None);
            assert_eq!(update.slug, None);
            assert_eq!(update.description, rustok_api::Patch::Keep);
        }
        BlogCategorySaveOperation::Create(_) => panic!("expected an update command"),
    }
    assert_eq!(
        command.busy_key,
        busy_key_for_category_save(Some("11111111-1111-1111-1111-111111111111"))
    );
}

#[test]
fn category_search_matches_name_slug_and_description() {
    let row = blog_category_admin_table_row_view(&BlogCategoryDetail {
        id: "id".to_string(),
        locale: "en".to_string(),
        effective_locale: "en".to_string(),
        name: "Company News".to_string(),
        slug: "company-news".to_string(),
        description: Some("Updates from the team".to_string()),
        parent_id: None,
        position: 2,
        created_at: "2026-06-14T00:00:00Z".to_string(),
        updated_at: None,
    });
    assert!(blog_category_matches_query(&row, "comp"));
    assert!(blog_category_matches_query(&row, "NEWS"));
    assert!(blog_category_matches_query(&row, "company-news"));
    assert!(blog_category_matches_query(&row, "team"));
    assert!(blog_category_matches_query(&row, "  "));
    assert!(!blog_category_matches_query(&row, "roadmap"));
}
