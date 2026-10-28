/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use chrono::Utc;
use rustok_blog::dto::{CategoryListItem, TagListItem};
use rustok_blog::graphql::{
    CreateBlogCategoryInput, GqlBlogCategory, GqlBlogTag, MaybeUndefined, UpdateBlogCategoryInput,
};
use uuid::Uuid;

#[test]
fn test_category_list_item_into_gql_blog_category() {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let item = CategoryListItem {
        id,
        locale: "ru".to_string(),
        effective_locale: "ru".to_string(),
        name: "Новости".to_string(),
        slug: "novosti".to_string(),
        description: Some("Новости платформы".to_string()),
        parent_id: None,
        position: 0,
        settings: serde_json::json!({"icon": "newspaper"}),
        created_at: now,
        updated_at: now,
    };

    let gql: GqlBlogCategory = item.into();
    assert_eq!(gql.id, id);
    assert_eq!(gql.name, "Новости");
    assert_eq!(gql.slug, "novosti");
    assert_eq!(gql.locale, "ru");
    assert_eq!(gql.effective_locale, "ru");
    assert_eq!(gql.position, 0);
    assert!(gql.settings.contains("newspaper"));
    assert_eq!(gql.created_at, now.to_rfc3339());
    assert_eq!(
        gql.description.as_deref(),
        Some("Новости платформы"),
        "the list projection must carry the canonical description"
    );
    assert_eq!(
        gql.updated_at.as_deref(),
        Some(now.to_rfc3339().as_str()),
        "the list projection must carry the update timestamp"
    );
}

#[test]
fn test_tag_list_item_into_gql_blog_tag() {
    let id = Uuid::new_v4();
    let now = Utc::now();
    let item = TagListItem {
        id,
        locale: "ru".to_string(),
        effective_locale: "ru".to_string(),
        name: "Rust".to_string(),
        slug: "rust".to_string(),
        use_count: 42,
        created_at: now,
    };

    let gql: GqlBlogTag = item.into();
    assert_eq!(gql.id, id);
    assert_eq!(gql.name, "Rust");
    assert_eq!(gql.slug, "rust");
    assert_eq!(gql.use_count, 42);
    assert_eq!(gql.created_at, now.to_rfc3339());
}

#[test]
fn test_create_and_update_category_input_conversions() {
    let input = CreateBlogCategoryInput {
        locale: "en".to_string(),
        name: "Tech".to_string(),
        slug: Some("tech".to_string()),
        description: Some("Technology news".to_string()),
        parent_id: None,
        position: Some(1),
        settings: Some("{\"featured\": true}".to_string()),
    };

    let domain: rustok_blog::dto::CreateCategoryInput =
        input.try_into().expect("valid input converts");
    assert_eq!(domain.locale, "en");
    assert_eq!(domain.name, "Tech");
    assert_eq!(domain.slug, Some("tech".to_string()));
    assert_eq!(domain.description, Some("Technology news".to_string()));
    assert_eq!(domain.position, Some(1));
    assert_eq!(domain.settings["featured"], true);

    let update = UpdateBlogCategoryInput {
        locale: "en".to_string(),
        name: Some("Technology".to_string()),
        slug: None,
        description: MaybeUndefined::Undefined,
        settings: None,
    };
    let domain_update: rustok_blog::dto::UpdateCategoryInput =
        update.try_into().expect("valid update converts");
    assert_eq!(domain_update.locale, "en");
    assert_eq!(domain_update.name, Some("Technology".to_string()));
    assert_eq!(domain_update.slug, None);
    assert_eq!(
        domain_update.description,
        rustok_api::Patch::Keep,
        "an absent GraphQL field must not clear the stored description"
    );

    // An explicit `null` is the only way to clear the field, and it must not be
    // collapsed into "not supplied".
    let clearing = UpdateBlogCategoryInput {
        locale: "en".to_string(),
        name: None,
        slug: None,
        description: MaybeUndefined::Null,
        settings: None,
    };
    let domain_clearing: rustok_blog::dto::UpdateCategoryInput =
        clearing.try_into().expect("a clearing update converts");
    assert_eq!(domain_clearing.description, rustok_api::Patch::Clear);
}

#[test]
fn category_input_conversion_rejects_invalid_settings_json() {
    let invalid_create = CreateBlogCategoryInput {
        locale: "en".to_string(),
        name: "Tech".to_string(),
        slug: None,
        description: None,
        parent_id: None,
        position: None,
        settings: Some("{not json".to_string()),
    };
    let create_result = rustok_blog::dto::CreateCategoryInput::try_from(invalid_create);
    assert!(create_result.is_err());

    let invalid_update = UpdateBlogCategoryInput {
        locale: "en".to_string(),
        name: None,
        slug: None,
        description: MaybeUndefined::Undefined,
        settings: Some("[1,".to_string()),
    };
    let update_result = rustok_blog::dto::UpdateCategoryInput::try_from(invalid_update);
    assert!(update_result.is_err());
}
