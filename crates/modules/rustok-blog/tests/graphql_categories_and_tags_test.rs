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
    CreateBlogCategoryInput, GqlBlogCategory, GqlBlogTag, UpdateBlogCategoryInput,
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
        parent_id: None,
        position: 0,
        settings: serde_json::json!({"icon": "newspaper"}),
        created_at: now,
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
    assert_eq!(gql.updated_at, None);
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

    let domain: rustok_blog::dto::CreateCategoryInput = input.into();
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
        description: None,
        settings: None,
    };
    let domain_update: rustok_blog::dto::UpdateCategoryInput = update.into();
    assert_eq!(domain_update.locale, "en");
    assert_eq!(domain_update.name, Some("Technology".to_string()));
    assert_eq!(domain_update.slug, None);
}
